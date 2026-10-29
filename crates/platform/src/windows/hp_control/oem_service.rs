//! User setting "Disable HP OMEN Cap": while HPOmenCap runs it rewrites CPU
//! power limits and blocks CPU-limit presets. Turning the setting on stops the
//! service and sets its start type to Disabled; turning it off restores the
//! saved start type and starts it again. Only the HP OmenCap.exe service in
//! its DriverStore package is accepted; no other service or driver.
use super::{Service, State, state};
use crate::profile_store::ProfileStore;
use omen_core::message::msg;
use serde::Serialize;
use std::{
    path::Path,
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{ERROR_SERVICE_ALREADY_RUNNING, ERROR_SERVICE_NOT_ACTIVE},
    System::{Services::*, SystemInformation::GetWindowsDirectoryW},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OemService {
    pub state: State,
    /// Start type is Disabled: Windows will not start it at sign-in.
    pub disabled: bool,
}

/// Read-only. `None`: HPOmenCap is not installed.
pub fn oem_service() -> Result<Option<OemService>, String> {
    let Some(service) = Service::open(SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG)? else {
        return Ok(None);
    };
    Ok(Some(OemService {
        state: state(service.status()?.dwCurrentState),
        disabled: service.configuration()?.start_type == SERVICE_DISABLED,
    }))
}

/// Stops and disables HPOmenCap, or restores its start type and starts it.
pub fn set_oem_service_disabled(disable: bool) -> Result<OemService, String> {
    if let Ok(Some(_)) = super::recovery_record() {
        return Err(msg("hp_service.session_active", &[]));
    }
    if !disable {
        // A pending hardware restore must finish before HP may write again.
        super::require_hardware_recovery_complete()?;
    }
    let access = SERVICE_QUERY_STATUS
        | SERVICE_QUERY_CONFIG
        | SERVICE_CHANGE_CONFIG
        | if disable { SERVICE_STOP } else { SERVICE_START };
    let service = Service::open(access)?.ok_or_else(|| msg("hp_service.absent", &[]))?;
    let config = service.configuration()?;
    if !oem_image_matches(&config.executable, &windows_directory()?)
        || config.service_type != SERVICE_WIN32_OWN_PROCESS
    {
        return Err(msg("hp_service.identity", &[]));
    }
    let store = ProfileStore::user_data()?;
    if disable {
        if config.start_type != SERVICE_DISABLED {
            store.save_hp_service_start_type(Some(config.start_type))?;
        }
        change_start_type(&service, SERVICE_DISABLED)?;
        if state(service.status()?.dwCurrentState) != State::Stopped {
            let mut status: SERVICE_STATUS = unsafe { std::mem::zeroed() };
            // SAFETY: Verified HP user-mode service handle; STOP only.
            if unsafe { ControlService(service.0, SERVICE_CONTROL_STOP, &mut status) } == 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(ERROR_SERVICE_NOT_ACTIVE as i32) {
                    return Err(msg("hp_service.stop_failed", &[&error]));
                }
            }
            wait(&service, State::Stopped)?;
        }
    } else {
        let start = restore_start_type(store.hp_service_start_type());
        change_start_type(&service, start)?;
        store.save_hp_service_start_type(None)?;
        if state(service.status()?.dwCurrentState) == State::Stopped {
            // SAFETY: Verified HP user-mode service, no service arguments.
            if unsafe { StartServiceW(service.0, 0, ptr::null()) } == 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(ERROR_SERVICE_ALREADY_RUNNING as i32) {
                    return Err(msg("hp_service.start_failed", &[&error]));
                }
            }
            wait(&service, State::Running)?;
        }
    }
    Ok(OemService {
        state: state(service.status()?.dwCurrentState),
        disabled: service.configuration()?.start_type == SERVICE_DISABLED,
    })
}

/// Only Automatic and Manual are restored; anything else becomes Automatic,
/// the start type HP installs.
fn restore_start_type(saved: Option<u32>) -> u32 {
    match saved {
        Some(start @ (SERVICE_AUTO_START | SERVICE_DEMAND_START)) => start,
        _ => SERVICE_AUTO_START,
    }
}

fn change_start_type(service: &Service, start: u32) -> Result<(), String> {
    // SAFETY: Only the start type changes; every other field is SERVICE_NO_CHANGE/null.
    if unsafe {
        ChangeServiceConfigW(
            service.0,
            SERVICE_NO_CHANGE,
            start,
            SERVICE_NO_CHANGE,
            ptr::null(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
        )
    } == 0
    {
        return Err(msg(
            "hp_service.config_failed",
            &[&std::io::Error::last_os_error()],
        ));
    }
    Ok(())
}

fn wait(service: &Service, target: State) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while state(service.status()?.dwCurrentState) != target {
        if Instant::now() >= deadline {
            return Err(msg("hp_service.timeout", &[]));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn windows_directory() -> Result<String, String> {
    let mut directory = [0u16; 32768];
    let count =
        unsafe { GetWindowsDirectoryW(directory.as_mut_ptr(), directory.len() as u32) } as usize;
    if count == 0 || count >= directory.len() {
        return Err("Windows directory is unavailable".into());
    }
    String::from_utf16(&directory[..count]).map_err(|e| e.to_string())
}

/// The configured image must be OmenCap.exe from an HP OMEN custom capability
/// driver package, without arguments. Any package version is accepted, so a
/// Windows Update of the HP component does not break the setting.
fn oem_image_matches(configured: &str, windows: &str) -> bool {
    let configured = configured.trim();
    let configured = configured
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(configured)
        .to_ascii_lowercase();
    let repository = Path::new(windows)
        .join("System32\\DriverStore\\FileRepository\\")
        .to_string_lossy()
        .to_ascii_lowercase();
    let Some(rest) = configured.strip_prefix(&repository) else {
        return false;
    };
    let Some((package, image)) = rest.split_once('\\') else {
        return false;
    };
    package.starts_with("hpomencustomcapcomp.inf_amd64_")
        && image == "x64\\omencap\\omencap.exe"
        && !configured.contains('"')
        && !configured.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_hp_omencap_image_from_its_driver_package_matches() {
        let windows = "C:\\Windows";
        let image = "C:\\Windows\\System32\\DriverStore\\FileRepository\\hpomencustomcapcomp.inf_amd64_38ec91e432e20182\\x64\\OmenCap\\OmenCap.exe";
        assert!(oem_image_matches(image, windows));
        assert!(oem_image_matches(&format!("\"{image}\""), windows));
        // A newer HP package from Windows Update.
        assert!(oem_image_matches(
            &image.replace("38ec91e432e20182", "0123456789abcdef"),
            windows
        ));
        for other in [
            format!("\"{image}\" --extra"),
            image.replace("OmenCap.exe", "Other.exe"),
            image.replace("hpomencustomcapcomp", "othervendor"),
            "C:\\Program Files\\OmenCap\\OmenCap.exe".into(),
            image.replace("x64\\OmenCap", "x64\\..\\OmenCap"),
        ] {
            assert!(!oem_image_matches(&other, windows), "{other}");
        }
    }

    #[test]
    fn restore_returns_automatic_or_manual_and_never_disabled() {
        assert_eq!(restore_start_type(None), SERVICE_AUTO_START);
        assert_eq!(
            restore_start_type(Some(SERVICE_DEMAND_START)),
            SERVICE_DEMAND_START
        );
        assert_eq!(
            restore_start_type(Some(SERVICE_DISABLED)),
            SERVICE_AUTO_START
        );
        assert_eq!(
            restore_start_type(Some(SERVICE_BOOT_START)),
            SERVICE_AUTO_START
        );
    }
}
