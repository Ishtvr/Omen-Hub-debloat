//! "Start with Windows": a per-user Run entry that opens the dashboard hidden
//! in the tray. It needs no administrator rights and applies no profile; the
//! protected logon task for presets stays separate.
use crate::native_ui::wide;
use std::{path::Path, ptr};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
    System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    },
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Omen Hub++";

/// Command line stored in the Run entry.
pub fn command(executable: &Path) -> String {
    format!("\"{}\" --tray", executable.display())
}

/// Whether a Run entry for Omen Hub++ exists.
pub fn enabled() -> bool {
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

/// Adds the entry for this executable, or removes it.
pub fn set(enable: bool) -> Result<(), String> {
    let (key, value) = (wide(RUN_KEY), wide(VALUE));
    // SAFETY: Terminated UTF-16 strings and an exact byte length.
    let status = unsafe {
        if enable {
            let data = wide(&command(
                &std::env::current_exe().map_err(|e| e.to_string())?,
            ));
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        } else {
            match RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) {
                ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
                status => status,
            }
        }
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(status as i32).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_entry_quotes_the_path_and_starts_in_tray() {
        assert_eq!(
            command(Path::new(
                r"C:\Program Files\OmenHubPlusPlus\Omen Hub++.exe"
            )),
            r#""C:\Program Files\OmenHubPlusPlus\Omen Hub++.exe" --tray"#
        );
    }
}
