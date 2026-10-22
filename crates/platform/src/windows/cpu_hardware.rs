//! Exact, optional read-only PawnIO transport. No driver installation, SDK init,
//! write-MSR, free function names or caller-selected registers.
mod clocks;

use clocks::ClockReader;
use omen_core::{
    CpuHardwareSample, CpuHardwareSource, CpuTemperatureKind, Inventory,
    cpu::{AmdPackageEnergy, amd_package_watts, amd_tctl_c},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Read as _, ptr, time::Instant};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, WAIT_ABANDONED,
        WAIT_OBJECT_0,
    },
    Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    },
    System::{
        IO::DeviceIoControl,
        Services::*,
        SystemInformation::GetWindowsDirectoryW,
        Threading::{
            CreateMutexW, GetCurrentProcessorNumber, GetCurrentThread, ReleaseMutex,
            SetThreadAffinityMask, WaitForSingleObject,
        },
    },
};

pub const CPU_MODULE_SHA256: &str =
    "dae74615761b78bdf064dfb3e136252ddcc6fc727d88f14738d0e5800d427a91";
pub const CPU_DRIVER_SHA256: &str =
    "fca6e7d58b0cf38dbb913a2b9e532f48629145d395f454b16a9f58e97b8d3940";
#[cfg(cpu_sensor_module)]
const MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cpu-module.bin"));
#[cfg(not(cpu_sensor_module))]
const MODULE: &[u8] = &[];
pub(super) const VERSION: u32 = (41394 << 16) | (0x861 << 2);
pub(super) const LOAD: u32 = (41394 << 16) | (0x821 << 2);
pub(super) const EXECUTE: u32 = (41394 << 16) | (0x841 << 2);

pub(super) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
pub(super) struct Handle(pub(super) HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
struct Affinity(usize);
impl Affinity {
    fn cpu_zero() -> Result<Self, String> {
        Self::pin(0)
    }
    fn pin(processor: u32) -> Result<Self, String> {
        // The qualified device has one package and one processor group. Save
        // and restore this thread's exact mask; never set a process policy.
        let mask = 1usize
            .checked_shl(processor)
            .ok_or("CPU affinity index is invalid")?;
        let old = unsafe { SetThreadAffinityMask(GetCurrentThread(), mask) };
        if old == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let guard = Self(old);
        if unsafe { GetCurrentProcessorNumber() } != processor {
            return Err("CPU reader is not running on the requested logical processor".into());
        }
        Ok(guard)
    }
    fn restore(mut self) -> Result<(), String> {
        if unsafe { SetThreadAffinityMask(GetCurrentThread(), self.0) } == 0 {
            return Err(format!(
                "CPU reader affinity restore: {}",
                std::io::Error::last_os_error()
            ));
        }
        self.0 = 0;
        Ok(())
    }
}
impl Drop for Affinity {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                SetThreadAffinityMask(GetCurrentThread(), self.0);
            }
        }
    }
}
struct PciLease<'a>(&'a Handle);
impl Drop for PciLease<'_> {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0.0);
        }
    }
}
#[derive(Clone, Copy)]
enum Read {
    EnergyUnits,
    PackageEnergy,
    Temperature,
    ActualCycles,
    ReferenceCycles,
}
impl Read {
    fn packet(self) -> [u8; 40] {
        let (name, register): (&[u8], u64) = match self {
            Self::EnergyUnits => (b"ioctl_read_msr", 0xc0010299),
            Self::PackageEnergy => (b"ioctl_read_msr", 0xc001029b),
            Self::Temperature => (b"ioctl_read_smn", 0x59800),
            Self::ActualCycles => (b"ioctl_read_msr", 0xc00000e8),
            Self::ReferenceCycles => (b"ioctl_read_msr", 0xc00000e7),
        };
        let mut packet = [0; 40];
        packet[..name.len()].copy_from_slice(name);
        packet[32..].copy_from_slice(&register.to_le_bytes());
        packet
    }
}
pub(super) fn driver_identity() -> Result<(), String> {
    // Query metadata only. The driver must already run. Never call a service
    // create/start/stop API or load an SDK to repair missing support.
    struct Service(SC_HANDLE);
    impl Drop for Service {
        fn drop(&mut self) {
            unsafe { CloseServiceHandle(self.0) };
        }
    }
    let manager = unsafe { OpenSCManagerW(ptr::null(), ptr::null(), SC_MANAGER_CONNECT) };
    if manager.is_null() {
        return Err(format!(
            "CPU driver metadata: {}",
            std::io::Error::last_os_error()
        ));
    }
    let manager = Service(manager);
    let service = unsafe {
        OpenServiceW(
            manager.0,
            wide("PawnIO").as_ptr(),
            SERVICE_QUERY_CONFIG | SERVICE_QUERY_STATUS,
        )
    };
    if service.is_null() {
        return Err(format!(
            "PawnIO must already be installed and running: {}",
            std::io::Error::last_os_error()
        ));
    }
    let service = Service(service);
    let mut status: SERVICE_STATUS_PROCESS = unsafe { std::mem::zeroed() };
    let mut needed = 0;
    if unsafe {
        QueryServiceStatusEx(
            service.0,
            SC_STATUS_PROCESS_INFO,
            (&mut status as *mut SERVICE_STATUS_PROCESS).cast(),
            size_of::<SERVICE_STATUS_PROCESS>() as u32,
            &mut needed,
        )
    } == 0
    {
        return Err(format!(
            "CPU driver status: {}",
            std::io::Error::last_os_error()
        ));
    }
    if status.dwCurrentState != SERVICE_RUNNING || status.dwServiceType != SERVICE_KERNEL_DRIVER {
        return Err("PawnIO must already be installed and running; no driver was started".into());
    }
    // Windows documents an 8-KiB maximum. Pointer-sized storage preserves the
    // native structure alignment; the returned UTF-16 pointer is range checked.
    let mut config = [0usize; 8192 / size_of::<usize>()];
    if unsafe {
        QueryServiceConfigW(
            service.0,
            config.as_mut_ptr().cast(),
            size_of_val(&config) as u32,
            &mut needed,
        )
    } == 0
    {
        return Err(format!(
            "CPU driver configuration: {}",
            std::io::Error::last_os_error()
        ));
    }
    let path_ptr = unsafe { &*config.as_ptr().cast::<QUERY_SERVICE_CONFIGW>() }.lpBinaryPathName;
    let start = config.as_ptr() as usize;
    let end = start + size_of_val(&config);
    let address = path_ptr as usize;
    if address < start || address >= end || address % align_of::<u16>() != 0 {
        return Err("PawnIO driver path is unavailable".into());
    }
    let path_cells = unsafe { std::slice::from_raw_parts(path_ptr, (end - address) / 2) };
    let length = path_cells
        .iter()
        .position(|v| *v == 0)
        .ok_or("PawnIO driver path is unterminated")?;
    let mut path = String::from_utf16(&path_cells[..length]).map_err(|e| e.to_string())?;
    if path.starts_with("\\SystemRoot\\") {
        let mut directory = [0u16; 512];
        let count = unsafe { GetWindowsDirectoryW(directory.as_mut_ptr(), directory.len() as u32) }
            as usize;
        if count == 0 || count >= directory.len() {
            return Err("PawnIO driver Windows directory is unavailable".into());
        }
        let directory = String::from_utf16(&directory[..count]).map_err(|e| e.to_string())?;
        path = format!("{directory}\\{}", &path[12..]);
    }
    if !std::path::Path::new(&path).is_absolute()
        || !path.to_ascii_lowercase().ends_with("\\pawnio.sys")
    {
        return Err("PawnIO driver path differs from the qualified installation".into());
    }
    let file = fs::File::open(&path).map_err(|e| e.to_string())?;
    // Stream the same bounded identity check. A temporary growing file Vec
    // needlessly retains allocator pages in the long-lived read-only helper.
    let mut file = file.take(1_048_577);
    let mut buffer = [0u8; 8192];
    let mut bytes = 0usize;
    let mut digest = Sha256::new();
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        bytes += count;
        digest.update(&buffer[..count]);
    }
    if bytes > 1_048_576 || format!("{:x}", digest.finalize()) != CPU_DRIVER_SHA256 {
        return Err("PawnIO driver hash differs from the measured 2.2.0 contract".into());
    }
    Ok(())
}
pub struct CpuHardwareReader {
    driver: Handle,
    pci: Handle,
    baseline: Option<(AmdPackageEnergy, Instant)>,
    clocks: ClockReader,
}
impl CpuHardwareReader {
    pub fn new(inventory: &Inventory) -> Result<Self, String> {
        if MODULE.is_empty() {
            return Err(
                "This build has no staged signed CPU module; use scripts/prepare-cpu-module.ps1"
                    .into(),
            );
        }
        if inventory.manufacturer != "HP"
            || inventory.board != "8E35"
            || inventory.bios != "F.13"
            || inventory.cpus.len() != 1
            || !inventory.cpus[0].contains("AMD Ryzen 9 8940HX")
        {
            return Err("CPU sensors are qualified only for HP8E35/F.13/Ryzen9 8940HX".into());
        }
        #[cfg(target_arch = "x86_64")]
        #[allow(unused_unsafe)]
        // Workspace Rust1.85 requires unsafe; current Rust makes CPUID safe.
        {
            let cpu = unsafe { std::arch::x86_64::__cpuid(1) };
            let extended = unsafe { std::arch::x86_64::__cpuid(0x80000001) };
            if cpu.eax != 10882834 || extended.ebx >> 28 != 1 {
                return Err(
                    "CPU family/model/stepping/package differs from the measured reader".into(),
                );
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        return Err("CPU sensor adapter requires Windows x64".into());
        driver_identity()?;
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
                "CPU driver access requires Windows approval: {}",
                std::io::Error::last_os_error()
            ));
        }
        let driver = Handle(handle);
        let mut version = [0; 4];
        let count = ioctl(&driver, VERSION, &[], &mut version)?;
        if count != 4 || u32::from_le_bytes(version) != 0x00020200 {
            return Err("CPU driver version/response differs from the measured2.2.0 ABI".into());
        }
        if format!("{:x}", Sha256::digest(MODULE)) != CPU_MODULE_SHA256 {
            return Err("Embedded CPU module differs from its qualified signed release".into());
        }
        if ioctl(&driver, LOAD, MODULE, &mut [])
            .map_err(|e| format!("Signed CPU module load failed: {e}"))?
            != 0
        {
            return Err("Signed CPU module load returned an unexpected output".into());
        }
        let pci = unsafe { CreateMutexW(ptr::null(), 0, wide("Global\\Access_PCI").as_ptr()) };
        if pci.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Self {
            driver,
            pci: Handle(pci),
            baseline: None,
            clocks: ClockReader::default(),
        })
    }
    fn read(&self, what: Read) -> Result<u64, String> {
        read_cell(&self.driver, what)
    }
    /// Thermal control needs only Tctl. Optional clock/energy warnings must not
    /// invalidate a good temperature or wake every CPU for a fan-control tick.
    pub fn temperature_sample(&self) -> Result<CpuHardwareSample, String> {
        let sample = CpuHardwareSample {
            schema: 1,
            source: CpuHardwareSource::PawnioAmdRapl,
            temperature_kind: CpuTemperatureKind::AmdTctl,
            package_power_w: None,
            temperature_c: Some(self.temperature_c()?),
            power_interval_ms: None,
            effective_clocks: None,
            warnings: vec![],
        };
        sample.validate()?;
        Ok(sample)
    }

    fn temperature_c(&self) -> Result<f64, String> {
        let wait = unsafe { WaitForSingleObject(self.pci.0, 100) };
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            return Err("CPU temperature reader could not acquire the shared PCI mutex".into());
        }
        let _pci = PciLease(&self.pci);
        let raw = self.read(Read::Temperature)?;
        if raw > u64::from(u32::MAX) {
            return Err("CPU temperature register has unexpected high bits".into());
        }
        amd_tctl_c(raw as u32).ok_or("CPU Tctl response is unavailable or implausible".into())
    }
    pub fn sample(&mut self) -> Result<CpuHardwareSample, String> {
        let result = (|| {
            let affinity = Affinity::cpu_zero()?;
            let units = self.read(Read::EnergyUnits)?;
            if units == 0 || units == u64::MAX {
                return Err("CPU energy units are unavailable".into());
            }
            let counter = self.read(Read::PackageEnergy)? as u32;
            let now = Instant::now();
            let current = AmdPackageEnergy { counter, units };
            let previous = self.baseline.replace((current, now));
            let (package_power_w, power_interval_ms) = if let Some((old, time)) = previous {
                let elapsed = now.duration_since(time);
                let watts = amd_package_watts(old, current, elapsed);
                (watts, watts.map(|_| elapsed.as_millis() as u64))
            } else {
                (None, None)
            };
            let temperature_c = Some(self.temperature_c()?);
            let mut warnings = if previous.is_some() && package_power_w.is_none() {
                vec![
                    "CPU energy units, counter or interval changed; a fresh baseline is required"
                        .into(),
                ]
            } else {
                vec![]
            };
            let effective_clocks = match self.clocks.sample(&self.driver) {
                Ok(clocks) => clocks,
                Err(error) => {
                    warnings.push(format!(
                        "CPU effective clock measurement unavailable: {error}"
                    ));
                    None
                }
            };
            let sample = CpuHardwareSample {
                schema: 1,
                source: CpuHardwareSource::PawnioAmdRapl,
                temperature_kind: CpuTemperatureKind::AmdTctl,
                package_power_w,
                temperature_c,
                power_interval_ms,
                effective_clocks,
                warnings,
            };
            affinity.restore()?;
            sample.validate()?;
            Ok(sample)
        })();
        if result.is_err() {
            self.baseline = None;
            self.clocks = ClockReader::default();
        }
        result
    }
}
fn read_cell(driver: &Handle, what: Read) -> Result<u64, String> {
    let packet = what.packet();
    let mut response = [0; 8];
    if ioctl(driver, EXECUTE, &packet, &mut response)? != 8 {
        return Err("CPU reader returned an incomplete cell".into());
    }
    Ok(u64::from_le_bytes(response))
}
pub(super) fn ioctl(
    driver: &Handle,
    code: u32,
    input: &[u8],
    output: &mut [u8],
) -> Result<u32, String> {
    let mut count = 0;
    // Exact bounded buffers remain live until the synchronous operation returns.
    let success = unsafe {
        DeviceIoControl(
            driver.0,
            code,
            if input.is_empty() {
                ptr::null()
            } else {
                input.as_ptr().cast()
            },
            input.len() as u32,
            if output.is_empty() {
                ptr::null_mut()
            } else {
                output.as_mut_ptr().cast()
            },
            output.len() as u32,
            &mut count,
            ptr::null_mut(),
        )
    };
    if success == 0 {
        return Err(format!(
            "CPU read transport: {}",
            std::io::Error::last_os_error()
        ));
    }
    if count as usize > output.len() {
        return Err("CPU read returned an excessive byte count".into());
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_pawnio_request_has_fixed_read_names_and_exact_64_bit_cell() {
        for (kind, name, register) in [
            (Read::EnergyUnits, "ioctl_read_msr", 0xc0010299u64),
            (Read::PackageEnergy, "ioctl_read_msr", 0xc001029bu64),
            (Read::Temperature, "ioctl_read_smn", 0x59800u64),
            (Read::ActualCycles, "ioctl_read_msr", 0xc00000e8u64),
            (Read::ReferenceCycles, "ioctl_read_msr", 0xc00000e7u64),
        ] {
            let packet = kind.packet();
            assert_eq!(&packet[..name.len()], name.as_bytes());
            assert!(packet[name.len()..32].iter().all(|v| *v == 0));
            assert_eq!(
                u64::from_le_bytes(packet[32..40].try_into().unwrap()),
                register
            );
        }
        assert_eq!(LOAD, 0xa1b22084);
        assert_eq!(EXECUTE, 0xa1b22104);
        assert_eq!(VERSION, 0xa1b22184);
    }
}
