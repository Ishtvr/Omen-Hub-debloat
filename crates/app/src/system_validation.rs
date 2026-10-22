//! Explicit, fixed, conservative qualification trials. Never run at startup.
//! Each trial uses the production transaction engine and restores the complete
//! original before returning the preceding recovery journal to its exact bytes.
use omen_core::{
    features::Feature,
    profiles::{Choice, CoreDelta, PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
    transaction::{self, Backend},
};
use omen_platform::profile_store::{ProfileStore, profile_digest};
use omen_platform::{ControlLease, Controller, CpuHardwareReader, RestoreState, SmuImage};
use serde_json::{Value as Json, json};

const CASES: &[&str] = &[
    "cpu-all-core",
    "cpu-one-core",
    "cpu-pbo-idempotent",
    "cpu-ppt",
    "cpu-slow",
    "cpu-tdc",
    "cpu-edc",
    "cpu-temperature",
    "cpu-fast-slow",
    "windows-boost",
    "windows-minimum",
    "windows-maximum",
    "windows-epp",
    "windows-parking-minimum",
    "windows-parking-maximum",
    "windows-energy",
    "windows-energy-incompatible",
    "windows-plan",
    "windows-plan-balanced",
    "windows-plan-policies",
    "windows-combined",
    "windows-refresh",
    "windows-ccd",
    "dynamic-boost",
    "dynamic-boost-aliases",
    "mux-preference",
    "removed-cpu-stapm",
    "removed-gpu-fps",
];

fn setting(feature: Feature, value: Value) -> Setting {
    Setting { feature, value }
}
fn integer(feature: Feature, value: i64) -> Setting {
    setting(feature, Value::Integer(value))
}
fn choice(feature: Feature, value: Choice) -> Setting {
    setting(feature, Value::Choice(value))
}

fn lower(controller: &mut Controller, feature: Feature, step: i64) -> Result<Setting, String> {
    let Value::Integer(value) = controller.read(feature)? else {
        return Err("Conservative limit trial needs an integer original".into());
    };
    let (minimum, _) = controller
        .capability(feature)
        .integer_range
        .ok_or("No qualified limit range")?;
    if value - step < minimum {
        return Err("Original is already at the minimum; no lower trial is possible".into());
    }
    Ok(integer(feature, value - step))
}

fn targets(name: &str, controller: &mut Controller) -> Result<Vec<Setting>, String> {
    use Feature::*;
    Ok(match name {
        "cpu-all-core" | "cpu-one-core" => {
            let image = controller.capture(CpuCurveOptimizerAllCore)?;
            // This fixed test is qualified for the user's stated -20 baseline.
            // -19 reduces the magnitude; it is not a millivolt voltage offset.
            if !matches!(&image.restore, RestoreState::AmdSmu { image: SmuImage::Margins { margins } }
                if margins.len() == 16 && margins.iter().all(|value| *value == -20))
            {
                return Err(
                    "Fixed CO trial requires the confirmed -20 baseline on all 16 cores".into(),
                );
            }
            vec![if name == "cpu-all-core" {
                integer(CpuCurveOptimizerAllCore, -19)
            } else {
                setting(
                    CpuCurveOptimizerPerCore,
                    Value::PerCore(vec![CoreDelta {
                        ccd: 0,
                        core: 0,
                        steps: -19,
                    }]),
                )
            }]
        }
        "cpu-pbo-idempotent" => {
            if controller.read(CpuPboScalar)? != Value::Integer(1) {
                return Err("Safe PBO check requires a 1x baseline".into());
            }
            vec![integer(CpuPboScalar, 1)]
        }
        "cpu-ppt" => vec![lower(controller, CpuPptLimitMw, 1000)?],
        "cpu-slow" | "cpu-slow-qualification" => vec![lower(controller, CpuSlowPptLimitMw, 1000)?],
        "cpu-tdc" => vec![lower(controller, CpuTdcMa, 1000)?],
        "cpu-edc" => vec![lower(controller, CpuEdcMa, 1000)?],
        "cpu-temperature" => vec![lower(controller, CpuTemperatureLimitC, 1)?],
        "cpu-fast-slow" => vec![
            lower(controller, CpuPptLimitMw, 1000)?,
            lower(controller, CpuSlowPptLimitMw, 1000)?,
        ],
        "windows-boost" => vec![choice(WindowsBoostMode, Choice::Disabled)],
        "windows-minimum" => vec![integer(WindowsMinimumProcessorPercent, 0)],
        "windows-maximum" => vec![integer(WindowsMaximumProcessorPercent, 90)],
        "windows-epp" => vec![integer(WindowsProcessorEppPercent, 80)],
        "windows-parking-minimum" => vec![integer(WindowsCoreParkingMinPercent, 0)],
        "windows-parking-maximum" => vec![integer(WindowsCoreParkingMaxPercent, 100)],
        "windows-energy" => vec![
            choice(WindowsEnergyMode, Choice::BestEfficiency),
            choice(WindowsPowerPlan, Choice::Balanced),
        ],
        "windows-plan" => vec![choice(WindowsPowerPlan, Choice::PowerSaver)],
        "windows-plan-balanced" => vec![choice(WindowsPowerPlan, Choice::Balanced)],
        "windows-plan-policies" => vec![
            // Intentionally put the plan last. The adapter must capture policies
            // from its destination, activate it first, and restore in reverse.
            integer(WindowsMaximumProcessorPercent, 90),
            choice(WindowsBoostMode, Choice::Disabled),
            choice(WindowsPowerPlan, Choice::Balanced),
        ],
        "windows-combined" => vec![
            choice(WindowsPowerPlan, Choice::Balanced),
            choice(WindowsEnergyMode, Choice::BestEfficiency),
            choice(WindowsBoostMode, Choice::Disabled),
            integer(WindowsMinimumProcessorPercent, 0),
            integer(WindowsMaximumProcessorPercent, 90),
            integer(WindowsProcessorEppPercent, 80),
            integer(WindowsCoreParkingMinPercent, 0),
            integer(WindowsCoreParkingMaxPercent, 100),
        ],
        "windows-refresh" => vec![integer(
            DisplayRefreshRateHz,
            controller
                .capability(DisplayRefreshRateHz)
                .integer_range
                .ok_or("No supported refresh rates")?
                .0,
        )],
        "windows-ccd" => vec![integer(CpuCcdAffinity, 1)],
        "dynamic-boost" | "dynamic-boost-aliases" => {
            let Value::Boolean(old) = controller.read(GpuDynamicBoost)? else {
                return Err("Dynamic Boost original unavailable".into());
            };
            let mut settings = vec![setting(GpuDynamicBoost, Value::Boolean(!old))];
            if name == "dynamic-boost-aliases" {
                settings.push(setting(GpuUnleashed, Value::Boolean(!old)));
            }
            settings
        }
        "windows-energy-incompatible" => vec![
            choice(WindowsEnergyMode, Choice::BestEfficiency),
            choice(WindowsPowerPlan, Choice::HighPerformance),
        ],
        "removed-cpu-stapm" => vec![integer(CpuStapmLimitMw, 80_000)],
        "removed-gpu-fps" => vec![integer(GpuFrameRateLimit, 60)],
        "mux-preference" => {
            let old = controller.read(GraphicsMode)?;
            let target = match old {
                Value::Choice(Choice::Hybrid) => Choice::DiscreteOnly,
                Value::Choice(Choice::DiscreteOnly) => Choice::Hybrid,
                _ => return Err("Safe MUX test requires Hybrid or Discrete baseline".into()),
            };
            vec![choice(GraphicsMode, target)]
        }
        _ => return Err("Unknown fixed qualification case".into()),
    })
}

fn run_case(name: &str, store: &mut ProfileStore) -> Result<Json, String> {
    let checkpoint = store.checkpoint_completed_journal()?;
    let mut controller = if name == "cpu-slow-qualification" {
        Controller::for_slow_limit_qualification()?
    } else {
        Controller::new(true, None)?
    };
    let before_gpu = controller.gpu_report()?;
    let before_curve = controller.gpu_curve()?;
    if controller.context()?.ac_connected != Some(true) {
        return Err("Fixed qualification requires AC power".into());
    }
    if name.starts_with("cpu-") {
        let sample = CpuHardwareReader::new(&omen_platform::inventory())?.temperature_sample()?;
        if !sample.temperature_c.is_some_and(|c| c < 85.0) {
            return Err("Conservative CPU qualification requires Tctl below 85 C".into());
        }
    }
    let cpu_features = [
        Feature::CpuCurveOptimizerAllCore,
        Feature::CpuPboScalar,
        Feature::CpuPptLimitMw,
        Feature::CpuSlowPptLimitMw,
        Feature::CpuTdcMa,
        Feature::CpuEdcMa,
        Feature::CpuTemperatureLimitC,
    ];
    let cpu_originals = if name.starts_with("cpu-") {
        cpu_features
            .iter()
            .map(|feature| controller.capture(*feature))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![]
    };
    let settings = targets(name, &mut controller)?;
    let profile = PerformanceProfile {
        schema: PROFILE_SCHEMA,
        id: format!("safe-{name}"),
        name: format!("Fixed safe {name}"),
        binding: controller.context()?.binding,
        settings,
    };
    let plan = transaction::plan(&profile, &mut controller, false);
    let must_reject = matches!(
        name,
        "windows-energy-incompatible" | "removed-cpu-stapm" | "removed-gpu-fps"
    );
    if must_reject && plan.applicable {
        return Err(
            "Retired or incompatible setting unexpectedly passed preflight; no SET was sent".into(),
        );
    }
    if !plan.applicable {
        return Ok(
            json!({"case":name,"status":if must_reject {"passed_preflight_rejection"} else {"failed"},"plan":plan}),
        );
    }
    let digest = profile_digest(&profile)?;
    let result = transaction::trial(&profile, &digest, &mut controller, store);
    let saved = store.last_transaction::<RestoreState>()?;
    if saved.profile_id != profile.id || saved.profile_digest != digest {
        return Err("Qualification journal identity differs; checkpoint retained".into());
    }
    // A new adapter session independently reads the full original images.
    // Destination-plan policy originals are checked even while inactive.
    let mut independent = Controller::new(true, None)?;
    independent.prepare_settings(&saved.requested)?;
    let mut original_checks = vec![];
    for original in &saved.original {
        let actual = independent.capture(original.feature)?;
        original_checks.push(json!({"feature":original.feature,"full_original_restored":actual.restore == original.restore,"original":original.restore,"after":actual.restore}));
    }
    for original in &cpu_originals {
        let actual = independent.capture(original.feature)?;
        original_checks.push(json!({"feature":original.feature,"cross_control_check":true,"full_original_restored":actual.restore==original.restore,"original":original.restore,"after":actual.restore}));
    }
    let after_gpu = independent.gpu_report()?;
    let after_curve = independent.gpu_curve()?;
    let restored = original_checks
        .iter()
        .all(|v| v["full_original_restored"] == true)
        && before_gpu.identity == after_gpu.identity
        && before_gpu.core == after_gpu.core
        && before_gpu.memory == after_gpu.memory
        && before_curve.controls_match(&after_curve)?;
    if restored && !store.pending()? {
        store.restore_completed_trial_checkpoint(checkpoint, &profile.id, &digest)?;
    }
    Ok(
        json!({"case":name,"status":if result.is_ok() && restored {"passed"} else {"failed"},
        "transaction":result,"full_originals_restored":restored,"checks":original_checks,
        "gpu_controls_preserved":before_curve.controls_match(&after_curve)?,
        "scope":if name == "mux-preference" {"BIOS boot preference GET/SET/restore only; no physical display switch or reboot"}
        else if name == "cpu-pbo-idempotent" {"1x GET/SET/readback/restore; scalar is not increased"}
        else if name == "windows-ccd" {"saved CCD selection and exact restore; foreground affinity is tested separately"}
        else {"fixed conservative target, full readback and immediate complete restore"}}),
    )
}

pub fn run(case: &str) -> Result<(), String> {
    let selected = if case == "all" {
        CASES.to_vec()
    } else if CASES.contains(&case) || case == "cpu-slow-qualification" {
        vec![case]
    } else {
        return Err(format!(
            "Unknown safe qualification case; choose all or one of {CASES:?}"
        ));
    };
    let _lease = ControlLease::acquire()?;
    let mut store = ProfileStore::user_data()?;
    let mut reports = vec![];
    for name in selected {
        let report = match run_case(name, &mut store) {
            Ok(report) => report,
            Err(error) => json!({"case":name,"status":"failed","error":error}),
        };
        let failed = report["status"] == "failed";
        reports.push(report);
        if failed {
            break;
        }
    }
    let failed = reports.iter().any(|r| r["status"] == "failed");
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema":1,"cases":reports,"pending_recovery":store.pending()?})
        )
        .map_err(|e| e.to_string())?
    );
    if failed {
        Err("Safe qualification stopped at a failed case; inspect its full restore record".into())
    } else {
        Ok(())
    }
}
