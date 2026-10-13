//! Exact private control-state ABI, scoped to the disassembled 617.14 driver.
//! Not a background telemetry source; only explicit curve operations initialize it.
use super::{
    gpu::NvidiaIdentity,
    nvidia::{system_library, text_buffer},
};
use libloading::Library;
use omen_core::vf::{VfNode, VfState};
use sha2::{Digest, Sha256};
use std::{ffi::c_void, fs::File, io::Read, ptr};
use windows_sys::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};

type Handle = *mut c_void;
type Query = unsafe extern "C" fn(u32) -> *const c_void;
type StatusFn = unsafe extern "C" fn() -> i32;
type Enumerate = unsafe extern "C" fn(*mut Handle, *mut i32) -> i32;
type Name = unsafe extern "C" fn(Handle, *mut i8) -> i32;
type Buffer = unsafe extern "C" fn(Handle, *mut u32) -> i32;
const MASK_BYTES: usize = 0x182c;
const CURVE_BYTES: usize = 0x1c28;
const CONTROL_BYTES: usize = 0x2420;
const RANGES_BYTES: usize = 0x928;
const HEADER_WORDS: usize = 17;
const IMPL_SHA: &str = "ff1527cea533c75f89033f7c0d9cdbaa6ec84bcac26c1db082227d68efd6a16e";
struct Blocks {
    mask: Vec<u32>,
    curve: Vec<u32>,
    control: Vec<u32>,
    ranges: Vec<u32>,
}

pub(super) struct VfControl {
    _library: Library,
    query: Query,
    unload: StatusFn,
    handle: Handle,
    mask: Buffer,
    curve: Buffer,
    control: Buffer,
    ranges: Buffer,
}
fn status(label: &str, result: i32) -> Result<(), String> {
    if result == 0 {
        Ok(())
    } else {
        Err(format!("NVAPI V/F {label}: status {result}"))
    }
}
fn implementation_hash(address: *const c_void) -> Result<(), String> {
    let mut module = ptr::null_mut();
    // SAFETY: FROM_ADDRESS interprets the pointer as code, not a string. Library
    // ownership and successful NVAPI init retain its module during this query.
    if unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            address.cast(),
            &mut module,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let mut path = vec![0_u16; 32768];
    // SAFETY: Live module and bounded writable UTF-16 buffer.
    let count =
        unsafe { GetModuleFileNameW(module, path.as_mut_ptr(), path.len() as u32) } as usize;
    if count == 0 || count >= path.len() {
        return Err("Private NVIDIA implementation path unavailable".into());
    }
    let path = String::from_utf16(&path[..count]).map_err(|e| e.to_string())?;
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut chunk = [0; 8192];
    loop {
        let count = file.read(&mut chunk).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&chunk[..count]);
    }
    if format!("{:x}", hash.finalize()) != IMPL_SHA {
        return Err(
            "Private V/F ABI evidence does not match the loaded NVIDIA implementation".into(),
        );
    }
    Ok(())
}
impl VfControl {
    pub fn new(identity: &NvidiaIdentity) -> Result<Self, String> {
        if identity.driver != "617.14"
            || identity.vbios != "98.06.2a.40.da"
            || identity.name != "NVIDIA GeForce RTX 5070 Laptop GPU"
        {
            return Err("V/F reader is not validated for this driver/VBIOS/GPU".into());
        }
        let library = system_library("nvapi64.dll")?;
        // SAFETY: Exact public QueryInterface and fixed public/private signatures.
        unsafe {
            let query: Query = *library
                .get(b"nvapi_QueryInterface\0")
                .map_err(|e| e.to_string())?;
            let get = |id| {
                let address = query(id);
                if address.is_null() {
                    Err(format!("Missing V/F interface {id:#x}"))
                } else {
                    Ok(address)
                }
            };
            let init: StatusFn = std::mem::transmute(get(0x0150e828)?);
            let unload: StatusFn = std::mem::transmute(get(0xd22bdd7e)?);
            let enumerate: Enumerate = std::mem::transmute(get(0xe5ac921f)?);
            let name: Name = std::mem::transmute(get(0xceee8e9f)?);
            let mask: Buffer = std::mem::transmute(get(0x507b4b59)?);
            let curve: Buffer = std::mem::transmute(get(0x21537ad4)?);
            let control: Buffer = std::mem::transmute(get(0x23f1b133)?);
            let ranges: Buffer = std::mem::transmute(get(0x64b43a6a)?);
            let control_address = get(0x23f1b133)?;
            status("initialize", init())?;
            // RAII is armed before every further error path.
            let mut session = Self {
                _library: library,
                query,
                unload,
                handle: ptr::null_mut(),
                mask,
                curve,
                control,
                ranges,
            };
            implementation_hash(control_address)?;
            let mut handles = [ptr::null_mut(); 64];
            let mut count = 0;
            status(
                "GPU enumeration",
                enumerate(handles.as_mut_ptr(), &mut count),
            )?;
            if count != 1 || handles[0].is_null() {
                return Err("V/F control requires one unambiguous NVIDIA GPU".into());
            }
            session.handle = handles[0];
            let mut actual = [0_i8; 64];
            status("GPU name", name(session.handle, actual.as_mut_ptr()))?;
            if text_buffer(&actual).trim_start_matches("NVIDIA ")
                != identity.name.trim_start_matches("NVIDIA ")
            {
                return Err("V/F and NVML device identities differ".into());
            }
            session.snapshot()?;
            Ok(session)
        }
    }
    fn get(&self, getter: Buffer, bytes: usize, masks: Option<&[u32]>) -> Result<Vec<u32>, String> {
        let mut words = vec![0_u32; bytes / 4];
        words[0] = (1 << 16) | bytes as u32;
        if let Some(masks) = masks {
            if masks.len() != 8 {
                return Err("Incomplete V/F mask".into());
            }
            words[1..9].copy_from_slice(masks);
        }
        // SAFETY: Exact validated v1 size/alignment; retained library and live GPU.
        status("GET", unsafe { getter(self.handle, words.as_mut_ptr()) })?;
        if words[0] != (1 << 16) | bytes as u32 {
            return Err("V/F returned another ABI version".into());
        }
        Ok(words)
    }
    fn blocks(&self) -> Result<Blocks, String> {
        let mask = self.get(self.mask, MASK_BYTES, None)?;
        let curve = self.get(self.curve, CURVE_BYTES, Some(&mask[1..9]))?;
        let control = self.get(self.control, CONTROL_BYTES, Some(&mask[1..9]))?;
        let ranges = self.get(self.ranges, RANGES_BYTES, None)?;
        if curve[1..9] != mask[1..9] || control[1..9] != mask[1..9] {
            return Err("V/F GETs have different node masks".into());
        }
        Ok(Blocks {
            mask,
            curve,
            control,
            ranges,
        })
    }
    pub fn snapshot(&self) -> Result<VfState, String> {
        let Blocks {
            mask,
            curve,
            control,
            ranges,
        } = self.blocks()?;
        let domains: Vec<_> = (0..32)
            .filter(|index| ranges[1] & (1 << index) != 0)
            .filter(|index| {
                let start = 10 + index * 18;
                ranges[start] == 0 && ranges[start + 1] == 0 // frequency / public Graphics
            })
            .collect();
        if domains.len() != 1 {
            return Err("No single Graphics V/F domain".into());
        }
        let start = 10 + domains[0] * 18;
        let bounds = ranges[start + 12];
        let first = (bounds & 255) as u16;
        let last = ((bounds >> 8) & 255) as u16;
        if last >= 255 || last < first {
            return Err("Invalid Graphics V/F point range".into());
        }
        let mut nodes = vec![];
        for index in first..=last {
            let index_usize = usize::from(index);
            let cstart = HEADER_WORDS + index_usize * 7;
            let dstart = HEADER_WORDS + index_usize * 9;
            if mask[1 + index_usize / 32] & (1 << (index_usize % 32)) == 0
                || mask[HEADER_WORDS + index_usize * 6] != 0
                || curve[cstart] != 0
                || control[dstart] != 0
            {
                return Err("Graphics V/F node is missing or not writable as a frequency".into());
            }
            nodes.push(VfNode {
                index,
                frequency_khz: curve[cstart + 1],
                voltage_uv: curve[cstart + 2],
                delta_khz: control[dstart + 5] as i32,
            });
        }
        let state = VfState {
            nodes,
            minimum_delta_khz: ranges[start + 11] as i32,
            maximum_delta_khz: ranges[start + 10] as i32,
            first_index: first,
            last_index: last,
        };
        state.validate()?;
        Ok(state)
    }
    /// Only the frequency-delta word is mutable. Voltage anchors and memory nodes
    /// are never written. Caller supplies a complete typed Graphics domain.
    pub fn set_deltas(&self, saved: &VfState, deltas: &[(u16, i32)]) -> Result<(), String> {
        saved.validate()?;
        let current = self.snapshot()?;
        if !saved.controls_match(&current)? {
            return Err("V/F controls changed before SET; another controller may be active".into());
        }
        if saved.first_index != current.first_index
            || saved.last_index != current.last_index
            || saved.minimum_delta_khz != current.minimum_delta_khz
            || saved.maximum_delta_khz != current.maximum_delta_khz
            || saved
                .nodes
                .iter()
                .zip(&current.nodes)
                .any(|(a, b)| a.index != b.index || a.voltage_uv != b.voltage_uv)
        {
            return Err("V/F grid/domain/limits changed before SET".into());
        }
        if deltas.len() != current.nodes.len()
            || deltas
                .iter()
                .zip(&current.nodes)
                .any(|((index, value), node)| {
                    *index != node.index
                        || !(current.minimum_delta_khz..=current.maximum_delta_khz).contains(value)
                })
        {
            return Err("V/F SET has incomplete, misplaced or out-of-range deltas".into());
        }
        let Blocks {
            mask, mut control, ..
        } = self.blocks()?;
        control[1..9].fill(0);
        for (index, delta) in deltas {
            let index = usize::from(*index);
            if mask[1 + index / 32] & (1 << (index % 32)) == 0 {
                return Err("V/F node no longer available".into());
            }
            control[1 + index / 32] |= 1 << (index % 32);
            control[HEADER_WORDS + index * 9 + 5] = *delta as u32;
        }
        // SAFETY: Setter signature and v1 layout are traced in the exact loaded
        // driver. Only validated Graphics masks/frequency words differ from GET.
        let address = unsafe { (self.query)(0x0733e009) };
        if address.is_null() {
            return Err("V/F frequency setter unavailable".into());
        }
        implementation_hash(address)?;
        let setter: Buffer = unsafe { std::mem::transmute(address) };
        status("frequency SET", unsafe {
            setter(self.handle, control.as_mut_ptr())
        })?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let state = self.snapshot()?;
            if state
                .nodes
                .iter()
                .zip(deltas)
                .all(|(node, (index, delta))| node.index == *index && node.delta_khz == *delta)
            {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err("V/F delta readback differs from the complete request".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}
impl Drop for VfControl {
    fn drop(&mut self) {
        // SAFETY: Paired init, library still owned.
        unsafe {
            (self.unload)();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_driver_v1_sizes_and_offsets() {
        assert_eq!(68 + 255 * 24, MASK_BYTES);
        assert_eq!(68 + 255 * 28, CURVE_BYTES);
        assert_eq!(68 + 255 * 36, CONTROL_BYTES);
        assert_eq!(40 + 32 * 72, RANGES_BYTES);
        assert_eq!((HEADER_WORDS + 5) * 4, 0x58);
        assert_eq!((1 << 16) | CONTROL_BYTES, 0x12420);
    }
}
