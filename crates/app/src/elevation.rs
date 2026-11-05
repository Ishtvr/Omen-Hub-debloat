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
    /// `true` stops and disables HPOmenCap, `false` restores it.
    HpService(bool),
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
            Self::HpService(_) => "hp_service",
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
        Action::HpService(disable) => format!(
            "hp-service-ui {} {nonce}",
            if disable { "disable" } else { "restore" }
        ),
    };
    let arguments = format!("--lang {} {arguments}", crate::i18n::language_code());
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
            // The user sees one short translated line; the complete error and
            // rollback details stay in the recorded control result (journal).
            // The marker is translated once, where the caller shows it.
            return Err(match record["result"]["Err"]["error"].as_str() {
                Some(text) if text.contains(omen_core::message::OPEN) => text.to_owned(),
                _ => omen_core::message::msg("elevation.failed_logged", &[]),
            });
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

/// Status line for a successful preset Apply: values that were left at their
/// original are named, translated; raw technical reasons stay in the journal.
pub fn applied_status(record: &serde_json::Value) -> String {
    let skipped: Vec<String> = record["result"]["Ok"]["skipped"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| {
            let reason = item["reason"].as_str().unwrap_or_default();
            if reason.contains(omen_core::message::OPEN) {
                crate::i18n::message(reason)
            } else {
                item["feature"]
                    .as_str()
                    .and_then(|key| serde_json::from_value(key.into()).ok())
                    .map_or_else(
                        || reason.to_owned(),
                        |feature: omen_core::features::Feature| {
                            t(&format!("field.{feature:?}")).to_owned()
                        },
                    )
            }
        })
        .collect();
    let mut status = if skipped.is_empty() {
        t("preset.applied").into()
    } else {
        tf("preset.applied_except", &[&skipped.join("; ")])
    };
    if record["unconfirmed"]
        .as_array()
        .is_some_and(|items| !items.is_empty())
    {
        status.push(' ');
        status.push_str(t("preset.applied_unconfirmed"));
    }
    status
}
