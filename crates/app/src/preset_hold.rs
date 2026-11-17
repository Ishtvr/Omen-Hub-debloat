//! "Keep preset" in the elevated dashboard process. Every few seconds the
//! applied preset's CPU limits and HP GPU flags are compared with the live
//! values; drifted ones are written again and logged to `preset-hold.log`.
//! The HP mode of a Dynamic Boost preset has no GET; its loss is seen as a
//! locked GPU limit under load. Policy and back-off live in `omen_core::hold`.
use omen_core::{
    features::Feature,
    hold::{self, Action, Hold},
    hp_mode::{self, HpMode},
    profiles::{Setting, Value},
};
use omen_platform::{ControlLease, Controller, GpuLimitProbe, profile_store::ProfileStore};
use std::{sync::Mutex, thread, time::Duration};

const INTERVAL: Duration = Duration::from_secs(5);
/// Consecutive locked readings before the mode counts as lost, so one
/// transient sample during a load change does not rewrite it.
const LOCKED_CHECKS: u8 = 2;
static NOTICE: Mutex<Option<String>> = Mutex::new(None);
pub fn take_notice() -> Option<String> {
    NOTICE.lock().ok()?.take()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Applied {
    id: String,
    digest: String,
    nonce: Option<String>,
    settings: Vec<Setting>,
}
impl Applied {
    /// Hold only settings the actual Apply verified. A completed journal also
    /// contains skipped requests for Recovery; those are not active targets.
    fn from_result(result: &serde_json::Value) -> Option<Self> {
        if result["operation"] != "apply" || !result["result"]["Ok"].is_object() {
            return None;
        }
        let applied = Self {
            id: result["profile_id"].as_str()?.to_owned(),
            digest: result["profile_digest"].as_str()?.to_owned(),
            nonce: result["nonce"].as_str().map(str::to_owned),
            settings: serde_json::from_value(result["result"]["Ok"]["verified"].clone()).ok()?,
        };
        Some(applied)
    }
    fn matches_journal(&self, id: &str, digest: &str, requested: &[Setting]) -> bool {
        self.id == id
            && self.digest == digest
            && self.settings.iter().all(|s| requested.contains(s))
    }
    fn current(&self, store: &ProfileStore) -> Result<bool, String> {
        if !store.app_settings()?.hold_preset || store.pending()? || store.fan_policy_pending()? {
            return Ok(false);
        }
        let Some((id, digest, requested)) = store.applied_settings()? else {
            return Ok(false);
        };
        if !self.matches_journal(&id, &digest, &requested) {
            return Ok(false);
        }
        let result = store.control_result()?;
        Ok(match result["operation"].as_str() {
            Some("apply") => Self::from_result(&result).as_ref() == Some(self),
            Some("mode" | "restore") => false,
            _ => true,
        })
    }
}

/// Watches the GPU release of a Dynamic Boost preset.
#[derive(Default)]
struct Release {
    /// Opened only while such a preset is held.
    probe: Option<GpuLimitProbe>,
    locked: u8,
}

/// Runs for the lifetime of the dashboard process; never blocks the UI.
pub fn spawn() {
    thread::spawn(|| {
        let mut hold = Hold::default();
        let mut controller = None;
        let mut active = None;
        let mut release = Release::default();
        loop {
            thread::sleep(INTERVAL);
            if tick(&mut hold, &mut controller, &mut active, &mut release).is_err() {
                // Driver or helper state may be stale; reopen on the next tick.
                controller = None;
            }
        }
    });
}

/// `active` is the digest of the preset the user applied last. A later mode
/// or Restore ends holding; other controls (lighting, fans, MUX) do not.
fn tick(
    hold: &mut Hold,
    controller: &mut Option<Controller>,
    active: &mut Option<Applied>,
    release: &mut Release,
) -> Result<(), String> {
    let store = ProfileStore::user_data()?;
    let stop = |hold: &mut Hold, release: &mut Release| {
        hold.clear();
        *release = Release::default();
    };
    if !store.app_settings().is_ok_and(|s| s.hold_preset) {
        stop(hold, release);
        return Ok(());
    }
    if let Ok(result) = store.control_result() {
        match result["operation"].as_str() {
            Some("apply") => *active = Applied::from_result(&result),
            Some("mode" | "restore") => *active = None,
            _ => {}
        }
    }
    let Some(applied) = active.as_ref() else {
        stop(hold, release);
        return Ok(());
    };
    // Never during a transaction or a pending recovery.
    if store.pending()? || store.fan_policy_pending()? {
        return Ok(());
    }
    let Some((id, applied_digest, settings)) = store.applied_settings()? else {
        stop(hold, release);
        return Ok(());
    };
    if !applied.matches_journal(&id, &applied_digest, &settings) {
        stop(hold, release);
        return Ok(());
    }
    // Explicit Apply of the same preset is a new attempt, too.
    hold.track(&format!(
        "{}:{}",
        applied.digest,
        applied.nonce.as_deref().unwrap_or_default()
    ));
    // An unavailable NVML must not stop keeping the CPU limits and flags.
    if keep_mode(hold, release, &store, applied).is_err() {
        *release = Release::default();
    }
    let targets = hold::targets(&applied.settings);
    if targets.is_empty() {
        return Ok(());
    }
    let controller = match controller {
        Some(controller) => controller,
        None => controller.insert(Controller::new(false, None)?),
    };
    for target in targets {
        // A new Apply/Restore may have finished after the initial snapshot.
        // Re-check the revision and live value while holding the same lease
        // as the write, so an old preset cannot overwrite a newer one.
        let Ok(_lease) = ControlLease::acquire_background() else {
            break;
        };
        if !applied.current(&store)? {
            break;
        }
        let drift = controller.hold_check(&target)?;
        match hold.decide(target.feature, drift.is_some()) {
            Action::Rewrite => {
                let result = controller.reassert(&target);
                log(&store, &id, &target, drift.as_ref(), &result);
            }
            Action::GiveUp => {
                if let Ok(mut notice) = NOTICE.lock() {
                    *notice = Some(crate::i18n::tf(
                        "hold.conflict",
                        &[&crate::i18n::t(&format!("field.{:?}", target.feature))],
                    ));
                }
                log(
                    &store,
                    &id,
                    &target,
                    drift.as_ref(),
                    &Err("given up: overwritten again after every rewrite".into()),
                );
            }
            Action::Keep | Action::Ignore => {}
        }
    }
    Ok(())
}

/// A Dynamic Boost preset switched to Unleashed (Performance on battery). A
/// locked enforced limit under load on consecutive checks means the firmware
/// dropped that mode: the mode payload and the preset's TPP are written
/// again; the GPU flags are kept with the other targets.
fn keep_mode(
    hold: &mut Hold,
    release: &mut Release,
    store: &ProfileStore,
    applied: &Applied,
) -> Result<(), String> {
    let id = &applied.id;
    let settings = &applied.settings;
    let ac = omen_platform::power_status().ac_connected == Some(true);
    let mode = match HpMode::of_settings(settings, ac) {
        Some(mode @ (HpMode::Unleashed | HpMode::Performance)) => mode,
        _ => {
            *release = Release::default();
            return Ok(());
        }
    };
    let probe = match &mut release.probe {
        Some(probe) => probe,
        None => release.probe.insert(GpuLimitProbe::new()?),
    };
    let (limit_mw, draw_mw) = probe.read()?;
    if hp_mode::gpu_release_lost(limit_mw, draw_mw) {
        release.locked = release.locked.saturating_add(1);
    } else {
        release.locked = 0;
    }
    let entry = |result: &str| {
        let _ = store.append_hold_log(&format!(
            "{} {id} HpMode limit_mw={limit_mw} draw_mw={draw_mw} mode={} {result}",
            unix_now(),
            mode.name()
        ));
    };
    match hold.decide(
        Feature::OmenPerformanceMode,
        release.locked >= LOCKED_CHECKS,
    ) {
        Action::Rewrite => {
            let Ok(_lease) = ControlLease::acquire_background() else {
                return Ok(());
            };
            if !applied.current(store)? {
                return Ok(());
            }
            release.locked = 0;
            // The transaction leaves TPP out (no readback); the stored preset
            // has it. Without one, the mode's own budget applies.
            let tpp = store
                .load(id)
                .ok()
                .filter(|p| {
                    omen_platform::profile_store::profile_digest(p)
                        .ok()
                        .as_deref()
                        == Some(applied.digest.as_str())
                })
                .and_then(|p| HpMode::shared_budget_of_profile(&p).ok().flatten())
                .unwrap_or(match mode {
                    HpMode::Unleashed => hp_mode::SHARED_BUDGET_MAX_W,
                    _ => 0,
                });
            let inventory = omen_platform::inventory();
            let result = omen_platform::write_hp_thermal_mode(&inventory, mode).and_then(|()| {
                if tpp == 0 {
                    Ok(())
                } else {
                    omen_platform::write_hp_shared_budget(&inventory, tpp)
                }
            });
            match result {
                Ok(()) => entry(&format!("rewritten tpp_w={tpp}")),
                Err(error) => entry(&format!("failed: {error}")),
            }
        }
        Action::GiveUp => entry("given up: mode lost again after every rewrite"),
        Action::Keep | Action::Ignore => {}
    }
    Ok(())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn log(
    store: &ProfileStore,
    id: &str,
    target: &Setting,
    live: Option<&Value>,
    result: &Result<(), String>,
) {
    let shown = |value: Option<&Value>| {
        value.map_or_else(
            || "?".to_owned(),
            |v| serde_json::to_string(v).unwrap_or_default(),
        )
    };
    let _ = store.append_hold_log(&format!(
        "{} {id} {:?} live={} target={} {}",
        unix_now(),
        target.feature,
        shown(live),
        shown(Some(&target.value)),
        match result {
            Ok(()) => "rewritten".to_owned(),
            Err(error) => format!("failed: {error}"),
        }
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use omen_core::{
        profiles::DeviceBinding,
        transaction::{Captured, Completion, Journal, PendingTransaction},
    };

    fn result(nonce: &str, settings: &[Setting]) -> serde_json::Value {
        serde_json::json!({"operation":"apply","profile_id":"preset-test","profile_digest":"revision","nonce":nonce,"result":{"Ok":{"verified":settings}}})
    }
    #[test]
    fn skipped_requests_never_become_hold_targets() {
        let verified = Setting {
            feature: Feature::CpuTemperatureLimitC,
            value: Value::Integer(85),
        };
        let skipped = Setting {
            feature: Feature::CpuPptLimitMw,
            value: Value::Integer(85_000),
        };
        let active = Applied::from_result(&result("a", std::slice::from_ref(&verified))).unwrap();
        assert!(active.matches_journal("preset-test", "revision", &[verified.clone(), skipped]));
        assert_eq!(hold::targets(&active.settings), [verified]);
        assert!(!active.matches_journal("preset-test", "other-revision", &active.settings));
        assert!(
            Applied::from_result(
                &serde_json::json!({"operation":"apply","result":{"Err":{"error":"failed"}}})
            )
            .is_none()
        );
    }
    #[test]
    fn old_revision_cannot_reassert_after_reapply_recovery_or_turning_hold_off() {
        let path = std::env::temp_dir().join(format!(
            "omen-hold-test-{}-{}",
            std::process::id(),
            crate::native_ui::token()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(path.clone());
        let mut store = ProfileStore::at(path).unwrap();
        let mut options = store.app_settings().unwrap();
        options.hold_preset = true;
        store.save_app_settings(&options).unwrap();
        let setting = Setting {
            feature: Feature::CpuTemperatureLimitC,
            value: Value::Integer(85),
        };
        let saved = PendingTransaction {
            schema: 1,
            profile_id: "preset-test".into(),
            profile_digest: "revision".into(),
            binding: DeviceBinding {
                manufacturer: "Test".into(),
                board: "Test".into(),
                bios: "Test".into(),
                cpu_names: vec![],
                nvidia_uuid: None,
                nvidia_driver: None,
                nvidia_vbios: None,
            },
            original: vec![Captured {
                feature: setting.feature,
                value: Value::Integer(95),
                restore: Value::Integer(95),
            }],
            requested: vec![setting.clone()],
        };
        store.begin(&saved).unwrap();
        <ProfileStore as Journal<Value>>::finish(&mut store, Completion::Applied).unwrap();
        store
            .save_control_result(&result("first", std::slice::from_ref(&setting)))
            .unwrap();
        let old = Applied::from_result(&store.control_result().unwrap()).unwrap();
        assert!(old.current(&store).unwrap());
        let pending_fan = store
            .prepare_fan_policy(
                omen_platform::profile_store::FanPolicyOwner::Library {
                    id: "fan-test".into(),
                },
                "8E35",
                "F.13",
                &[
                    omen_core::profiles::FanPoint {
                        temperature_c: 30,
                        duty_percent: 30,
                    },
                    omen_core::profiles::FanPoint {
                        temperature_c: 90,
                        duty_percent: 100,
                    },
                ],
            )
            .unwrap();
        assert!(
            !old.current(&store).unwrap(),
            "Hold pauses during a fan recovery duty"
        );
        store.restore_fan_policy(&pending_fan).unwrap();
        assert!(old.current(&store).unwrap());
        // Even reapplying the same digest invalidates the old in-flight work.
        store
            .save_control_result(&result("second", std::slice::from_ref(&setting)))
            .unwrap();
        assert!(!old.current(&store).unwrap());
        let current = Applied::from_result(&store.control_result().unwrap()).unwrap();
        options.hold_preset = false;
        store.save_app_settings(&options).unwrap();
        assert!(!current.current(&store).unwrap());
        options.hold_preset = true;
        store.save_app_settings(&options).unwrap();
        store.begin(&saved).unwrap();
        assert!(!current.current(&store).unwrap());
    }
}
