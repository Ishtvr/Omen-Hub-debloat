//! NVIDIA controls through public NVML contracts. Never calls private NVAPI IDs.
use super::nvidia::{system_library, text_buffer};
use libloading::Library;
use omen_core::{
    features::Feature,
    profiles::{Setting, Value},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{CString, c_char, c_void},
    mem::size_of,
    ptr,
};

type Handle = *mut c_void;
type StatusFn = unsafe extern "C" fn() -> i32;
type CountFn = unsafe extern "C" fn(*mut u32) -> i32;
type IndexFn = unsafe extern "C" fn(u32, *mut Handle) -> i32;
type UuidFn = unsafe extern "C" fn(*const c_char, *mut Handle) -> i32;
type DeviceTextFn = unsafe extern "C" fn(Handle, *mut c_char, u32) -> i32;
type SystemTextFn = unsafe extern "C" fn(*mut c_char, u32) -> i32;
type OffsetFn = unsafe extern "C" fn(Handle, *mut ClockOffset) -> i32;
type ThermalFn = unsafe extern "C" fn(Handle, i32, *mut u32) -> i32;

// nvmlClockOffset_v1_t: NVIDIA/go-nvml, 24-byte C layout, version in bits 24..31.
// This version convention differs from NVAPI's bits 16..31.
#[repr(C)]
#[derive(Debug, Default)]
struct ClockOffset {
    version: u32,
    clock_type: i32,
    pstate: i32,
    offset_mhz: i32,
    minimum_mhz: i32,
    maximum_mhz: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NvidiaIdentity {
    pub uuid: String,
    pub name: String,
    pub driver: String,
    pub vbios: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OffsetState {
    pub feature: Feature,
    pub current_mhz: i32,
    pub minimum_mhz: i32,
    pub maximum_mhz: i32,
}

#[derive(Debug, Serialize)]
pub struct GpuControlsReport {
    pub identity: NvidiaIdentity,
    pub core: Result<OffsetState, String>,
    pub memory: Result<OffsetState, String>,
    pub temperature_c: Option<u32>,
    pub slowdown_temperature_c: Option<u32>,
    pub clock_setter_export: bool,
}

/// Thread-confined RAII driver session. Handles and pointers never outlive its DLL.
pub(super) struct NvidiaControl {
    _library: Library,
    shutdown: StatusFn,
    handle: Handle,
    name: DeviceTextFn,
    uuid: DeviceTextFn,
    vbios: DeviceTextFn,
    driver: SystemTextFn,
    get_offsets: OffsetFn,
    set_offsets: Option<OffsetFn>,
    temperature: Option<ThermalFn>,
    threshold: Option<ThermalFn>,
}

fn status(operation: &str, result: i32) -> Result<(), String> {
    if result == 0 {
        Ok(())
    } else {
        Err(format!("NVML {operation}: status {result}"))
    }
}

fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
    // SAFETY: Each caller supplies the fixed public NVML signature for this name.
    // The session retains the loaded library for every use of the copied pointer.
    unsafe { library.get::<T>(name) }
        .map(|f| *f)
        .map_err(|e| e.to_string())
}

impl NvidiaControl {
    pub fn new(requested_uuid: Option<&str>) -> Result<Self, String> {
        let library = system_library("nvml.dll")?;
        let init: StatusFn = symbol(&library, b"nvmlInit_v2\0")?;
        let shutdown = symbol(&library, b"nvmlShutdown\0")?;
        let count: CountFn = symbol(&library, b"nvmlDeviceGetCount_v2\0")?;
        let index: IndexFn = symbol(&library, b"nvmlDeviceGetHandleByIndex_v2\0")?;
        let by_uuid: UuidFn = symbol(&library, b"nvmlDeviceGetHandleByUUID\0")?;
        let name = symbol(&library, b"nvmlDeviceGetName\0")?;
        let uuid = symbol(&library, b"nvmlDeviceGetUUID\0")?;
        let vbios = symbol(&library, b"nvmlDeviceGetVbiosVersion\0")?;
        let driver = symbol(&library, b"nvmlSystemGetDriverVersion\0")?;
        let get_offsets = symbol(&library, b"nvmlDeviceGetClockOffsets\0")?;
        let set_offsets = symbol(&library, b"nvmlDeviceSetClockOffsets\0").ok();
        let temperature = symbol(&library, b"nvmlDeviceGetTemperature\0").ok();
        let threshold = symbol(&library, b"nvmlDeviceGetTemperatureThreshold\0").ok();
        // SAFETY: Public no-argument initializer; all shutdown/error paths are paired.
        status("initialize", unsafe { init() })?;
        let mut session = Self {
            _library: library,
            shutdown,
            handle: ptr::null_mut(),
            name,
            uuid,
            vbios,
            driver,
            get_offsets,
            set_offsets,
            temperature,
            threshold,
        };
        // Construct RAII ownership before enumeration so failures also shut down.
        if let Some(id) = requested_uuid {
            let id = CString::new(id).map_err(|_| "GPU UUID contains a null byte")?;
            // SAFETY: String and writable handle live through the exact C call.
            status("GPU by UUID", unsafe {
                by_uuid(id.as_ptr(), &mut session.handle)
            })?;
        } else {
            let mut devices = 0;
            // SAFETY: Exactly sized, initialized output storage.
            status("GPU count", unsafe { count(&mut devices) })?;
            if devices != 1 {
                return Err("GPU writes require an explicit UUID when NVIDIA device count is not exactly one".into());
            }
            // SAFETY: One device was enumerated; output is valid through the call.
            status("GPU handle", unsafe { index(0, &mut session.handle) })?;
        }
        if session.handle.is_null() {
            return Err("NVML returned a null GPU handle".into());
        }
        session.identity()?;
        Ok(session)
    }

    fn device_text(&self, get: DeviceTextFn, label: &str) -> Result<String, String> {
        let mut buffer = [0; 96];
        // SAFETY: Live device, writable buffer and public length in bytes.
        status(label, unsafe {
            get(self.handle, buffer.as_mut_ptr(), buffer.len() as u32)
        })?;
        let text = text_buffer(&buffer);
        if text.is_empty() {
            return Err(format!("NVML {label} returned an empty identity"));
        }
        Ok(text)
    }

    pub fn identity(&self) -> Result<NvidiaIdentity, String> {
        let mut driver = [0; 96];
        // SAFETY: Fixed public signature, writable buffer length is exact.
        status("driver version", unsafe {
            (self.driver)(driver.as_mut_ptr(), driver.len() as u32)
        })?;
        let driver = text_buffer(&driver);
        if driver.is_empty() {
            return Err("NVML returned no driver identity".into());
        }
        Ok(NvidiaIdentity {
            uuid: self.device_text(self.uuid, "GPU UUID")?,
            name: self.device_text(self.name, "GPU name")?,
            driver,
            vbios: self.device_text(self.vbios, "VBIOS")?,
        })
    }

    pub fn offset(&self, feature: Feature) -> Result<OffsetState, String> {
        let clock_type = match feature {
            Feature::GpuCoreOffsetMhz => 0,   // NVML_CLOCK_GRAPHICS
            Feature::GpuMemoryOffsetMhz => 2, // NVML_CLOCK_MEM
            _ => return Err("Feature is not a public NVML clock offset".into()),
        };
        let mut value = ClockOffset {
            version: size_of::<ClockOffset>() as u32 | (1 << 24),
            clock_type,
            pstate: 0,
            ..Default::default()
        };
        // SAFETY: Exact versioned public C structure; P0/domain fixed by the enum.
        status("clock offset GET", unsafe {
            (self.get_offsets)(self.handle, &mut value)
        })?;
        if value.clock_type != clock_type
            || value.pstate != 0
            || value.minimum_mhz > value.maximum_mhz
            || !(value.minimum_mhz..=value.maximum_mhz).contains(&value.offset_mhz)
        {
            return Err("NVML returned inconsistent clock-offset state".into());
        }
        Ok(OffsetState {
            feature,
            current_mhz: value.offset_mhz,
            minimum_mhz: value.minimum_mhz,
            maximum_mhz: value.maximum_mhz,
        })
    }

    fn thermal(&self, get: Option<ThermalFn>, kind: i32) -> Option<u32> {
        let mut value = 0;
        // SAFETY: Optional documented function, live handle and exact integer output.
        let result = unsafe { get?(self.handle, kind, &mut value) };
        (result == 0 && (1..=150).contains(&value)).then_some(value)
    }

    pub fn report(&self) -> Result<GpuControlsReport, String> {
        Ok(GpuControlsReport {
            identity: self.identity()?,
            core: self.offset(Feature::GpuCoreOffsetMhz),
            memory: self.offset(Feature::GpuMemoryOffsetMhz),
            temperature_c: self.thermal(self.temperature, 0),
            slowdown_temperature_c: self.thermal(self.threshold, 1),
            clock_setter_export: self.set_offsets.is_some(),
        })
    }

    pub fn thermal_guard(&self, model_threshold_c: u32) -> Result<(), String> {
        let temperature = self
            .thermal(self.temperature, 0)
            .ok_or("No GPU temperature for write validation")?;
        let slowdown = self
            .thermal(self.threshold, 1)
            .ok_or("No driver thermal slowdown threshold for write validation")?;
        if temperature.saturating_add(3) >= slowdown.min(model_threshold_c) {
            return Err("GPU is too close to its model/driver thermal threshold".into());
        }
        Ok(())
    }

    // Kept module-private through the controller: no generic opcode/domain escape hatch.
    pub fn set_offset(&self, setting: &Setting) -> Result<(), String> {
        setting.validate()?;
        let original = self.offset(setting.feature)?;
        let Value::Integer(requested) = setting.value else {
            return Err("Offset needs an integer in NVML MHz".into());
        };
        let requested =
            i32::try_from(requested).map_err(|_| "Offset overflows the public C ABI")?;
        if !(original.minimum_mhz..=original.maximum_mhz).contains(&requested) {
            return Err("Offset is outside current driver limits".into());
        }
        let setter = self
            .set_offsets
            .ok_or("NVML clock-offset SET is unavailable")?;
        let mut value = ClockOffset {
            version: size_of::<ClockOffset>() as u32 | (1 << 24),
            clock_type: if setting.feature == Feature::GpuCoreOffsetMhz {
                0
            } else {
                2
            },
            pstate: 0,
            offset_mhz: requested,
            ..Default::default()
        };
        // SAFETY: Public C contract, live library/handle, validated signed range,
        // fixed clock domain/P0 and initialized reserved/output fields.
        status("clock offset SET", unsafe {
            setter(self.handle, &mut value)
        })?;
        // Some drivers complete the setting after returning. Poll boundedly; a
        // successful return code alone is never treated as a successful apply.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let actual = self.offset(setting.feature)?.current_mhz;
            if actual == requested {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "NVML {:?} SET returned success but requested {requested} MHz, read back {actual} MHz",
                    setting.feature
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

impl Drop for NvidiaControl {
    fn drop(&mut self) {
        // SAFETY: Paired with successful init, while the library is still loaded.
        unsafe {
            (self.shutdown)();
        }
    }
}

pub fn read_gpu_controls(uuid: Option<&str>) -> Result<GpuControlsReport, String> {
    NvidiaControl::new(uuid)?.report()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nvml_clock_offset_uses_its_own_exact_abi_and_version_convention() {
        assert_eq!(size_of::<ClockOffset>(), 24);
        assert_eq!(std::mem::align_of::<ClockOffset>(), 4);
        assert_eq!(std::mem::offset_of!(ClockOffset, offset_mhz), 12);
        assert_eq!(std::mem::offset_of!(ClockOffset, maximum_mhz), 20);
        assert_eq!(size_of::<ClockOffset>() as u32 | (1 << 24), 0x01000018);
    }
}
