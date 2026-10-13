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
            | "profile-apply"
            | "profile-restore"
            | "profile-ui-apply"
            | "profile-ui-restore"
            | "profile-ui-startup-enable"
            | "profile-startup-enable"
            | "profile-startup-disable"
            | "profile-startup-apply"
            | "profile-startup-status"
            | "profile-storage"
            | "profile-migrate-legacy"
    )
}

pub fn run(args: &[String]) -> Result<(), String> {
    let result = run_inner(args);
    // Return early errors (e.g. access/session/empty journal) to the matching UI
    // request, without overwriting a more detailed transaction/rollback result.
    if let Err(error) = &result {
        let ui = match args.first().map(String::as_str) {
            Some("profile-ui-apply") if args.len() == 3 => Some(("apply", args[2].as_str())),
            Some("profile-ui-restore") if args.len() == 2 => Some(("restore", args[1].as_str())),
            Some("profile-ui-startup-enable") if args.len() == 3 => {
                Some(("startup", args[2].as_str()))
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
        Some("gpu-curve-read") if args.len() == 1 => {
            let mut controller = Controller::new(true, None)?;
            return print(&controller.gpu_curve()?);
        }
        Some("profile-storage") if args.len() == 1 => {
            return print(&serde_json::json!({"directory":ProfileStore::user_data()?.root()}));
        }
        Some("profile-migrate-legacy") if args.len() == 1 => {
            let _lease = ControlLease::acquire()?;
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
        Some("gpu-validate-memory-curve") if args.len() == 1 => {
            let _lease = ControlLease::acquire()?;
            if store.pending()? {
                return Err("Pending recovery blocks a preservation trial".into());
            }
            let mut controller = Controller::new(true, None)?;
            let before = controller.gpu_curve()?;
            let result = controller.validate_memory_curve_preservation(&mut store)?;
            print(
                &serde_json::json!({"before":before,"transaction":result,"after":controller.gpu_curve()?,"gpu":controller.gpu_report()?}),
            )
        }
        Some("gpu-validate-curve") if args.len() == 1 => {
            let _lease = ControlLease::acquire()?;
            if store.pending()? {
                return Err("Pending recovery blocks a curve trial".into());
            }
            let mut controller = Controller::new(true, None)?;
            let before = controller.gpu_curve()?;
            let result = controller.validate_gpu_curve(&mut store)?;
            let after = controller.gpu_curve()?;
            print(
                &serde_json::json!({"before":before,"transaction":result,"after":after,"gpu":controller.gpu_report()?}),
            )
        }
        Some("gpu-validate") if args.len() == 1 => {
            let _lease = ControlLease::acquire()?;
            if store.pending()? {
                return Err("An earlier transaction requires recovery before validation".into());
            }
            let mut controller = Controller::new(true, None)?;
            let before = controller.gpu_report()?;
            let result = controller.validate_gpu_offsets(&mut store)?;
            let after = controller.gpu_report()?;
            print(&serde_json::json!({"before":before,"transaction":result,"after":after}))
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
            let mut controller = Controller::for_profile(&profile)?;
            print(&transaction::plan(&profile, &mut controller, false))
        }
        Some("profile-apply" | "profile-ui-apply")
            if args.len() == 2 || (args[0] == "profile-ui-apply" && args.len() == 3) =>
        {
            let _lease = ControlLease::acquire()?;
            let nonce = args.get(2).map(String::as_str);
            validate_nonce(nonce)?;
            let profile = store.load(&args[1])?;
            let digest = profile_digest(&profile)?;
            let mut controller = Controller::for_profile(&profile)?;
            let result = transaction::apply(&profile, &digest, &mut controller, &mut store, false);
            store.save_control_result(&serde_json::json!({"schema":1,"operation":"apply","nonce":nonce,"profile_id":profile.id,"profile_digest":digest,"result":result}))?;
            print(&result)?;
            result
                .map(|_| ())
                .map_err(|e| serde_json::to_string_pretty(&e).unwrap_or(e.error))
        }
        Some("profile-restore" | "profile-ui-restore")
            if args.len() == 1 || (args[0] == "profile-ui-restore" && args.len() == 2) =>
        {
            let _lease = ControlLease::acquire()?;
            let nonce = args.get(1).map(String::as_str);
            validate_nonce(nonce)?;
            // Disable further automatic application before explicit recovery.
            store.disable_startup()?;
            let saved = store.last_transaction::<RestoreState>()?;
            let mut controller = Controller::new(true, saved.binding.nvidia_uuid.as_deref())?;
            let result = transaction::recover(&saved, &mut controller, &mut store);
            store.save_control_result(&serde_json::json!({"schema":1,"operation":"restore","nonce":nonce,"result":result}))?;
            result.map_err(|e| serde_json::to_string_pretty(&e).unwrap_or(e.error))?;
            print(
                &serde_json::json!({"restored":true,"profile_id":saved.profile_id,"gpu":controller.gpu_report()?}),
            )
        }
        Some("profile-startup-enable" | "profile-ui-startup-enable")
            if args.len() == 2 || (args[0] == "profile-ui-startup-enable" && args.len() == 3) =>
        {
            let _lease = ControlLease::acquire()?;
            let nonce = args.get(2).map(String::as_str);
            validate_nonce(nonce)?;
            omen_platform::verify_startup_installation()?;
            let profile = store.load(&args[1])?;
            let mut controller = Controller::for_profile(&profile)?;
            let plan = transaction::plan(&profile, &mut controller, true);
            if !plan.applicable {
                return Err(serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?);
            }
            store.startup_eligibility(&args[1])?;
            controller.verify_applied(&store.last_transaction::<RestoreState>()?)?;
            omen_platform::authorize_startup_profile(&profile)?;
            store.enable_startup(&args[1])?;
            store.save_control_result(&serde_json::json!({"schema":1,"operation":"startup","nonce":nonce,"result":{"Ok":store.startup()?}}))?;
            print(&store.startup()?)
        }
        Some("profile-startup-disable") if args.len() == 1 => {
            let _lease = ControlLease::acquire()?;
            store.disable_startup()?;
            print(&store.startup()?)
        }
        Some("profile-startup-apply") if args.len() == 1 => {
            let _lease = ControlLease::acquire()?;
            let selection_at_start = store.startup()?;
            let result = (|| {
                let Some(profile) = store.startup_profile()? else {
                    return Ok(None);
                };
                omen_platform::verify_startup_profile(&profile)?;
                let digest = profile_digest(&profile)?;
                let mut controller = Controller::for_profile(&profile)?;
                transaction::apply(&profile, &digest, &mut controller, &mut store, true)
                    .map(Some)
                    .map_err(|e| serde_json::to_string_pretty(&e).unwrap_or(e.error))
            })();
            if result.is_err() {
                store.disable_startup()?;
            }
            store.save_startup_result(&serde_json::json!({"schema":1,"executable":std::env::current_exe().map_err(|e| e.to_string())?,"profile_directory":store.root(),"selection_at_start":selection_at_start,"result":result}))?;
            print(&result)?;
            result.map(|_| ())
        }
        Some("profile-startup-status") if args.len() == 1 => print(
            &serde_json::json!({"selection":store.startup()?,"installation":omen_platform::verify_startup_installation()}),
        ),
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
