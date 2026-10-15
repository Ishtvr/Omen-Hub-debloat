//! Shared fixed-command UAC child bridge. No caller-defined executable/command.
use crate::native_ui::{token, wide};
use omen_platform::profile_store::ProfileStore;
use std::time::{Duration, Instant};
use windows_sys::Win32::{
    Foundation::*,
    System::Threading::*,
    UI::{Shell::*, WindowsAndMessaging::SW_HIDE},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Apply,
    Restore,
    Startup,
}
impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Restore => "restore",
            Self::Startup => "startup",
        }
    }
}
pub fn run(
    id: Option<&str>,
    action: Action,
    mut stopped: impl FnMut() -> bool,
) -> Result<serde_json::Value, String> {
    if let Some(id) = id {
        omen_core::profiles::validate_id(id)?;
    }
    let nonce = token();
    let arguments = match action {
        Action::Restore => id.map_or_else(
            || format!("profile-ui-restore {nonce}"),
            |id| format!("profile-ui-restore {id} {nonce}"),
        ),
        Action::Apply => format!(
            "profile-ui-apply {} {nonce}",
            id.ok_or("Zuerst ein Profil speichern.")?
        ),
        Action::Startup => format!(
            "profile-ui-startup-enable {} {nonce}",
            id.ok_or("Zuerst ein Profil speichern.")?
        ),
    };
    let file = wide(
        &std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy(),
    );
    let parameters = wide(&arguments);
    let verb = wide("runas");
    let mut execute: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    execute.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    execute.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    execute.lpVerb = verb.as_ptr();
    execute.lpFile = file.as_ptr();
    execute.lpParameters = parameters.as_ptr();
    execute.nShow = SW_HIDE;
    // All strings, structure and process handle have explicit bounded lifetimes.
    if unsafe { ShellExecuteExW(&mut execute) } == 0 {
        return Err(format!(
            "Windows-Freigabe nicht erfolgt: {}",
            std::io::Error::last_os_error()
        ));
    }
    if execute.hProcess.is_null() {
        return Err("Windows gab keinen Steuerprozess zurück.".into());
    }
    let result = (|| {
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            match unsafe { WaitForSingleObject(execute.hProcess, 50) } {
                WAIT_OBJECT_0 => break,
                WAIT_TIMEOUT => {}
                _ => return Err(std::io::Error::last_os_error().to_string()),
            }
            if stopped() {
                return Err("Fenster geschlossen; der Steuerprozess beendet Prüfung und Wiederherstellung selbst.".into());
            }
            if Instant::now() >= deadline {
                return Err("Steuerprozess läuft noch. Sein Ergebnis prüfen, bevor eine weitere Aktion beginnt.".into());
            }
        }
        let mut code = 1;
        if unsafe { GetExitCodeProcess(execute.hProcess, &mut code) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let record = ProfileStore::user_data()?.control_result()?;
        if record["schema"] != 1
            || record["nonce"].as_str() != Some(&nonce)
            || record["operation"].as_str() != Some(action.name())
        {
            return Err("Kein passendes Ergebnisprotokoll vorhanden.".into());
        }
        if code != 0 || record["result"].get("Err").is_some() {
            return Err(format!(
                "Steuerung fehlgeschlagen: {}",
                record["result"]["Err"]
            ));
        }
        if record["result"].get("Ok").is_none() {
            return Err("Kein verifizierter Abschluss im Ergebnis.".into());
        }
        Ok(record)
    })();
    // Never terminate a transaction in progress, including a WMI provider wait.
    unsafe {
        CloseHandle(execute.hProcess);
    }
    result
}
