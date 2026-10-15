//! AMD SMU mailbox through the signed PawnIO RyzenSMU module. Only fixed
//! Raphael/Dragon Range RSMU messages are reachable; no caller-chosen opcode.
use super::cpu_hardware::{EXECUTE, Handle, LOAD, VERSION, driver_identity, ioctl, wide};
use omen_core::Inventory;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::ptr;
use windows_sys::Win32::{
    Foundation::{
        GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0,
    },
    Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    },
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};

pub const SMU_MODULE_SHA256: &str =
    "301d9ca397108e09f31bfbd5ac4c9bb4f352a5de68532c32db3ba7ddcde93450";
#[cfg(smu_module)]
const MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/smu-module.bin"));
#[cfg(not(smu_module))]
const MODULE: &[u8] = &[];

/// Raphael / Dragon Range RSMU messages (ZenStates-Core Zen4Settings).
// Setters stay unused until the read-only probe has qualified readback.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    SetPptLimit,
    SetTdcLimit,
    SetEdcLimit,
    SetTctlMax,
    SetPboScalar,
    GetPboScalar,
    SetAllCoreMargin,
    SetCoreMargin,
    GetCoreMargin,
    GetFusedPpt,
    GetFusedTdc,
}
impl Message {
    fn id(self) -> u64 {
        match self {
            Self::SetPptLimit => 0x56,
            Self::SetTdcLimit => 0x57,
            Self::SetEdcLimit => 0x58,
            Self::SetTctlMax => 0x59,
            Self::SetPboScalar => 0x5B,
            Self::GetPboScalar => 0x6D,
            Self::SetAllCoreMargin => 0x07,
            Self::SetCoreMargin => 0x06,
            Self::GetCoreMargin => 0xD5,
            Self::GetFusedPpt => 0xDC,
            Self::GetFusedTdc => 0xDB,
        }
    }
}

/// Raphael-family core mask: CCD in bits 31..28, core in bits 23..20.
pub fn core_mask(ccd: u8, core: u8) -> u32 {
    (u32::from(ccd) << 28) | ((u32::from(core) % 8) << 20)
}
/// Curve Optimizer margins travel as a 16-bit two's-complement field.
pub fn margin_arg(margin: i32) -> u32 {
    (margin as i16 as u16) as u32
}
pub fn margin_value(raw: u32) -> i32 {
    i32::from(raw as u16 as i16)
}

pub fn module_present() -> bool {
    !MODULE.is_empty()
}

pub struct Smu {
    driver: Handle,
    pci: Handle,
}

struct PciLease<'a>(&'a Handle);
impl Drop for PciLease<'_> {
    fn drop(&mut self) {
        // SAFETY: Released only after a successful wait by this thread.
        unsafe {
            ReleaseMutex(self.0.0);
        }
    }
}

fn packet(name: &str, cells: &[u64]) -> Vec<u8> {
    let mut packet = vec![0u8; 32 + 8 * cells.len()];
    packet[..name.len()].copy_from_slice(name.as_bytes());
    for (i, cell) in cells.iter().enumerate() {
        packet[32 + 8 * i..40 + 8 * i].copy_from_slice(&cell.to_le_bytes());
    }
    packet
}
fn cells(bytes: &[u8]) -> Vec<u64> {
    bytes
        .chunks_exact(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

impl Smu {
    pub fn new(inventory: &Inventory) -> Result<Self, String> {
        if MODULE.is_empty() {
            return Err("Diese App-Version enthält keinen SMU-Adapter.".into());
        }
        if inventory.manufacturer != "HP"
            || inventory.board != "8E35"
            || inventory.bios != "F.13"
            || inventory.cpus.len() != 1
            || !inventory.cpus[0].contains("AMD Ryzen 9 8940HX")
        {
            return Err("CPU tuning is qualified only for HP8E35/F.13/Ryzen9 8940HX".into());
        }
        driver_identity()?;
        // SAFETY: Fixed device path; the handle is owned and closed on drop.
        let handle = unsafe {
            CreateFileW(
                wide("\\\\?\\GLOBALROOT\\Device\\PawnIO").as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!(
                "CPU tuning requires administrator rights: {}",
                std::io::Error::last_os_error()
            ));
        }
        let driver = Handle(handle);
        let mut version = [0; 4];
        if ioctl(&driver, VERSION, &[], &mut version)? != 4
            || u32::from_le_bytes(version) != 0x00020200
        {
            return Err("PawnIO version differs from the measured 2.2.0 ABI".into());
        }
        if format!("{:x}", Sha256::digest(MODULE)) != SMU_MODULE_SHA256 {
            return Err("Embedded SMU module differs from its signed release".into());
        }
        ioctl(&driver, LOAD, MODULE, &mut []).map_err(|e| format!("SMU module load: {e}"))?;
        // SAFETY: Named shared mutex other PawnIO/ZenStates tools also use.
        let pci = unsafe { CreateMutexW(ptr::null(), 0, wide("Global\\Access_PCI").as_ptr()) };
        if pci.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let smu = Self {
            driver,
            pci: Handle(pci),
        };
        // The module itself refuses unknown CPUs; double-check its decision.
        let name = smu.call("ioctl_get_code_name", &[], 1)?;
        if name.first().copied().unwrap_or(0) == 0 {
            return Err("SMU module did not recognise this CPU".into());
        }
        Ok(smu)
    }
    fn call(&self, name: &str, input: &[u64], outputs: usize) -> Result<Vec<u64>, String> {
        // SAFETY: Valid mutex handle; a bounded wait never blocks the app forever.
        let wait = unsafe { WaitForSingleObject(self.pci.0, 500) };
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            return Err("SMU busy: shared PCI mutex not acquired".into());
        }
        let _lease = PciLease(&self.pci);
        let mut out = vec![0u8; 8 * outputs];
        let count = ioctl(&self.driver, EXECUTE, &packet(name, input), &mut out)? as usize;
        if count != out.len() {
            return Err(format!("{name} returned {count} of {} bytes", out.len()));
        }
        Ok(cells(&out))
    }
    /// Sends one fixed RSMU message; returns the six response arguments.
    pub fn send(&self, message: Message, arg: u32) -> Result<[u32; 6], String> {
        let out = self
            .call(
                "ioctl_send_smu_command",
                &[message.id(), u64::from(arg), 0, 0, 0, 0, 0],
                6,
            )
            .map_err(|e| format!("SMU {message:?}: {e}"))?;
        Ok(std::array::from_fn(|i| out[i] as u32))
    }
    /// Refreshes and reads the PM table as 32-bit floats.
    pub fn pm_table(&self, floats: usize) -> Result<(u32, Vec<f32>), String> {
        let resolved = self.call("ioctl_resolve_pm_table", &[], 2)?;
        self.call("ioctl_update_pm_table", &[], 0)?;
        let raw = self.call("ioctl_read_pm_table", &[], floats.div_ceil(2))?;
        let values = raw
            .iter()
            .flat_map(|cell| {
                [
                    f32::from_bits(*cell as u32),
                    f32::from_bits((*cell >> 32) as u32),
                ]
            })
            .take(floats)
            .collect();
        Ok((resolved[0] as u32, values))
    }
    pub fn smu_version(&self) -> Result<u32, String> {
        Ok(self.call("ioctl_get_smu_version", &[], 1)?[0] as u32)
    }
}

/// Read-only diagnostic used to qualify readback before any SMU write.
#[derive(Debug, Serialize)]
pub struct SmuProbe {
    pub smu_version: Result<u32, String>,
    pub pm_table_version: Result<u32, String>,
    pub pm_table_head: Result<Vec<f32>, String>,
    pub pbo_scalar_raw: Result<u32, String>,
    pub fused_ppt_raw: Result<u32, String>,
    pub fused_tdc_raw: Result<u32, String>,
    pub core_margins: Vec<Result<i32, String>>,
}
pub fn probe(inventory: &Inventory) -> Result<SmuProbe, String> {
    let smu = Smu::new(inventory)?;
    let table = smu.pm_table(64);
    Ok(SmuProbe {
        smu_version: smu.smu_version(),
        pm_table_version: table.as_ref().map(|t| t.0).map_err(Clone::clone),
        pm_table_head: table.map(|t| t.1),
        pbo_scalar_raw: smu.send(Message::GetPboScalar, 0).map(|r| r[0]),
        fused_ppt_raw: smu.send(Message::GetFusedPpt, 0).map(|r| r[0]),
        fused_tdc_raw: smu.send(Message::GetFusedTdc, 0).map(|r| r[0]),
        // 8940HX: two CCDs with eight cores each.
        core_margins: (0..16u8)
            .map(|i| {
                smu.send(Message::GetCoreMargin, core_mask(i / 8, i % 8))
                    .map(|r| margin_value(r[0]))
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn core_mask_and_margin_encoding_match_zenstates() {
        assert_eq!(core_mask(1, 3), 0x1030_0000);
        assert_eq!(margin_arg(-15), 0xFFF1);
        assert_eq!(margin_value(0xFFF1), -15);
        assert_eq!(margin_value(margin_arg(30)), 30);
        let p = packet("ioctl_send_smu_command", &[0x56, 45000]);
        assert_eq!(&p[..22], b"ioctl_send_smu_command");
        assert_eq!(u64::from_le_bytes(p[40..48].try_into().unwrap()), 45000);
    }
}
