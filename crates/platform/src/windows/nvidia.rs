//! Public NVIDIA telemetry only. No undocumented power-policy setters.
use libloading::Library;
use omen_core::GpuSample;
use std::{
    ffi::{c_char, c_void},
    mem::size_of,
    ptr,
};

type Handle = *mut c_void;
type StatusFn = unsafe extern "C" fn() -> i32;
type Query = unsafe extern "C" fn(u32) -> *mut c_void;
type EnumGpus = unsafe extern "C" fn(*mut Handle, *mut u32) -> i32;
type NameFn = unsafe extern "C" fn(Handle, *mut c_char) -> i32;
type ThermalFn = unsafe extern "C" fn(Handle, u32, *mut Thermal) -> i32;
type ClockFn = unsafe extern "C" fn(Handle, *mut Clocks) -> i32;
type NvmlDevice = unsafe extern "C" fn(u32, *mut Handle) -> i32;
type NvmlName = unsafe extern "C" fn(Handle, *mut c_char, u32) -> i32;
type NvmlValue = unsafe extern "C" fn(Handle, *mut u32) -> i32;
type NvmlLimits = unsafe extern "C" fn(Handle, *mut u32, *mut u32) -> i32;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Sensor {
    controller: i32,
    minimum: i32,
    maximum: i32,
    current: i32,
    target: i32,
}
#[repr(C)]
#[derive(Default)]
struct Thermal {
    version: u32,
    count: u32,
    sensors: [Sensor; 3],
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Domain {
    present: u32,
    frequency: u32,
}
#[repr(C)]
#[derive(Default)]
struct Clocks {
    version: u32,
    clock_type: u32,
    domains: [Domain; 32],
}

pub(super) fn system_library(name: &str) -> Result<Library, String> {
    // SAFETY: Only fixed driver DLL names are used. Restrict search to System32,
    // preventing DLL loading from the working directory or PATH.
    unsafe { libloading::os::windows::Library::load_with_flags(name, 0x00000800) }
        .map(Into::into)
        .map_err(|e| format!("{name}: {e}"))
}
pub(super) fn text_buffer(buffer: &[c_char]) -> String {
    let bytes: Vec<u8> = buffer
        .iter()
        .take_while(|b| **b != 0)
        .map(|b| *b as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn same_gpu_name(first: &str, second: &str) -> bool {
    first.trim_start_matches("NVIDIA ") == second.trim_start_matches("NVIDIA ")
}

fn nvapi_session_lost(status: i32) -> bool {
    matches!(status, -4 | -6 | -8 | -10)
}
fn nvml_session_lost(status: i32) -> bool {
    matches!(status, 1 | 15)
}

struct Nvapi {
    _library: Library,
    unload: StatusFn,
    gpu: Handle,
    name: String,
    thermal: ThermalFn,
    clocks: ClockFn,
}
impl Nvapi {
    fn new() -> Result<Self, String> {
        let library = system_library("nvapi64.dll")?;
        // SAFETY: Function IDs and C ABIs match NVIDIA's public nvapi headers.
        // Typed pointers never outlive the library retained in Self.
        unsafe {
            let query: Query = *library
                .get(b"nvapi_QueryInterface\0")
                .map_err(|e| e.to_string())?;
            let get = |id| {
                let p = query(id);
                if p.is_null() {
                    Err(format!("Missing NVAPI function {id:#x}"))
                } else {
                    Ok(p)
                }
            };
            let init: StatusFn = std::mem::transmute(get(0x0150e828)?);
            let unload: StatusFn = std::mem::transmute(get(0xd22bdd7e)?);
            let enumerate: EnumGpus = std::mem::transmute(get(0xe5ac921f)?);
            let name_fn: NameFn = std::mem::transmute(get(0xceee8e9f)?);
            let thermal: ThermalFn = std::mem::transmute(get(0xe3640a56)?);
            let clocks: ClockFn = std::mem::transmute(get(0xdcb616c3)?);
            let status = init();
            if status != 0 {
                return Err(format!("NVAPI initialization: {status}"));
            }
            let mut gpus = [ptr::null_mut(); 64];
            let mut count = 0;
            let status = enumerate(gpus.as_mut_ptr(), &mut count);
            if status != 0 || count == 0 || gpus[0].is_null() {
                unload();
                return Err(format!("NVAPI GPU enumeration: {status}, count {count}"));
            }
            let mut name = [0; 64];
            if name_fn(gpus[0], name.as_mut_ptr()) != 0 {
                unload();
                return Err("NVAPI GPU identity unavailable".into());
            }
            Ok(Self {
                _library: library,
                unload,
                gpu: gpus[0],
                name: text_buffer(&name),
                thermal,
                clocks,
            })
        }
    }
    fn sample(&self) -> (GpuSample, bool) {
        let mut sample = GpuSample {
            name: self.name.clone(),
            ..Default::default()
        };
        let mut thermal = Thermal {
            version: size_of::<Thermal>() as u32 | (2 << 16),
            ..Default::default()
        };
        let mut clocks = Clocks {
            version: size_of::<Clocks>() as u32 | (3 << 16),
            ..Default::default()
        };
        // SAFETY: Driver handle and function pointers are owned by this live instance.
        // Structures are repr(C), versioned and initialized, including reserved fields.
        unsafe {
            let status = (self.thermal)(self.gpu, 0, &mut thermal);
            if status == 0 {
                sample.temperature_c = thermal
                    .sensors
                    .iter()
                    .take((thermal.count as usize).min(3))
                    .find(|s| s.target == 1)
                    .map(|s| s.current);
            } else {
                sample
                    .warnings
                    .push(format!("NVAPI temperature unavailable: {status}"));
                if nvapi_session_lost(status) {
                    return (sample, true);
                }
            }
            let status = (self.clocks)(self.gpu, &mut clocks);
            if status == 0 && clocks.domains[0].present & 1 != 0 {
                sample.clock_mhz = Some(clocks.domains[0].frequency / 1000);
            } else {
                sample
                    .warnings
                    .push(format!("NVAPI clock unavailable: {status}"));
            }
            if nvapi_session_lost(status) {
                return (sample, true);
            }
        }
        (sample, false)
    }
}
impl Drop for Nvapi {
    fn drop(&mut self) {
        unsafe {
            (self.unload)();
        }
    }
}

struct Nvml {
    _library: Library,
    shutdown: StatusFn,
    gpu: Handle,
    name: String,
    power: NvmlValue,
    default_limit: NvmlValue,
    constraints: NvmlLimits,
}
impl Nvml {
    fn new(expected_name: Option<&str>) -> Result<Self, String> {
        let library = system_library("nvml.dll")?;
        // SAFETY: These are the documented NVML C signatures; DLL lifetime is retained.
        unsafe {
            let init: StatusFn = *library.get(b"nvmlInit_v2\0").map_err(|e| e.to_string())?;
            let shutdown: StatusFn = *library.get(b"nvmlShutdown\0").map_err(|e| e.to_string())?;
            let device: NvmlDevice = *library
                .get(b"nvmlDeviceGetHandleByIndex_v2\0")
                .map_err(|e| e.to_string())?;
            let name_fn: NvmlName = *library
                .get(b"nvmlDeviceGetName\0")
                .map_err(|e| e.to_string())?;
            let power = *library
                .get(b"nvmlDeviceGetPowerUsage\0")
                .map_err(|e| e.to_string())?;
            let default_limit = *library
                .get(b"nvmlDeviceGetPowerManagementDefaultLimit\0")
                .map_err(|e| e.to_string())?;
            let constraints = *library
                .get(b"nvmlDeviceGetPowerManagementLimitConstraints\0")
                .map_err(|e| e.to_string())?;
            let status = init();
            if status != 0 {
                return Err(format!("NVML initialization: {status}"));
            }
            let mut gpu = ptr::null_mut();
            let status = device(0, &mut gpu);
            if status != 0 || gpu.is_null() {
                shutdown();
                return Err(format!("NVML GPU enumeration: {status}"));
            }
            let mut name = [0; 96];
            let status = name_fn(gpu, name.as_mut_ptr(), name.len() as u32);
            if status != 0 {
                shutdown();
                return Err(format!("NVML identity: {status}"));
            }
            let name = text_buffer(&name);
            if expected_name.is_some_and(|expected| !same_gpu_name(expected, &name)) {
                shutdown();
                return Err("NVML/NVAPI GPU names differ; power readings were not combined".into());
            }
            Ok(Self {
                _library: library,
                shutdown,
                gpu,
                name,
                power,
                default_limit,
                constraints,
            })
        }
    }
    fn sample(&self, sample: &mut GpuSample) -> bool {
        if sample.name.is_empty() {
            sample.name = self.name.clone();
        }
        // SAFETY: All out parameters have the exact documented u32 layout.
        unsafe {
            for (label, getter, output) in [
                ("power", self.power, &mut sample.power_w),
                (
                    "default power limit",
                    self.default_limit,
                    &mut sample.default_limit_w,
                ),
            ] {
                let mut value = 0;
                let status = getter(self.gpu, &mut value);
                if status == 0 {
                    *output = Some(f64::from(value) / 1000.0);
                } else {
                    sample
                        .warnings
                        .push(format!("NVML {label} unavailable: {status}"));
                    if nvml_session_lost(status) {
                        return true;
                    }
                }
            }
            let (mut minimum, mut maximum) = (0, 0);
            let status = (self.constraints)(self.gpu, &mut minimum, &mut maximum);
            if status == 0 {
                sample.maximum_limit_w = Some(f64::from(maximum) / 1000.0);
            } else {
                sample
                    .warnings
                    .push(format!("NVML power constraints unavailable: {status}"));
            }
            if nvml_session_lost(status) {
                return true;
            }
        }
        false
    }
}
impl Drop for Nvml {
    fn drop(&mut self) {
        unsafe {
            (self.shutdown)();
        }
    }
}

/// Construct and use on a single worker thread. No unsafe Send/Sync implementation.
pub struct Nvidia {
    nvapi: Option<Nvapi>,
    nvml: Option<Nvml>,
    warnings: Vec<String>,
    session_lost: bool,
}
impl Default for Nvidia {
    fn default() -> Self {
        Self::new()
    }
}
impl Nvidia {
    pub fn new() -> Self {
        let mut warnings = vec![];
        let nvapi = match Nvapi::new() {
            Ok(api) => Some(api),
            Err(e) => {
                warnings.push(e);
                None
            }
        };
        let nvml = match Nvml::new(nvapi.as_ref().map(|api| api.name.as_str())) {
            Ok(api) => Some(api),
            Err(e) => {
                warnings.push(e);
                None
            }
        };
        Self {
            nvapi,
            nvml,
            warnings,
            session_lost: false,
        }
    }
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
    pub fn needs_reconnect(&self) -> bool {
        self.session_lost
    }
    pub fn sample(&mut self) -> Option<GpuSample> {
        if self.nvapi.is_none() && self.nvml.is_none() {
            self.session_lost = true;
            return None;
        }
        let (mut sample, mut lost) = self
            .nvapi
            .as_ref()
            .map(|api| api.sample())
            .unwrap_or_default();
        if let Some(nvml) = self.nvml.as_ref().filter(|_| !lost) {
            lost |= nvml.sample(&mut sample);
        }
        self.session_lost = lost;
        if lost {
            self.warnings.extend(sample.warnings.iter().cloned());
        }
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconnects_on_lost_session_but_not_unsupported_sensors() {
        assert!(nvapi_session_lost(-10));
        assert!(nvapi_session_lost(-8));
        assert!(!nvapi_session_lost(-104));
        assert!(nvml_session_lost(15));
        assert!(!nvml_session_lost(3));
    }
    #[test]
    fn public_nvapi_structures_have_exact_abi_sizes() {
        assert_eq!(size_of::<Thermal>(), 68);
        assert_eq!(size_of::<Clocks>(), 264);
    }
    #[test]
    fn refuses_to_combine_different_gpu_models() {
        assert!(same_gpu_name(
            "GeForce RTX 5070 Laptop GPU",
            "NVIDIA GeForce RTX 5070 Laptop GPU"
        ));
        assert!(same_gpu_name(
            "NVIDIA GeForce RTX 5070 Laptop GPU",
            "GeForce RTX 5070 Laptop GPU"
        ));
        assert!(!same_gpu_name(
            "GeForce RTX 5070 Laptop GPU",
            "NVIDIA GeForce RTX 5070 Ti Laptop GPU"
        ));
    }
}
