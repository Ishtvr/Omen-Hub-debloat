//! One-time local logon setup from this exact executable, after explicit UAC.
//! The fixed script is compiled in; no downloaded/user-supplied command runs.
use std::{os::windows::process::CommandExt, path::PathBuf};
use windows_sys::Win32::{
    System::SystemInformation::GetSystemDirectoryW, UI::Shell::IsUserAnAdmin,
};
pub fn ensure_current_installation() -> Result<(), String> {
    if omen_platform::verify_startup_installation().is_ok() {
        return Ok(());
    }
    run_installer(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/install-startup.ps1"
    )))?;
    omen_platform::verify_startup_installation().map(|_| ())
}
/// Installs or updates the logon fan service from this exact executable.
pub fn ensure_fan_service() -> Result<(), String> {
    if omen_platform::fan_service_installed() {
        return Ok(());
    }
    run_installer(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/install-fan-service.ps1"
    )))?;
    if omen_platform::fan_service_installed() {
        Ok(())
    } else {
        Err("Lüfterdienst wurde nicht eingerichtet.".into())
    }
}
fn run_installer(installer: &str) -> Result<(), String> {
    if unsafe { IsUserAnAdmin() } == 0 {
        return Err("Windows-Freigabe für den Anmeldestart erforderlich.".into());
    }
    let mut system = [0u16; 32768];
    let count = unsafe { GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32) } as usize;
    if count == 0 || count >= system.len() {
        return Err("Windows-Systemordner nicht verfügbar.".into());
    }
    let shell = PathBuf::from(String::from_utf16(&system[..count]).map_err(|e| e.to_string())?)
        .join("WindowsPowerShell/v1.0/powershell.exe");
    let source = std::env::current_exe().map_err(|e| e.to_string())?;
    let quoted = source
        .to_str()
        .ok_or("Executable path is not Unicode")?
        // PowerShell also ends single-quoted strings at U+2018..U+201B.
        .chars()
        .flat_map(|c| {
            let quote = matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}');
            std::iter::repeat_n(c, if quote { 2 } else { 1 })
        })
        .collect::<String>();
    let script = format!("& {{\n{installer}\n}} -Mode Install -SourceBinary '{quoted}'");
    let output = std::process::Command::new(shell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
        ])
        .arg(script)
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Einrichtung fehlgeschlagen: {}",
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(1200)
                .collect::<String>()
        ));
    }
    Ok(())
}
