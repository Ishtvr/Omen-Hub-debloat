//! Reversible NVIDIA Platform Controller (`nvpcf.sys`) Sub-function #9 bypass
//! for HP 8E35 / BIOS F.13 / AMD Ryzen 9 8940HX.
//!
//! Empirically verified on 25 November 2026:
//! - In Unleashed (`0x04`) and Performance (`0x31`) with Dynamic Boost (`PPAB = 1`),
//!   `nvpcf.sys` calls ACPI `NVDA0820._DSM(rev=0x200, subfunc=9)` with `CPTD = 80 W`,
//!   which writes `EC0.NSPT = 80` and `EC0.NFPT = 80`, resetting Fast PPT (PM cell 2)
//!   to 80 W and Slow PPT (PM cell 4) to `OSPT` (45..=80 W) every 1-7 seconds.
//! - `nvpcf.sys` reads two documented service registry DWORDs under
//!   `HKLM\SYSTEM\CurrentControlSet\Services\nvpcf`:
//!   `pcfKeyConfigVersion = 0x200` and `pcfKeySupportedFuncs`.
//! - Setting `pcfKeySupportedFuncs = 0x0507` (ACPI's `0x0707` with only bit 9 cleared)
//!   cleanly skips registering the Sub-function #9 CPU-TDP overwriter while keeping
//!   Sub-function #2 (Dynamic Boost cTGP +20 W, PPAB +/-15 W, TPP 55-72 W), #8 (battery)
//!   and #10 (DTTL) fully operational.
//! - Restarting `ACPI\NVDA0820\NPCF` via native `cfgmgr32.dll` (`CM_Disable_DevNode` +
//!   `CM_Enable_DevNode`) applies or reverts the setting immediately without a reboot.
use crate::profile_store::{NvpcfPptOriginal, ProfileStore};
use omen_core::{
    Inventory,
    features::Feature,
    hp_mode::HpMode,
    message::msg,
    profiles::{PerformanceProfile, Value},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::Read,
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Devices::DeviceAndDriverInstallation::{
        CM_Disable_DevNode, CM_Enable_DevNode, CM_Get_DevNode_Status, CM_Locate_DevNodeW,
        CR_SUCCESS, DN_STARTED,
    },
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
    Storage::FileSystem::FILE_SHARE_READ,
    System::{
        Registry::{
            HKEY, HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_DWORD, REG_EXPAND_SZ,
            REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
        },
        Services::SERVICE_KERNEL_DRIVER,
        SystemInformation::GetWindowsDirectoryW,
    },
};

const SERVICE_SUBKEY: &str = "SYSTEM\\CurrentControlSet\\Services\\nvpcf";
const DEVICE_INSTANCE_ID: &str = "ACPI\\NVDA0820\\NPCF";
const DRIVER_RELATIVE: &str = "System32\\drivers\\nvpcf.sys";
const VALUE_CONFIG_VERSION: &str = "pcfKeyConfigVersion";
const VALUE_SUPPORTED_FUNCS: &str = "pcfKeySupportedFuncs";
const CONFIG_VERSION_2X: u32 = 0x0000_0200;
/// HP 8E35 / F.13 SSDT `SSD4` `NVDA0820._DSM(rev=0x200, func=0)` bitmask:
/// bits 0, 1, 2, 8, 9, 10 (`0x0707`).
const ACPI_STOCK_SUPPORTED_FUNCS: u32 = 0x0000_0707;
/// Sub-function #9: CPU TDP overwriter (`CPTD = 80000 mW -> EC0.NSPT/NFPT = 80`).
const SUBFUNC_CPU_TDP_BIT: u32 = 1 << 9;
/// Clears only bit 9; keeps Dynamic Boost (#2), battery (#8) and DTTL (#10) active.
const UNLOCKED_SUPPORTED_FUNCS: u32 = ACPI_STOCK_SUPPORTED_FUNCS & !SUBFUNC_CPU_TDP_BIT;
const MAX_DRIVER_BYTES: u64 = 4 * 1024 * 1024;
const RESTART_SETTLE: Duration = Duration::from_millis(500);
const RESTART_WAIT: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockOutcome {
    AlreadyActive,
    EnabledNow,
}

#[derive(Debug, Clone, Serialize)]
pub struct NvpcfPptStatus {
    pub model_supported: bool,
    pub device_present: bool,
    pub driver_qualified: bool,
    pub driver_sha256: Option<String>,
    pub unlocked: bool,
    pub config_version: Option<u32>,
    pub supported_funcs: Option<u32>,
    pub saved_original: Option<NvpcfPptOriginal>,
    pub qualification_error: Option<String>,
}

/// Whether `profile` needs the NVPCF Sub-function #9 bypass on this power source:
/// Dynamic Boost (`Unleashed` on AC or `Performance` on battery) combined with
/// an explicit Fast PPT limit or a Slow PPT limit outside HP `0x29`'s 45..=80 W window.
pub fn preset_needs_unlock(profile: &PerformanceProfile, ac: bool) -> bool {
    if !matches!(
        HpMode::of_preset(profile, ac),
        Some(HpMode::Unleashed | HpMode::Performance)
    ) {
        return false;
    }
    profile
        .effective()
        .settings
        .iter()
        .any(|s| match s.feature {
            Feature::CpuPptLimitMw => true,
            Feature::CpuSlowPptLimitMw => match s.value {
                Value::Integer(mw) => mw % 1000 != 0 || !(45_000..=80_000).contains(&mw),
                _ => false,
            },
            _ => false,
        })
}

fn model_supported(inventory: &Inventory) -> bool {
    inventory.manufacturer == "HP"
        && inventory.board == "8E35"
        && inventory.bios == "F.13"
        && inventory.cpus.len() == 1
        && inventory.cpus[0].contains("AMD Ryzen 9 8940HX")
}

fn is_unlocked_values(state: NvpcfPptOriginal) -> bool {
    state.config_version == Some(CONFIG_VERSION_2X)
        && state.supported_funcs == Some(UNLOCKED_SUPPORTED_FUNCS)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn utf16le_bytes(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2);
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

fn contains_symbol(haystack: &[u8], name: &str) -> bool {
    contains_subslice(haystack, name.as_bytes())
        || contains_subslice(haystack, &utf16le_bytes(name))
}

/// Verifies the driver binary implements both NVPCF registry override keys
/// (stored as ASCII in `nvpcf.sys` 32.0.16.2067 and converted via
/// `RtlAnsiStringToUnicodeString`; UTF-16LE is also accepted).
fn driver_supports_registry_override(bytes: &[u8]) -> bool {
    contains_symbol(bytes, VALUE_CONFIG_VERSION) && contains_symbol(bytes, VALUE_SUPPORTED_FUNCS)
}

/// Accepts only the standard Windows driver path for `nvpcf.sys`.
fn image_path_matches(configured: &str) -> bool {
    let normalized = configured.trim().trim_matches('"').to_ascii_lowercase();
    if normalized.contains("..") || normalized.contains(' ') {
        return false;
    }
    matches!(
        normalized.as_str(),
        "\\systemroot\\system32\\drivers\\nvpcf.sys"
            | "system32\\drivers\\nvpcf.sys"
            | "%systemroot%\\system32\\drivers\\nvpcf.sys"
    )
}

struct RegKey(HKEY);
impl Drop for RegKey {
    fn drop(&mut self) {
        // SAFETY: `self.0` was opened by `RegOpenKeyExW`.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
impl RegKey {
    fn open(access: u32) -> Result<Self, String> {
        let mut handle: HKEY = ptr::null_mut();
        // SAFETY: Valid null-terminated UTF-16 subkey path and out pointer.
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                wide(SERVICE_SUBKEY).as_ptr(),
                0,
                access,
                &mut handle,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("RegOpenKeyExW(nvpcf): Win32 error {status}"));
        }
        Ok(Self(handle))
    }

    fn read_dword(&self, name: &str) -> Result<Option<u32>, String> {
        let mut value_type: u32 = 0;
        let mut data: u32 = 0;
        let mut size: u32 = size_of::<u32>() as u32;
        // SAFETY: `data` is 4-byte writable storage matching `size`.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                wide(name).as_ptr(),
                ptr::null(),
                &mut value_type,
                (&mut data as *mut u32).cast(),
                &mut size,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != ERROR_SUCCESS {
            return Err(format!("RegQueryValueExW({name}): Win32 error {status}"));
        }
        if value_type != REG_DWORD || size != size_of::<u32>() as u32 {
            return Err(format!("nvpcf registry value {name} is not a REG_DWORD"));
        }
        Ok(Some(data))
    }

    fn read_string(&self, name: &str) -> Result<Option<String>, String> {
        let mut value_type: u32 = 0;
        let mut size: u32 = 0;
        let wide_name = wide(name);
        // SAFETY: Querying required byte length with null buffer.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                wide_name.as_ptr(),
                ptr::null(),
                &mut value_type,
                ptr::null_mut(),
                &mut size,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != ERROR_SUCCESS {
            return Err(format!("RegQueryValueExW({name}): Win32 error {status}"));
        }
        if !matches!(value_type, REG_SZ | REG_EXPAND_SZ) || size == 0 || size > 4096 {
            return Err(format!(
                "nvpcf registry value {name} has unexpected type/size"
            ));
        }
        let mut units = vec![0u16; (size as usize).div_ceil(2)];
        let mut bytes = (units.len() * 2) as u32;
        // SAFETY: Buffer matches `bytes` length.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                wide_name.as_ptr(),
                ptr::null(),
                &mut value_type,
                units.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("RegQueryValueExW({name}): Win32 error {status}"));
        }
        let end = units.iter().position(|c| *c == 0).unwrap_or(units.len());
        String::from_utf16(&units[..end])
            .map(Some)
            .map_err(|e| e.to_string())
    }

    fn set_dword(&self, name: &str, value: u32) -> Result<(), String> {
        let bytes = value.to_le_bytes();
        // SAFETY: Valid handle, UTF-16 name and 4-byte REG_DWORD payload.
        let status = unsafe {
            RegSetValueExW(
                self.0,
                wide(name).as_ptr(),
                0,
                REG_DWORD,
                bytes.as_ptr(),
                bytes.len() as u32,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("RegSetValueExW({name}): Win32 error {status}"));
        }
        Ok(())
    }

    fn delete_value(&self, name: &str) -> Result<(), String> {
        // SAFETY: Valid handle and null-terminated UTF-16 value name.
        let status = unsafe { RegDeleteValueW(self.0, wide(name).as_ptr()) };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("RegDeleteValueW({name}): Win32 error {status}"))
        }
    }

    fn read_state(&self) -> Result<NvpcfPptOriginal, String> {
        Ok(NvpcfPptOriginal {
            config_version: self.read_dword(VALUE_CONFIG_VERSION)?,
            supported_funcs: self.read_dword(VALUE_SUPPORTED_FUNCS)?,
        })
    }

    fn apply_state(&self, state: NvpcfPptOriginal) -> Result<(), String> {
        match state.config_version {
            Some(v) => self.set_dword(VALUE_CONFIG_VERSION, v)?,
            None => self.delete_value(VALUE_CONFIG_VERSION)?,
        }
        match state.supported_funcs {
            Some(v) => self.set_dword(VALUE_SUPPORTED_FUNCS, v)?,
            None => self.delete_value(VALUE_SUPPORTED_FUNCS)?,
        }
        Ok(())
    }
}

fn driver_path() -> Result<PathBuf, String> {
    let mut directory = [0u16; 32768];
    // SAFETY: Buffer is writable and size matches capacity.
    let count =
        unsafe { GetWindowsDirectoryW(directory.as_mut_ptr(), directory.len() as u32) } as usize;
    if count == 0 || count >= directory.len() {
        return Err("Windows directory is unavailable".into());
    }
    Ok(
        PathBuf::from(String::from_utf16(&directory[..count]).map_err(|e| e.to_string())?)
            .join(DRIVER_RELATIVE),
    )
}

fn locate_nvpcf_devnode() -> Result<u32, String> {
    let mut dev_inst: u32 = 0;
    // SAFETY: Fixed null-terminated PnP instance ID for NVIDIA Platform Controller.
    let cr = unsafe { CM_Locate_DevNodeW(&mut dev_inst, wide(DEVICE_INSTANCE_ID).as_ptr(), 0) };
    if cr != CR_SUCCESS {
        return Err(format!(
            "ACPI\\NVDA0820\\NPCF device node not found (CR=0x{cr:x})"
        ));
    }
    Ok(dev_inst)
}

fn inspect_driver_binary() -> Result<String, String> {
    let key = RegKey::open(KEY_QUERY_VALUE)?;
    if key.read_dword("Type")? != Some(SERVICE_KERNEL_DRIVER) {
        return Err(msg("nvpcf.err.driver", &[]));
    }
    let image = key
        .read_string("ImagePath")?
        .ok_or_else(|| msg("nvpcf.err.driver", &[]))?;
    if !image_path_matches(&image) {
        return Err(msg("nvpcf.err.driver", &[]));
    }
    let path = driver_path()?;
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&path)
        .map_err(|_| msg("nvpcf.err.driver", &[]))?;
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    if len == 0 || len > MAX_DRIVER_BYTES {
        return Err(msg("nvpcf.err.driver", &[]));
    }
    let mut bytes = Vec::with_capacity(len as usize);
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 != len || !driver_supports_registry_override(&bytes) {
        return Err(msg("nvpcf.err.driver", &[]));
    }
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn qualify(inventory: &Inventory) -> Result<String, String> {
    if !model_supported(inventory) {
        return Err(msg("nvpcf.err.unsupported", &[]));
    }
    locate_nvpcf_devnode()?;
    inspect_driver_binary()
}

fn restart_nvpcf_device() -> Result<(), String> {
    let dev_inst = locate_nvpcf_devnode()?;
    // SAFETY: Fixed qualified `ACPI\NVDA0820\NPCF` devnode handle.
    let dis = unsafe { CM_Disable_DevNode(dev_inst, 0) };
    if dis != CR_SUCCESS {
        return Err(msg("nvpcf.err.restart", &[&format!("disable=0x{dis:x}")]));
    }
    std::thread::sleep(Duration::from_millis(250));
    // SAFETY: Re-enabling the same devnode immediately.
    let en = unsafe { CM_Enable_DevNode(dev_inst, 0) };
    if en != CR_SUCCESS {
        return Err(msg("nvpcf.err.restart", &[&format!("enable=0x{en:x}")]));
    }
    let deadline = Instant::now() + RESTART_WAIT;
    loop {
        let mut status: u32 = 0;
        let mut problem: u32 = 0;
        // SAFETY: Valid devnode handle and out pointers.
        let cr = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, dev_inst, 0) };
        if cr == CR_SUCCESS && (status & DN_STARTED) != 0 && problem == 0 {
            std::thread::sleep(RESTART_SETTLE);
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(msg(
                "nvpcf.err.restart",
                &[&format!(
                    "cr=0x{cr:x} status=0x{status:x} problem={problem}"
                )],
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Read-only inspection of the NVPCF Fast/Slow PPT bypass state.
pub fn inspect(inventory: &Inventory, store: &ProfileStore) -> NvpcfPptStatus {
    let supported = model_supported(inventory);
    let device_present = locate_nvpcf_devnode().is_ok();
    let driver = if supported && device_present {
        inspect_driver_binary()
    } else {
        Err(if !supported {
            msg("nvpcf.err.unsupported", &[])
        } else {
            msg("nvpcf.err.driver", &[])
        })
    };
    let state = RegKey::open(KEY_QUERY_VALUE)
        .and_then(|k| k.read_state())
        .ok();
    let unlocked = state.is_some_and(is_unlocked_values);
    NvpcfPptStatus {
        model_supported: supported,
        device_present,
        driver_qualified: driver.is_ok(),
        driver_sha256: driver.as_ref().ok().cloned(),
        unlocked,
        config_version: state.and_then(|s| s.config_version),
        supported_funcs: state.and_then(|s| s.supported_funcs),
        saved_original: store.nvpcf_ppt_original(),
        qualification_error: driver.err(),
    }
}

/// Fast read-only check whether the bypass is currently active in the registry.
pub fn is_currently_unlocked(inventory: &Inventory) -> bool {
    model_supported(inventory)
        && RegKey::open(KEY_QUERY_VALUE)
            .and_then(|k| k.read_state())
            .is_ok_and(is_unlocked_values)
}

/// Ensures `nvpcf.sys` has Sub-function #9 disabled (`pcfKeyConfigVersion=0x200`,
/// `pcfKeySupportedFuncs=0x0507`) so Fast PPT and Slow PPT persist with Dynamic
/// Boost enabled. Captures the original registry state durably before any write
/// and rolls back cleanly if restarting `ACPI\NVDA0820\NPCF` fails.
pub fn ensure_unlocked(
    inventory: &Inventory,
    store: &ProfileStore,
) -> Result<UnlockOutcome, String> {
    qualify(inventory)?;
    let key = RegKey::open(KEY_QUERY_VALUE | KEY_SET_VALUE)?;
    let current = key.read_state()?;
    if is_unlocked_values(current) {
        return Ok(UnlockOutcome::AlreadyActive);
    }
    let created_backup = store.nvpcf_ppt_original().is_none();
    if created_backup {
        store.save_nvpcf_ppt_original(Some(current))?;
    }
    let target = NvpcfPptOriginal {
        config_version: Some(CONFIG_VERSION_2X),
        supported_funcs: Some(UNLOCKED_SUPPORTED_FUNCS),
    };
    if let Err(error) = key.apply_state(target).and_then(|()| {
        drop(key);
        restart_nvpcf_device()
    }) {
        if let Ok(rollback_key) = RegKey::open(KEY_SET_VALUE) {
            let _ = rollback_key.apply_state(current);
        }
        if let Ok(dev_inst) = locate_nvpcf_devnode() {
            // SAFETY: Best-effort re-enable if disable succeeded before failure.
            unsafe {
                CM_Enable_DevNode(dev_inst, 0);
            }
        }
        if created_backup {
            let _ = store.save_nvpcf_ppt_original(None);
        }
        return Err(error);
    }
    Ok(UnlockOutcome::EnabledNow)
}

/// Restores the saved original `nvpcf` registry values and restarts
/// `ACPI\NVDA0820\NPCF` if Omen Hub++ previously enabled the bypass.
/// Returns `Ok(true)` when a saved original was restored, `Ok(false)` when none existed.
pub fn restore_if_saved(inventory: &Inventory, store: &ProfileStore) -> Result<bool, String> {
    let Some(original) = store.nvpcf_ppt_original() else {
        return Ok(false);
    };
    if !model_supported(inventory) {
        return Err(msg("nvpcf.err.unsupported", &[]));
    }
    let key = RegKey::open(KEY_QUERY_VALUE | KEY_SET_VALUE)?;
    let current = key.read_state()?;
    if current != original {
        key.apply_state(original)?;
        drop(key);
        if locate_nvpcf_devnode().is_ok() {
            restart_nvpcf_device()?;
        }
    }
    store.save_nvpcf_ppt_original(None)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omen_core::profiles::{DeviceBinding, PROFILE_SCHEMA, Setting};

    fn preset(settings: Vec<Setting>) -> PerformanceProfile {
        PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "nvpcf-test".into(),
            name: "NVPCF Test".into(),
            binding: DeviceBinding {
                manufacturer: "HP".into(),
                board: "8E35".into(),
                bios: "F.13".into(),
                cpu_names: vec!["AMD Ryzen 9 8940HX with Radeon Graphics".into()],
                nvidia_uuid: None,
                nvidia_driver: None,
                nvidia_vbios: None,
            },
            settings,
        }
    }

    #[test]
    fn unlocked_mask_clears_only_subfunction_nine() {
        assert_eq!(ACPI_STOCK_SUPPORTED_FUNCS, 0x0707);
        assert_eq!(UNLOCKED_SUPPORTED_FUNCS, 0x0507);
        assert_eq!(
            UNLOCKED_SUPPORTED_FUNCS ^ ACPI_STOCK_SUPPORTED_FUNCS,
            1 << 9
        );
        assert_ne!(UNLOCKED_SUPPORTED_FUNCS & (1 << 2), 0); // Dynamic Boost
        assert_ne!(UNLOCKED_SUPPORTED_FUNCS & (1 << 8), 0); // Battery
        assert_ne!(UNLOCKED_SUPPORTED_FUNCS & (1 << 10), 0); // DTTL
        assert_eq!(UNLOCKED_SUPPORTED_FUNCS & (1 << 9), 0); // CPU TDP overwriter disabled
        assert!(is_unlocked_values(NvpcfPptOriginal {
            config_version: Some(0x200),
            supported_funcs: Some(0x0507),
        }));
        assert!(!is_unlocked_values(NvpcfPptOriginal {
            config_version: None,
            supported_funcs: None,
        }));
    }

    #[test]
    fn preset_needs_unlock_only_for_dynamic_boost_with_fast_or_extended_slow_ppt() {
        let boosted_fast = preset(vec![
            Setting {
                feature: Feature::GpuUnleashed,
                value: Value::Boolean(true),
            },
            Setting {
                feature: Feature::CpuPptLimitMw,
                value: Value::Integer(100_000),
            },
        ]);
        assert!(preset_needs_unlock(&boosted_fast, true));
        assert!(preset_needs_unlock(&boosted_fast, false));

        let boosted_slow_in_window = preset(vec![
            Setting {
                feature: Feature::GpuUnleashed,
                value: Value::Boolean(true),
            },
            Setting {
                feature: Feature::CpuSlowPptLimitMw,
                value: Value::Integer(65_000),
            },
        ]);
        assert!(!preset_needs_unlock(&boosted_slow_in_window, true));

        let boosted_slow_below_45 = preset(vec![
            Setting {
                feature: Feature::GpuUnleashed,
                value: Value::Boolean(true),
            },
            Setting {
                feature: Feature::CpuSlowPptLimitMw,
                value: Value::Integer(30_000),
            },
        ]);
        assert!(preset_needs_unlock(&boosted_slow_below_45, true));

        let balanced_fast = preset(vec![
            Setting {
                feature: Feature::GpuUnleashed,
                value: Value::Boolean(false),
            },
            Setting {
                feature: Feature::CpuPptLimitMw,
                value: Value::Integer(30_000),
            },
        ]);
        assert!(!preset_needs_unlock(&balanced_fast, true));
    }

    #[test]
    fn driver_binary_contract_and_image_path_checks_reject_unmatched_inputs() {
        assert!(image_path_matches(
            "\\SystemRoot\\System32\\drivers\\nvpcf.sys"
        ));
        assert!(image_path_matches("System32\\drivers\\nvpcf.sys"));
        assert!(!image_path_matches("C:\\Temp\\nvpcf.sys"));
        assert!(!image_path_matches(
            "\\SystemRoot\\System32\\drivers\\..\\nvpcf.sys"
        ));

        let mut valid_u16 = Vec::new();
        valid_u16.extend_from_slice(&utf16le_bytes("pcfKeyConfigVersion"));
        valid_u16.extend_from_slice(&[0, 0, 0, 0]);
        valid_u16.extend_from_slice(&utf16le_bytes("pcfKeySupportedFuncs"));
        assert!(driver_supports_registry_override(&valid_u16));
        assert!(driver_supports_registry_override(
            b"pcfKeyConfigVersion\0pcfKeySupportedFuncs\0"
        ));
        assert!(!driver_supports_registry_override(b"pcfKeyConfigVersion\0"));
        assert!(!driver_supports_registry_override(&utf16le_bytes(
            "pcfKeySupportedFuncs"
        )));
    }
}
