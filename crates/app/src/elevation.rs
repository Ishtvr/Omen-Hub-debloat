//! Shared fixed-command UAC child bridge. No caller-defined executable/command.
use crate::{
    i18n::{t, tf},
    native_ui::{token, wide},
};
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
    Mode(omen_core::hp_mode::HpMode),
    FanService,
    Graphics(omen_core::quick_switch::GraphicsMode),
}
impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Restore => "restore",
            Self::Startup => "startup",
            Self::Mode(_) => "mode",
            Self::FanService => "fan",
            Self::Graphics(_) => "mux",
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
            id.ok_or(t("elevation.save_first"))?
        ),
        Action::Startup => format!(
            "profile-ui-startup-enable {} {nonce}",
            id.ok_or(t("elevation.save_first"))?
        ),
        Action::Mode(mode) => format!("mode-ui-apply {} {nonce}", mode.name()),
        Action::FanService => format!("fan-service-install {nonce}"),
        Action::Graphics(mode) => format!("mux-ui-apply {} {nonce}", mode.key()),
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
        return Err(tf(
            "elevation.declined",
            &[&std::io::Error::last_os_error()],
        ));
    }
    if execute.hProcess.is_null() {
        return Err(t("elevation.no_process").into());
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
                return Err(t("elevation.window_closed").into());
            }
            if Instant::now() >= deadline {
                return Err(t("elevation.still_running").into());
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
            return Err(t("elevation.no_record").into());
        }
        if code != 0 || record["result"].get("Err").is_some() {
            // Show the child's message, not the JSON envelope around it.
            let error = &record["result"]["Err"];
            let rollback: Vec<String> = error["rollback_errors"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|e| {
                    e.as_str()
                        .map_or_else(|| e.to_string(), crate::i18n::message)
                })
                .collect();
            let detail = match error["error"].as_str() {
                Some(text) if rollback.is_empty() => crate::i18n::message(text),
                Some(text) => format!("{} ({})", crate::i18n::message(text), rollback.join("; ")),
                None => error.to_string(),
            };
            return Err(tf("elevation.failed", &[&detail]));
        }
        if record["result"].get("Ok").is_none() {
            return Err(t("elevation.unverified").into());
        }
        Ok(record)
    })();
    // Never terminate a transaction in progress, including a WMI provider wait.
    unsafe {
        CloseHandle(execute.hProcess);
    }
    result
}
