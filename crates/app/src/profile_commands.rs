//! Explicit CLI entry points. Profile persistence is separate from hardware apply.
use omen_core::{
    features::{Feature, ValueKind},
    profiles::{Choice, PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
    transaction::{self, Backend},
};
use omen_platform::{
    ControlLease, Controller, RestoreState,
    profile_store::{ProfileStore, profile_digest},
};
use serde::Serialize;
use std::path::Path;

fn print<T: Serialize>(value: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn parse_value(feature: Feature, text: &str) -> Result<Value, String> {
    if feature.retired() {
        return Err(crate::i18n::t("profile.err.removed_intel").into());
    }
    match feature.descriptor().value_kind {
        ValueKind::Integer => text
            .parse::<i64>()
            .map(Value::Integer)
            .map_err(|e| e.to_string()),
        ValueKind::Boolean => text
            .parse::<bool>()
            .map(Value::Boolean)
            .map_err(|e| e.to_string()),
        ValueKind::Choice => {
            let encoded = serde_json::to_string(text).map_err(|e| e.to_string())?;
            serde_json::from_str::<Choice>(&encoded)
                .map(Value::Choice)
                .map_err(|e| e.to_string())
        }
        _ => serde_json::from_str::<Value>(text).map_err(|e| e.to_string()),
    }
}

pub fn handles(command: &str) -> bool {
    matches!(
        command,
        "gpu-capabilities"
            | "system-validate-safe"
            | "fan-validate-max"
            | "keyboard-validate-colors"
            | "keyboard-validate-lighting"
            | "keyboard-ui-read"
            | "gpu-curve-read"
            | "gpu-validate-curve"
            | "gpu-validate-memory-curve"
            | "gpu-validate"
            | "profile-catalog"
            | "profile-create"
            | "profile-list"
            | "profile-show"
            | "profile-set"
            | "profile-set-vf"
            | "profile-import"
            | "profile-plan"
            | "profile-verify-current"
            | "profile-apply"
            | "profile-apply-trace"
            | "profile-restore"
            | "profile-ui-apply"
            | "profile-ui-replace"
            | "mode-ui-replace"
            | "mode-ui-apply"
            | "hp-performance-step"
            | "cpu-smu-probe"
            | "fan-service"
            | "fan-service-stop"
            | "mux-ui-apply"
            | "hp-service-ui"
            | "hp-service-restore"
            | "fan-service-install"
            | "cpu-smu-calibrate-limits"
            | "profile-ui-restore"
            | "profile-ui-restore-keep"
            | "profile-ui-startup-enable"
            | "profile-startup-enable"
            | "profile-startup-disable"
            | "profile-startup-apply"
            | "profile-startup-status"
            | "profile-startup-check"
            | "profile-storage"
            | "profile-migrate-legacy"
            | "nvpcf-ppt-status"
            | "nvpcf-ppt-restore"
            | "gpu-tgp-status"
            | "gpu-tgp-restore"
            | "device-defaults-detect"
            | "device-defaults-ui-detect"
    )
}

/// Every fixed GPU trial keeps the full production recovery checkpoint. Even a
/// failed trial may return to that checkpoint only after complete live restore
/// and an exact, closed trial journal; a pending recovery is never hidden.
fn gpu_qualification(store: &mut ProfileStore, command: &str) -> Result<(), String> {
    let _lease = ControlLease::acquire_foreground()?;
    let checkpoint = store.checkpoint_completed_journal()?;
    let mut controller = Controller::new(true, None)?;
    let before_gpu = controller.gpu_report()?;
    let before_curve = controller.gpu_curve()?;
    let (id, digests): (&str, &[&str]) = match command {
        "gpu-validate" => (
            "driver-validation",
            &[
                "fixed-driver-validation-v3-negative",
                "fixed-driver-validation-v3-positive",
            ],
        ),
        "gpu-validate-curve" => ("vf-driver-validation", &["fixed-vf-frequency-trial-v1"]),
        "gpu-validate-memory-curve" => (
            "memory-curve-validation",
            &["fixed-memory-curve-preservation-v1"],
        ),
        _ => return Err("Unknown fixed GPU qualification".into()),
    };
    let result = match command {
        "gpu-validate" => controller
            .validate_gpu_offsets(store)
            .and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string())),
        "gpu-validate-curve" => controller
            .validate_gpu_curve(store)
            .and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string())),
        _ => controller
            .validate_memory_curve_preservation(store)
            .and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string())),
    };
    let after_gpu = controller.gpu_report();
    let after_curve = controller.gpu_curve();
    let final_check: Result<(), String> = (|| {
        let gpu = after_gpu.as_ref().map_err(Clone::clone)?;
        let curve = after_curve.as_ref().map_err(Clone::clone)?;
        if before_gpu.identity != gpu.identity
            || before_gpu.core != gpu.core
            || before_gpu.memory != gpu.memory
            || !before_curve.controls_match(curve)?
        {
            return Err("Fixed GPU trial did not restore every original GPU control".into());
        }
        Ok(())
    })();
    if final_check.is_ok() {
        let completed = store.last_transaction::<RestoreState>()?;
        if completed.profile_id == id && digests.contains(&completed.profile_digest.as_str()) {
            store.restore_completed_trial_checkpoint(checkpoint, id, &completed.profile_digest)?;
        } else if result.is_ok() {
            return Err("Completed GPU trial journal identity differs".into());
        }
    }
    print(
        &serde_json::json!({"before":before_gpu,"before_curve":before_curve,"transaction":result,
        "after":after_gpu,"after_curve":after_curve,"final_check":final_check,
        "hardware_scope":"fixed MHz offsets/curve points; complete immediate restore; no voltage/power field writes"}),
    )?;
    result?;
    final_check
}

/// Before a preset's transaction: the HP mode its Dynamic Boost field selects
/// (the Hub's full mode sequence, incl. its TPP and PL1), then the preset's
/// own TPP. The transaction then writes the preset's flags, curve, offsets,
/// CPU limits and Windows values, so they win; CPU values the preset leaves
/// out follow the mode. Neither step has a readback-based restore. The
/// caller holds the control lease.
fn apply_hp_prelude(
    profile: &omen_core::profiles::PerformanceProfile,
    inventory: &omen_core::Inventory,
) -> Result<Vec<String>, String> {
    use omen_core::hp_mode::HpMode;
    // Validate even the unconfirmed shared-budget request before any mode SET.
    // An invalid value still refuses the preset; a firmware step that does not
    // confirm is named and the remaining values are applied (user rule).
    let shared_budget = HpMode::shared_budget_of_profile(profile)?;
    let ac = omen_platform::power_status().ac_connected == Some(true);
    let mut failed = vec![];
    if let Some(mode) = HpMode::of_preset(profile, ac) {
        if let Err(error) = omen_platform::apply_hp_mode(inventory, mode) {
            failed.push(error);
        }
        // Let the firmware settle its mode tables before Windows and SMU
        // values follow (300–500 ms, battery-switch order).
        std::thread::sleep(std::time::Duration::from_millis(400));
        // The firmware keeps the mode's PL1; make it the preset's Slow PPT
        // (or cap to Fast PPT when Fast < mode default). In Unleashed, values
        // outside HP 0x29's 45..=80 W window are applied directly via SMU
        // after NVPCF Sub-function #9 is unlocked.
        if let Some(watts) = HpMode::effective_pl1_for_preset(profile, mode)
            && !(mode == HpMode::Unleashed && !(45..=80).contains(&watts))
            && let Err(error) = omen_platform::write_hp_cpu_pl1(inventory, watts)
        {
            failed.push(error);
        }
    }
    if let Some(watts) = shared_budget
        && let Err(error) = omen_platform::write_hp_shared_budget(inventory, watts)
    {
        failed.push(error);
    }
    Ok(failed)
}

fn prepare_apply(
    profile: &omen_core::profiles::PerformanceProfile,
    store: &mut ProfileStore,
    startup: bool,
    replace_pending: bool,
) -> Result<Controller, String> {
    // Recovery and preflight precede even the mode/TPP prelude.
    if replace_pending {
        recover_pending(store)?;
    } else if store.pending()? || store.fan_policy_pending()? {
        return Err(omen_core::message::msg("store.err.pending_checkpoint", &[]));
    }
    let mut controller = Controller::for_profile(profile)?;
    let checked = transaction::plan(profile, &mut controller, startup);
    if !checked.applicable {
        return Err(checked
            .blockers
            .iter()
            .map(|b| b.reason.as_str())
            .collect::<Vec<_>>()
            .join("; "));
    }
    Ok(controller)
}

/// The recorded original is restored/verified before any replacement prelude.
/// Recovery failure retains the pending journal and dispatches no new target.
fn recover_pending(store: &mut ProfileStore) -> Result<(), String> {
    if store.pending()? || store.fan_pending_direct_applied()? {
        let saved = store.last_transaction::<RestoreState>()?;
        let mut controller = Controller::for_recovery(&saved)?;
        transaction::recover(&saved, &mut controller, store)
            .map_err(|e| serde_json::to_string_pretty(&e).unwrap_or(e.error))?;
    }
    crate::profile_fan::recover_pending(store)?;
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), String> {
    let result = run_inner(args);
    // Return early errors (e.g. access/session/empty journal) to the matching UI
    // request, without overwriting a more detailed transaction/rollback result.
    if let Err(error) = &result {
        let ui = match args.first().map(String::as_str) {
            Some("keyboard-ui-read") if args.len() == 2 || args.len() == 3 => {
                Some(("keyboard_read", args[1].as_str()))
            }
            Some("profile-ui-apply" | "profile-ui-replace") if args.len() == 3 => {
                Some(("apply", args[2].as_str()))
            }
            Some("mode-ui-apply" | "mode-ui-replace") if args.len() == 3 => {
                Some(("mode", args[2].as_str()))
            }
            Some("fan-service-install") if args.len() == 2 => Some(("fan", args[1].as_str())),
            Some("mux-ui-apply") if args.len() == 3 => Some(("mux", args[2].as_str())),
            Some("hp-service-ui") if args.len() == 3 => Some(("hp_service", args[2].as_str())),
            Some("profile-ui-restore") if args.len() == 2 => Some(("restore", args[1].as_str())),
            Some("profile-ui-restore" | "profile-ui-restore-keep") if args.len() == 3 => {
                Some(("restore", args[2].as_str()))
            }
            Some("profile-ui-startup-enable") if args.len() == 3 => {
                Some(("startup", args[2].as_str()))
            }
            Some("device-defaults-ui-detect") if args.len() == 2 => {
                Some(("device_defaults", args[1].as_str()))
            }
            _ => None,
        };
        if let Some((operation, nonce)) = ui {
            validate_nonce(Some(nonce))?;
            let store = ProfileStore::user_data()?;
            if !store
                .control_result()
                .is_ok_and(|r| r["nonce"].as_str() == Some(nonce))
            {
                store.save_control_result(&serde_json::json!({"schema":1,"operation":operation,"nonce":nonce,"result":{"Err":{"error":error,"rollback_errors":[]}}}))?;
            }
        }
    }
    result
}

fn run_inner(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("system-validate-safe") if args.len() == 3 && args[2] == "--confirm-safe-writes" => {
            return crate::system_validation::run(&args[1]);
        }
        Some("cpu-smu-calibrate-limits") if args.len() == 1 => {
            // Fixed approved qualification: lower each limit once, find its
            // PM-table cell, write the previous value back.
            let _lease = ControlLease::acquire_foreground()?;
            let report = omen_platform::smu_calibrate_limits(&omen_platform::inventory())?;
            let store = ProfileStore::user_data()?;
            std::fs::write(
                store.root().join("smu-limit-calibration.json"),
                serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            return print(&report);
        }
        Some("mux-ui-apply") if args.len() == 3 => {
            validate_nonce(Some(&args[2]))?;
            let mode = omen_core::quick_switch::GraphicsMode::parse(&args[1])
                .ok_or("Unknown graphics mode")?;
            let _lease = ControlLease::acquire_foreground()?;
            let report = omen_platform::apply_graphics_mode(&omen_platform::inventory(), mode)?;
            let store = ProfileStore::user_data()?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_secs();
            let record = omen_core::quick_switch::GraphicsRecord::after_write(
                store.graphics_mode(),
                mode,
                report.previous,
                now,
                omen_platform::boot_unix_s(),
            );
            store.save_graphics_mode(&record)?;
            store.save_control_result(&serde_json::json!({"schema":1,"operation":"mux","nonce":args[2],"result":{"Ok":report}}))?;
            return print(&report);
        }
        Some("hp-service-ui") if args.len() == 3 => {
            // Fixed target: only HPOmenCap, only its start type and run state.
            validate_nonce(Some(&args[2]))?;
            let disable = match args[1].as_str() {
                "disable" => true,
                "restore" => false,
                _ => return Err("Unknown HP service action".into()),
            };
            let _lease = ControlLease::acquire_foreground()?;
            let report = omen_platform::hp_control::set_oem_service_disabled(disable)?;
            ProfileStore::user_data()?.save_control_result(&serde_json::json!({"schema":1,"operation":"hp_service","nonce":args[2],"result":{"Ok":report}}))?;
            return print(&report);
        }
        Some("hp-service-restore") if args.len() == 1 => {
            // Uninstaller: return HPOmenCap only if this app disabled it.
            if ProfileStore::user_data()?.hp_service_start_type().is_none() {
                return print(&serde_json::json!({"restored":false}));
            }
            let _lease = ControlLease::acquire_foreground()?;
            let report = omen_platform::hp_control::set_oem_service_disabled(false)?;
            return print(&serde_json::json!({"restored":true,"service":report}));
        }
        Some("fan-service-stop") if args.len() == 1 => {
            return print(
                &serde_json::json!({"stop_requested":omen_platform::stop_fan_service()?}),
            );
        }
        Some("fan-service") if args.len() == 1 => {
            // Runs for the whole logon session from the protected task copy.
            let store = ProfileStore::user_data()?;
            let status = ProfileStore::user_data()?;
            return omen_platform::run_fan_service(
                &omen_platform::inventory(),
                || store.fan_mode(),
                || store.active_fan_curve(),
                || store.ccd_selection(),
                |s| {
                    let _ = status.save_fan_status(s);
                },
            );
        }
        Some("fan-service-install") if args.len() == 2 => {
            validate_nonce(Some(&args[1]))?;
            crate::startup_setup::ensure_fan_service()?;
            ProfileStore::user_data()?.save_control_result(&serde_json::json!({"schema":1,"operation":"fan","nonce":args[1],"result":{"Ok":true}}))?;
            return print(&serde_json::json!({"fan_service":"installed"}));
        }
        Some("cpu-smu-probe") if args.len() == 1 => {
            // Read-only: SMU version, PM table head, PBO scalar and CO margins.
            let report = omen_platform::smu_probe(&omen_platform::inventory())?;
            let store = ProfileStore::user_data()?;
            std::fs::write(
                store.root().join("smu-probe.json"),
                serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            return print(&report);
        }
        Some("nvpcf-ppt-status") if args.len() == 1 => {
            let store = ProfileStore::user_data()?;
            return print(&omen_platform::nvpcf_ppt::inspect(
                &omen_platform::inventory(),
                &store,
            ));
        }
        Some("nvpcf-ppt-restore") if args.len() == 1 => {
            let _lease = ControlLease::acquire_foreground()?;
            let store = ProfileStore::user_data()?;
            let restored =
                omen_platform::nvpcf_ppt::restore_if_saved(&omen_platform::inventory(), &store)?;
            return print(&serde_json::json!({"restored": restored}));
        }
        Some("gpu-tgp-status") if args.len() == 1 => {
            let store = ProfileStore::user_data()?;
            return print(&omen_platform::vbios_tgp::inspect(
                &omen_platform::inventory(),
                &store,
            ));
        }
        Some("gpu-tgp-restore") if args.len() == 1 => {
            let _lease = ControlLease::acquire_foreground()?;
            let store = ProfileStore::user_data()?;
            let restored = omen_platform::vbios_tgp::restore_if_active(
                &omen_platform::inventory(),
                &store,
                omen_platform::read_nvml_power_limits,
            )?;
            return print(&serde_json::json!({"restored": restored}));
        }
        Some("device-defaults-detect") if args.len() == 1 => {
            let store = ProfileStore::user_data()?;
            let record =
                omen_platform::discover_device_defaults(&omen_platform::inventory(), &store)?;
            return print(&record);
        }
        Some("device-defaults-ui-detect") if args.len() == 2 => {
            validate_nonce(Some(&args[1]))?;
            let store = ProfileStore::user_data()?;
            let record =
                omen_platform::discover_device_defaults(&omen_platform::inventory(), &store)?;
            store.save_control_result(&serde_json::json!({
                "schema": 1,
                "operation": "device_defaults",
                "nonce": args[1],
                "result": { "Ok": record }
            }))?;
            return print(&record);
        }
        Some("mode-ui-apply" | "mode-ui-replace") if args.len() == 3 => {
            validate_nonce(Some(&args[2]))?;
            let mode = omen_core::hp_mode::HpMode::parse(&args[1])?;
            let _lease = ControlLease::acquire_foreground()?;
            let mut user_store = ProfileStore::user_data()?;
            if args[0] == "mode-ui-replace" {
                recover_pending(&mut user_store)?;
            } else if user_store.pending()? || user_store.fan_policy_pending()? {
                return Err(omen_core::message::msg("store.err.pending_checkpoint", &[]));
            }
            let inventory = omen_platform::inventory();
            let _ = omen_platform::nvpcf_ppt::restore_if_saved(&inventory, &user_store);
            let _ = omen_platform::vbios_tgp::restore_if_active(
                &inventory,
                &user_store,
                omen_platform::read_nvml_power_limits,
            );
            let report = omen_platform::apply_hp_mode(&inventory, mode)?;
            ProfileStore::user_data()?.save_control_result(&serde_json::json!({"schema":1,"operation":"mode","nonce":args[2],"result":{"Ok":report}}))?;
            return print(&report);
        }
        Some("hp-performance-step") if args.len() == 2 => {
            // Measurement only: the Hub's Performance slider step L5/L6 thermal
            // byte. Run `mode-ui-apply performance` first for flags, TPP and PL1.
            let _lease = ControlLease::acquire_foreground()?;
            omen_platform::write_hp_performance_step(&omen_platform::inventory(), &args[1])?;
            ProfileStore::user_data()?.save_control_result(&serde_json::json!({"schema":1,"operation":"mode","nonce":null,"result":{"Ok":{"performance_step":args[1]}}}))?;
            return print(&serde_json::json!({"performance_step":args[1],"readback":false}));
        }
        Some("keyboard-ui-read") if args.len() == 2 || args.len() == 3 => {
            validate_nonce(Some(&args[1]))?;
            let store = ProfileStore::user_data()?;
            let mut controller = if let Some(id) = args.get(2) {
                Controller::for_profile(&store.load(id)?)?
            } else {
                Controller::new(false, None)?
            };
            let state = controller.keyboard_state()?;
            let plan = args
                .get(2)
                .map(|id| {
                    store
                        .load(id)
                        .map(|p| transaction::plan(&p, &mut controller, false))
                })
                .transpose()?;
            let result = serde_json::json!({"state":state,"plan":plan,"binding":controller.context()?.binding});
            store.save_control_result(&serde_json::json!({"schema":1,"operation":"keyboard_read","nonce":args[1],"result":{"Ok":result}}))?;
            return print(&result);
        }
        Some("gpu-curve-read") if args.len() == 1 => {
            let mut controller = Controller::new(true, None)?;
            return print(&controller.gpu_curve()?);
        }
        Some("profile-storage") if args.len() == 1 => {
            return print(&serde_json::json!({"directory":ProfileStore::user_data()?.root()}));
        }
        Some("profile-migrate-legacy") if args.len() == 1 => {
            let _lease = ControlLease::acquire_foreground()?;
            let store = ProfileStore::user_data()?;
            return print(
                &serde_json::json!({"copied_profiles":store.migrate_legacy()?,"directory":store.root(),"startup_enabled":false,"hardware_applied":false}),
            );
        }
        Some("profile-catalog") if args.len() == 1 => {
            return print(
                &Feature::all()
                    .iter()
                    .map(|f| f.descriptor())
                    .collect::<Vec<_>>(),
            );
        }
        Some("gpu-capabilities") if args.len() <= 2 => {
            let mut controller = Controller::new(true, args.get(1).map(String::as_str))?;
            return print(&serde_json::json!({
                "gpu": controller.gpu_report()?, "context": controller.context()?,
                "features": Feature::all().iter().map(|f| controller.capability(*f)).collect::<Vec<_>>()
            }));
        }
        _ => {}
    }
    let mut store = ProfileStore::user_data()?;
    match args.first().map(String::as_str) {
        Some("fan-validate-max") if args.len() == 1 => {
            let _lease = ControlLease::acquire_foreground()?;
            if store.pending()? {
                return Err("Pending recovery blocks the fixed max-fan trial".into());
            }
            let mut controller = Controller::new(false, None)?;
            let before = controller.fan_report()?;
            let checkpoint = store.checkpoint_completed_journal()?;
            let result = controller.validate_max_fan(&mut store);
            let after = controller.fan_report();
            let final_check = after.as_ref().map_err(|e| e.clone()).and_then(|r| {
                if r.max_fan == before.max_fan
                    && r.table_sha256 == before.table_sha256
                    && r.topology == before.topology
                {
                    Ok(())
                } else {
                    Err("Final max-fan original/layout differs".into())
                }
            });
            if result.is_ok() && final_check.is_ok() {
                let completed = store.last_transaction::<RestoreState>()?;
                store.restore_completed_trial_checkpoint(
                    checkpoint,
                    "max-fan-validation",
                    &completed.profile_digest,
                )?;
            }
            print(
                &serde_json::json!({"before":before,"transaction":result,"observations":controller.fan_validation_observations(),"after":after,"final_check":final_check,"scope":"Fixed maximum cooling ON only, then original flag restore; no software fan levels, performance, power or voltage SET"}),
            )?;
            result.map(|_| ())?;
            final_check
        }
        Some("keyboard-validate-colors" | "keyboard-validate-lighting") if args.len() == 1 => {
            let _lease = ControlLease::acquire_foreground()?;
            if store.pending()? {
                return Err("Pending recovery blocks the fixed keyboard color trial".into());
            }
            let mut controller = Controller::new(false, None)?;
            let before = controller.keyboard_state()?;
            let checkpoint = store.checkpoint_completed_journal()?;
            let mode = args[0] == "keyboard-validate-lighting";
            let result = if mode {
                controller.validate_keyboard_lighting(&mut store)
            } else {
                controller.validate_keyboard_colors(&mut store)
            };
            let after = controller.keyboard_state();
            let final_check = after
                .as_ref()
                .map_err(|e| format!("Final keyboard GET failed: {e}"))
                .and_then(|state| {
                    if *state == before {
                        Ok(())
                    } else {
                        Err("Keyboard state changed after the trial restore".into())
                    }
                });
            if result.is_ok() && final_check.is_ok() {
                let completed = store.last_transaction::<RestoreState>()?;
                store.restore_completed_trial_checkpoint(
                    checkpoint,
                    if mode {
                        "keyboard-lighting-validation"
                    } else {
                        "keyboard-color-validation"
                    },
                    &completed.profile_digest,
                )?;
            }
            print(
                &serde_json::json!({"before":before,"transaction":result,"after":after,"final_check":final_check,"hardware_scope":if mode {"one zone1 RGB byte and exact64/E4 static/preset switch; immediate full restore; no power/voltage writes"}else{"one zone1 RGB byte; immediate full restore; no brightness/power/voltage writes"}}),
            )?;
            result.map(|_| ())?;
            final_check
        }
        Some(command @ ("gpu-validate" | "gpu-validate-curve" | "gpu-validate-memory-curve"))
            if args.len() == 1 =>
        {
            gpu_qualification(&mut store, command)
        }
        Some("profile-create") if args.len() == 3 => {
            let mut controller = Controller::new(true, None)?;
            let profile = PerformanceProfile {
                schema: PROFILE_SCHEMA,
                id: args[1].clone(),
                name: args[2].clone(),
                binding: controller.context()?.binding,
                settings: vec![],
            };
            let path = store.save(&profile)?;
            print(&serde_json::json!({"saved":path,"hardware_applied":false,"profile":profile}))
        }
        Some("profile-list") if args.len() == 1 => print(&store.list()?),
        Some("profile-show") if args.len() == 2 => print(&store.load(&args[1])?),
        Some("profile-import") if args.len() == 2 => {
            let path = store.import(Path::new(&args[1]))?;
            print(&serde_json::json!({"saved":path,"hardware_applied":false}))
        }
        Some("profile-set") if args.len() == 4 => {
            let mut profile = store.load(&args[1])?;
            let encoded = serde_json::to_string(&args[2]).map_err(|e| e.to_string())?;
            let feature = serde_json::from_str::<Feature>(&encoded).map_err(|e| e.to_string())?;
            let setting = Setting {
                feature,
                value: parse_value(feature, &args[3])?,
            };
            setting.validate()?;
            profile.settings.retain(|s| s.feature != feature);
            profile.settings.push(setting);
            let path = store.save(&profile)?;
            print(&serde_json::json!({"saved":path,"hardware_applied":false,"profile":profile}))
        }
        Some("profile-set-vf") if args.len() == 4 => {
            let mut profile = store.load(&args[1])?;
            let mut controller = Controller::new(true, profile.binding.nvidia_uuid.as_deref())?;
            if !profile.binding.matches(&controller.context()?.binding) {
                return Err(
                    "Profile board/BIOS/GPU binding differs from the current device".into(),
                );
            }
            let voltage = args[2]
                .parse::<u32>()
                .map_err(|_| "Voltage anchor must be integer mV")?;
            let frequency = args[3]
                .parse::<u32>()
                .map_err(|_| "Frequency target must be integer MHz")?;
            let value = omen_core::vf::plateau(&controller.gpu_curve()?, voltage, frequency)?;
            profile
                .settings
                .retain(|s| !matches!(s.feature, Feature::GpuCoreOffsetMhz | Feature::GpuVfCurve));
            profile.settings.push(Setting {
                feature: Feature::GpuVfCurve,
                value,
            });
            let path = store.save(&profile)?;
            print(&serde_json::json!({"saved":path,"hardware_applied":false,"profile":profile}))
        }
        Some("profile-plan") if args.len() == 2 => {
            let profile = store.load(&args[1])?;
            let parts = omen_core::fan_profile::split(&profile)?;
            let mut controller = Controller::for_profile(&parts.direct)?;
            let mut plan =
                serde_json::to_value(transaction::plan(&parts.direct, &mut controller, false))
                    .map_err(|e| e.to_string())?;
            if let Some(shared) = parts.shared {
                plan = serde_json::json!({"direct_applicable":plan["applicable"],"direct_plan":plan,"cooling_request":shared,"cooling_request_readiness":"checked_on_apply","cooling_hardware_verified":false});
            }
            print(&plan)
        }
        Some("profile-verify-current") if args.len() == 2 => {
            let _lease = ControlLease::acquire_foreground()?;
            let profile = store.load(&args[1])?;
            // The journal must describe a completed application of this exact
            // revision. Verification only GETs current targets; it never applies
            // a profile, repairs a mismatch, or grants a startup approval.
            let parts = omen_core::fan_profile::split(&profile)?;
            if store.pending()? || store.fan_policy_pending()? {
                return Err(omen_core::message::msg("store.err.pending_checkpoint", &[]));
            }
            let Some((id, digest, requested)) = store.applied_settings()? else {
                return Err(omen_core::message::msg("profile.err.other_profile", &[]));
            };
            if id != profile.id
                || digest != profile_digest(&profile)?
                || requested != parts.direct.effective().settings
            {
                return Err(omen_core::message::msg("profile.err.other_profile", &[]));
            }
            let mut controller = Controller::for_profile(&parts.direct)?;
            controller.verify_applied(&store.last_transaction::<RestoreState>()?)?;
            crate::profile_fan::verify_request(&store, &profile)?;
            print(
                &serde_json::json!({"schema":1,"profile_id":profile.id,"profile_digest":profile_digest(&profile)?,"verified":parts.shared.is_none(),"direct_verified":true,"cooling_request_matches":parts.shared.is_some(),"cooling_hardware_verified":false,"hardware_applied":false}),
            )
        }
        Some(
            "profile-apply" | "profile-apply-trace" | "profile-ui-apply" | "profile-ui-replace",
        ) if args.len() == 2
            || (matches!(args[0].as_str(), "profile-ui-apply" | "profile-ui-replace")
                && args.len() == 3) =>
        {
            let _lease = ControlLease::acquire_foreground()?;
            let nonce = if matches!(args[0].as_str(), "profile-ui-apply" | "profile-ui-replace") {
                args.get(2).map(String::as_str)
            } else {
                None
            };
            validate_nonce(nonce)?;
            let profile = store.load(&args[1])?;
            let max_tgp_w = store.gpu_tgp_max_w()?;
            let requested_tgp_w = profile.gpu_dynamic_boost_limit_w_max(max_tgp_w)?;
            let digest = profile_digest(&profile)?;
            let parts = omen_core::fan_profile::split(&profile)?;
            if profile
                .settings
                .iter()
                .any(|s| s.feature == Feature::CpuCcdAffinity)
                && parts.shared.is_none()
            {
                // The background service pins programs; install or update it
                // first so the saved selection takes effect.
                crate::startup_setup::ensure_fan_service()?;
            }
            let mut controller = prepare_apply(
                &parts.direct,
                &mut store,
                false,
                args[0] == "profile-ui-replace",
            )?;
            let tgp_outcome = if requested_tgp_w.is_some() || store.vbios_tgp_state().is_some() {
                // Close any live NVML/NVAPI handles before a potential dGPU PnP restart.
                let inventory = controller.inventory().clone();
                drop(controller);
                let outcome = omen_platform::vbios_tgp::apply_for_profile(
                    &inventory,
                    &store,
                    &profile,
                    omen_platform::read_nvml_power_limits,
                )?;
                controller = Controller::for_profile(&parts.direct)?;
                outcome
            } else {
                omen_platform::vbios_tgp::ApplyOutcome::Unchanged
            };
            let ac = omen_platform::power_status().ac_connected == Some(true);
            if omen_platform::nvpcf_ppt::preset_needs_unlock(&profile, ac) {
                omen_platform::nvpcf_ppt::ensure_unlocked(controller.inventory(), &store)?;
                controller.set_nvpcf_ppt_unlocked(true);
            }
            let cooling_request =
                crate::profile_fan::prepare(&profile, &store, controller.inventory())?;
            let prelude_failed = match apply_hp_prelude(&profile, controller.inventory()) {
                Ok(failed) => failed,
                Err(error) => {
                    return Err(crate::profile_fan::prelude_failed(
                        &store,
                        cooling_request.as_ref(),
                        error,
                    ));
                }
            };
            let tracing = args[0] == "profile-apply-trace";
            if tracing {
                controller.trace_cpu_readbacks();
            }
            // Logged only: preset values overwrite the HP service's (user rule).
            let notes: Vec<String> =
                omen_platform::hp_control::check_cpu_power_settings(&profile.effective().settings)
                    .err()
                    .into_iter()
                    .collect();
            let result =
                transaction::apply(&parts.direct, &digest, &mut controller, &mut store, false);
            let (result, cooling) = crate::profile_fan::finish(
                &mut store,
                &mut controller,
                &profile,
                cooling_request.as_ref(),
                result,
            );
            // TPP has a SET contract but no exact GET. Do not include it in
            // the verified transaction or imply a fully confirmed firmware state.
            let unconfirmed: Vec<_> = profile
                .settings
                .iter()
                .filter(|s| s.feature == Feature::HpSharedPowerMw)
                .collect();
            // The HP thermal mode (0x1A) has no GET either: requested only.
            let mode = omen_core::hp_mode::HpMode::of_settings(
                &profile.settings,
                omen_platform::power_status().ac_connected == Some(true),
            )
            .map(|mode| mode.name());
            // Settings a documented precedence rule replaced (never written).
            let effective = profile.effective().settings;
            let overridden: Vec<_> = profile
                .settings
                .iter()
                .filter(|s| {
                    !matches!(
                        s.feature,
                        Feature::HpSharedPowerMw | Feature::GpuDynamicBoostLimitMw
                    ) && !effective.iter().any(|e| e.feature == s.feature)
                })
                .map(|s| s.feature)
                .collect();
            // Keep preset holds only an Apply made since the current boot.
            let boot = omen_platform::boot_unix_s();
            let gpu_tgp_w = tgp_outcome.active_watts();
            let gpu_tgp_restored =
                tgp_outcome == omen_platform::vbios_tgp::ApplyOutcome::RestoredToStock;
            let gpu_restarted = tgp_outcome.gpu_restarted();
            store.save_control_result(&serde_json::json!({"schema":1,"operation":"apply","nonce":nonce,"profile_id":profile.id,"profile_digest":digest,"notes":notes,"unconfirmed":unconfirmed,"mode_requested":mode,"overridden":overridden,"prelude_failed":prelude_failed,"gpu_tgp_w":gpu_tgp_w,"gpu_tgp_restored":gpu_tgp_restored,"gpu_restarted":gpu_restarted,"boot_unix_s":boot,"cooling":cooling,"result":result}))?;
            if tracing {
                print(
                    &serde_json::json!({"result":result,"cpu_readbacks":controller.cpu_readback_observations()}),
                )?;
            } else {
                print(&result)?;
            }
            result
                .map(|_| ())
                .map_err(|e| serde_json::to_string_pretty(&e).unwrap_or(e.error))
        }
        Some("profile-restore" | "profile-ui-restore" | "profile-ui-restore-keep")
            if args.len() == 1
                || (args[0] == "profile-ui-restore" && (args.len() == 2 || args.len() == 3))
                || (args[0] == "profile-ui-restore-keep" && args.len() == 3) =>
        {
            let _lease = ControlLease::acquire_foreground()?;
            let nonce = if args.len() == 3 {
                args.get(2)
            } else {
                args.get(1)
            }
            .map(String::as_str);
            validate_nonce(nonce)?;
            if !store.pending()?
                && store.fan_policy_pending()?
                && !store.fan_pending_direct_applied()?
            {
                if args.len()==3 && !store.fan_policy_request()?.is_some_and(|r|matches!(&r.owner,omen_platform::profile_store::FanPolicyOwner::Profile{id,..} if id==&args[1])) {
                    return Err(omen_core::message::msg("profile.err.other_profile",&[]));
                }
                crate::profile_fan::recover_pending(&store)?;
                store.save_control_result(&serde_json::json!({"schema":1,"operation":"restore","nonce":nonce,"result":{"Ok":null},"cooling_preferences_restored":true,"direct_restored":false}))?;
                return print(
                    &serde_json::json!({"cooling_preferences_restored":true,"hardware_handover_confirmed":false}),
                );
            }
            let saved = store.last_transaction::<RestoreState>()?;
            if args.len() == 3 && saved.profile_id != args[1] {
                return Err(omen_core::message::msg("profile.err.other_profile", &[]));
            }
            // Disable further automatic application before explicit recovery.
            // The automatic battery return only undoes its own battery preset
            // and keeps the user's sign-in approval.
            if args[0] != "profile-ui-restore-keep" {
                store.disable_startup()?;
            }
            let mut controller = Controller::for_recovery(&saved)?;
            let inventory = controller.inventory().clone();
            let mut direct_restored = false;
            let mut cooling_restored = false;
            let mut nvpcf_restored = false;
            let mut gpu_tgp_restored = false;
            let keep_startup = args[0] == "profile-ui-restore-keep";
            let result = transaction::recover(&saved, &mut controller, &mut store).and_then(|()| {
                direct_restored = true;
                crate::profile_fan::restore_for(&store, &saved.profile_id, &saved.profile_digest)
                    .map(|value| {
                        cooling_restored = value;
                    })
                    .map_err(|error| transaction::ApplyFailure {
                        error: omen_core::message::msg(
                            "fan_profile.err.restore_after_direct",
                            &[&error],
                        ),
                        rollback_errors: vec![],
                    })
            });
            drop(controller);
            let result = result.and_then(|()| {
                if !keep_startup {
                    nvpcf_restored = omen_platform::nvpcf_ppt::restore_if_saved(&inventory, &store)
                        .map_err(|error| transaction::ApplyFailure {
                            error,
                            rollback_errors: vec![],
                        })?;
                    gpu_tgp_restored = omen_platform::vbios_tgp::restore_if_active(
                        &inventory,
                        &store,
                        omen_platform::read_nvml_power_limits,
                    )
                    .map_err(|error| transaction::ApplyFailure {
                        error,
                        rollback_errors: vec![],
                    })?;
                }
                Ok(())
            });
            store.save_control_result(&serde_json::json!({"schema":1,"operation":"restore","nonce":nonce,"result":result,"direct_restored":direct_restored,"cooling_preferences_restored":cooling_restored,"nvpcf_restored":nvpcf_restored,"gpu_tgp_restored":gpu_tgp_restored,"cooling_hardware_verified":false}))?;
            result.map_err(|e| serde_json::to_string_pretty(&e).unwrap_or(e.error))?;
            let gpu_after = if saved.requested.iter().any(|s| s.feature.needs_nvidia()) {
                Some(Controller::for_recovery(&saved)?.gpu_report()?)
            } else {
                None
            };
            print(
                &serde_json::json!({"restored":true,"profile_id":saved.profile_id,"gpu_tgp_restored":gpu_tgp_restored,"nvpcf_restored":nvpcf_restored,"gpu":gpu_after}),
            )
        }
        Some("profile-startup-enable" | "profile-ui-startup-enable")
            if args.len() == 2 || (args[0] == "profile-ui-startup-enable" && args.len() == 3) =>
        {
            let _lease = ControlLease::acquire_foreground()?;
            let nonce = args.get(2).map(String::as_str);
            validate_nonce(nonce)?;
            let profile = store.load(&args[1])?;
            omen_core::fan_profile::startup_allowed(&profile, true)?;
            let direct = omen_core::fan_profile::direct_part(&profile)?;
            let mut controller = Controller::for_profile(&direct)?;
            let plan = transaction::plan(&direct, &mut controller, true);
            if !plan.applicable {
                return Err(serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?);
            }
            store.startup_eligibility(&args[1])?;
            controller.verify_applied(&store.last_transaction::<RestoreState>()?)?;
            if args[0] == "profile-ui-startup-enable" {
                crate::startup_setup::ensure_current_installation()?;
            } else {
                omen_platform::verify_startup_installation()?;
            }
            omen_platform::authorize_startup_profile(&profile)?;
            store.enable_startup(&args[1])?;
            store.save_control_result(&serde_json::json!({"schema":1,"operation":"startup","nonce":nonce,"result":{"Ok":store.startup()?}}))?;
            print(&store.startup()?)
        }
        Some("profile-startup-disable") if args.len() == 1 => {
            let _lease = ControlLease::acquire_foreground()?;
            store.disable_startup()?;
            print(&store.startup()?)
        }
        Some("profile-startup-apply") if args.len() == 1 => {
            let _lease = ControlLease::acquire_foreground()?;
            let selection_at_start = store.startup()?;
            // Approval ends for a changed preset, device or binding; a firmware
            // step or value that fails after those checks does not revoke it.
            let mut device_checked = false;
            // Signed in on battery: a preset with its own battery behaviour is
            // not applied. The running dashboard applies that behaviour and
            // returns to this preset when the charger is connected.
            let mut skipped_on_battery = false;
            // The linked curve's helper request, or why it was not made.
            let mut cooling = None;
            let mut cooling_error = None;
            let result = (|| {
                let Some(profile) = store.startup_profile()? else {
                    return Ok(None);
                };
                let selection = omen_core::battery::Selection::Preset(profile.id.clone());
                let ac = omen_platform::power_status().ac_connected;
                skipped_on_battery =
                    ac == Some(false) && store.battery_selection_for(Some(&selection)).is_some();
                // Either way this preset is the one in use on AC from now on,
                // so its battery behaviour and the return apply to it. A new
                // boot also ends the battery session of the earlier one.
                let mut state = store.battery_state().at_boot(omen_platform::boot_unix_s());
                state.ac_selection = Some(selection);
                if skipped_on_battery {
                    store.save_battery_state(&state)?;
                    return Ok(None);
                }
                omen_core::fan_profile::startup_allowed(&profile, true)?;
                omen_platform::verify_startup_profile(&profile)?;
                let digest = profile_digest(&profile)?;
                let direct = omen_core::fan_profile::direct_part(&profile)?;
                let mut controller = prepare_apply(&direct, &mut store, true, false)?;
                device_checked = true;
                if omen_platform::nvpcf_ppt::preset_needs_unlock(&profile, ac == Some(true))
                    && omen_platform::nvpcf_ppt::ensure_unlocked(controller.inventory(), &store)
                        .is_ok()
                {
                    controller.set_nvpcf_ppt_unlocked(true);
                }
                // The linked curve is requested from the fan helper exactly as
                // a manual Apply does. A helper that cannot take it yet (it
                // may still be starting) does not hold back the other values.
                let request =
                    match crate::profile_fan::prepare(&profile, &store, controller.inventory()) {
                        Ok(request) => request,
                        Err(error) => {
                            cooling_error = Some(error);
                            None
                        }
                    };
                if let Err(error) = apply_hp_prelude(&profile, controller.inventory()) {
                    return Err(crate::profile_fan::prelude_failed(
                        &store,
                        request.as_ref(),
                        error,
                    ));
                }
                let applied =
                    transaction::apply(&direct, &digest, &mut controller, &mut store, true);
                let (applied, requested) = crate::profile_fan::finish(
                    &mut store,
                    &mut controller,
                    &profile,
                    request.as_ref(),
                    applied,
                );
                cooling = requested;
                let completion =
                    applied.map_err(|e| serde_json::to_string_pretty(&e).unwrap_or(e.error))?;
                if ac != Some(false) {
                    // Best effort: the applied preset stands either way.
                    let _ = store.save_battery_state(&state);
                }
                Ok(Some(completion))
            })();
            if result.is_err() && !device_checked {
                store.disable_startup()?;
            }
            store.save_startup_result(&serde_json::json!({"schema":1,"recorded_at_unix_seconds":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs()).ok(),"process_id":std::process::id(),"executable":std::env::current_exe().map_err(|e| e.to_string())?,"profile_directory":store.root(),"selection_at_start":selection_at_start,"skipped_on_battery":skipped_on_battery,"cooling":cooling,"cooling_error":cooling_error,"result":result}))?;
            print(&result)?;
            result.map(|_| ())
        }
        Some("profile-startup-status") if args.len() == 1 => print(
            &serde_json::json!({"selection":store.startup()?,"installation":omen_platform::verify_startup_installation()}),
        ),
        Some("profile-startup-check") if args.len() == 1 => {
            // Inspection only: never apply, enable, authorize or disable startup.
            let selection = store.startup()?;
            let result = (|| -> Result<Option<serde_json::Value>, String> {
                let Some(profile) = store.startup_profile()? else {
                    return Ok(None);
                };
                store.startup_eligibility(&profile.id)?;
                omen_platform::verify_startup_profile(&profile)?;
                let direct = omen_core::fan_profile::direct_part(&profile)?;
                let mut controller = Controller::for_profile(&direct)?;
                let plan = transaction::plan(&direct, &mut controller, true);
                if !plan.applicable {
                    return Err(serde_json::to_string(&plan).map_err(|e| e.to_string())?);
                }
                Ok(Some(
                    serde_json::json!({"profile_id":profile.id,"profile_digest":profile_digest(&profile)?}),
                ))
            })();
            print(
                &serde_json::json!({"schema":1,"selection":selection,"ready":matches!(&result,Ok(Some(_))),"verification":result,"hardware_applied":false}),
            )
        }
        _ => Err("Invalid profile command or argument count; use --help".into()),
    }
}

fn validate_nonce(nonce: Option<&str>) -> Result<(), String> {
    if nonce.is_some_and(|n| {
        n.is_empty() || n.len() > 64 || !n.bytes().all(|b| b.is_ascii_alphanumeric())
    }) {
        return Err("Invalid UI operation token".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cli_values_cannot_reintroduce_retired_settings() {
        for name in [
            "intel_pl1_mw",
            "intel_cache_voltage_offset_mv",
            "intel_core_ratios",
        ] {
            let feature = serde_json::from_value::<Feature>(serde_json::json!(name)).unwrap();
            assert!(parse_value(feature, "45000").is_err());
        }
        assert_eq!(
            parse_value(Feature::CpuPptLimitMw, "45000").unwrap(),
            Value::Integer(45000)
        );
    }
}
