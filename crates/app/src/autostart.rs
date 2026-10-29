//! "Start with Windows": a logon task that opens the dashboard hidden in the
//! tray with administrator rights. It applies no profile; the protected logon
//! task for presets stays separate. Windows does not start programs that need
//! administrator rights from the Run key, so an entry from older builds is
//! moved to the task.
use crate::native_ui::wide;
use std::ptr;
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
    System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW},
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Omen Hub++";

/// Run entry written by builds before the logon task.
fn legacy_entry() -> bool {
    let (key, value) = (wide(RUN_KEY), wide(VALUE));
    let mut size = 0u32;
    // SAFETY: Size query only; no buffer is passed.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut size,
        ) == ERROR_SUCCESS
    }
}
fn remove_legacy_entry() -> Result<(), String> {
    let (key, value) = (wide(RUN_KEY), wide(VALUE));
    // SAFETY: Terminated UTF-16 strings.
    match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) } {
        ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
        status => Err(std::io::Error::from_raw_os_error(status as i32).to_string()),
    }
}

/// Whether Omen Hub++ starts with Windows.
pub fn enabled() -> bool {
    omen_platform::tray_task_installed() || legacy_entry()
}

/// Creates the logon task for this executable, or removes it.
pub fn set(enable: bool) -> Result<(), String> {
    if !crate::single_instance::elevated() {
        return Err(crate::i18n::t("settings.admin_required").into());
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    omen_platform::set_tray_task(enable.then_some(executable.as_path()))?;
    remove_legacy_entry()
}

/// Moves an older Run entry to the logon task once administrator rights are
/// available. Failure keeps the entry; the setting can be switched again.
pub fn migrate() {
    if crate::single_instance::elevated() && legacy_entry() {
        let _ = set(true);
    }
}
