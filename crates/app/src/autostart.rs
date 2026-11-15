//! "Start with Windows" with explicit window/tray behavior. The dashboard runs
//! with administrator rights, and Windows does not start such programs from
//! the Run key, so this is a per-user logon task with the highest run level.
//! A Run entry from older builds is read and moved to the task. It applies
//! no profile; the protected logon task for presets stays separate.
use crate::native_ui::wide;
use std::{path::Path, ptr};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
    System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW},
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Omen Hub++";

/// Command line the logon task runs.
pub fn command(executable: &Path, start_in_tray: bool) -> String {
    format!(
        "\"{}\"{}",
        executable.display(),
        if start_in_tray { " --tray" } else { "" }
    )
}

/// Splits a [`command`] into program and arguments.
fn split(command: &str) -> Option<(&str, &str)> {
    let (program, arguments) = command.strip_prefix('"')?.split_once('"')?;
    Some((program, arguments.trim()))
}

/// Run entry written by builds before the logon task.
fn legacy_entry() -> Result<Option<String>, String> {
    let (key, value) = (wide(RUN_KEY), wide(VALUE));
    let mut data = vec![0u16; 8192];
    let mut size = (data.len() * 2) as u32;
    unsafe {
        let status = RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            ptr::null_mut(),
            data.as_mut_ptr().cast(),
            &mut size,
        );
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(status as i32).to_string());
        }
        let end = data
            .iter()
            .position(|c| *c == 0)
            .ok_or("Unterminated startup entry")?;
        String::from_utf16(&data[..end])
            .map(Some)
            .map_err(|e| e.to_string())
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

/// The current start command: the logon task, else an older Run entry.
pub fn entry() -> Result<Option<String>, String> {
    if let Some((program, arguments)) = omen_platform::tray_task_command()? {
        return Ok(Some(command(
            Path::new(&program),
            arguments.trim() == "--tray",
        )));
    }
    legacy_entry()
}

/// Creates the logon task for this command, or removes it. Either way an
/// older Run entry is removed.
pub fn set_entry(entry: Option<&str>) -> Result<(), String> {
    if !crate::single_instance::elevated() {
        return Err(omen_core::message::msg("settings.admin_required", &[]));
    }
    let task = entry
        .map(|entry| split(entry).ok_or("Unsupported start command"))
        .transpose()?;
    omen_platform::set_tray_task(task.map(|(program, arguments)| (Path::new(program), arguments)))?;
    remove_legacy_entry()
}

/// Moves an older Run entry to the logon task once administrator rights are
/// available. Failure keeps the entry; Settings can save it again.
pub fn migrate() {
    if !crate::single_instance::elevated() {
        return;
    }
    if let Ok(Some(entry)) = legacy_entry()
        && matches!(omen_platform::tray_task_command(), Ok(None))
    {
        let _ = set_entry(Some(&entry));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_command_quotes_the_path_and_starts_in_tray() {
        assert_eq!(
            command(
                Path::new(r"C:\Program Files\OmenHubPlusPlus\Omen Hub++.exe"),
                true
            ),
            r#""C:\Program Files\OmenHubPlusPlus\Omen Hub++.exe" --tray"#
        );
        assert_eq!(
            command(Path::new(r"D:\Omen Hub++.exe"), false),
            r#""D:\Omen Hub++.exe""#
        );
    }

    #[test]
    fn start_command_splits_back_into_program_and_arguments() {
        for tray in [true, false] {
            let line = command(
                Path::new(r"D:\Omen Gaming Hub Rewrite\Omen Hub++.exe"),
                tray,
            );
            assert_eq!(
                split(&line),
                Some((
                    r"D:\Omen Gaming Hub Rewrite\Omen Hub++.exe",
                    if tray { "--tray" } else { "" }
                ))
            );
        }
        assert_eq!(split(r"D:\unquoted.exe --tray"), None);
    }
}
