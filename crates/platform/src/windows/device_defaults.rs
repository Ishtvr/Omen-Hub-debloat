//! Automatic and on-demand device defaults discovery.
//!
//! - `discover_passive` performs zero hardware writes and never wakes a
//!   sleeping dGPU on battery; it captures Windows, display, BIOS and
//!   clean-boot SMU defaults automatically when Presets opens.
//! - `discover_active_elevated` (CLI `device-defaults-detect` & Settings
//!   button) safely probes Standard (Balanced), Performance and (on AC)
//!   Unleashed modes to discover and persist each mode's exact TDC, EDC,
//!   PPT, Slow/PL1 and Tctl defaults for any supported laptop, restoring
//!   the user's prior mode and active tuning afterwards.
use super::{
    ControlLease, Controller, RestoreState, boot_unix_s, cpu_tuning, dgpu, hp, nvidia,
    power_status, smu::Smu, windows_settings,
};
use crate::profile_store::ProfileStore;
use omen_core::{
    Inventory,
    battery::Selection,
    device_defaults::{
        DeviceDefaultsRecord, ModeDefaults, ModePowerDefaults, merge_discovered, resolve_defaults,
        sanitize_default, zero_per_core,
    },
    features::Feature,
    hp_mode::HpMode,
    profiles::{Setting, Value},
    transaction::{Access, Backend},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const WINDOWS_DEFAULT_FEATURES: &[Feature] = &[
    Feature::WindowsBoostMode,
    Feature::WindowsMinimumProcessorPercent,
    Feature::WindowsMaximumProcessorPercent,
    Feature::WindowsProcessorEppPercent,
    Feature::WindowsCoreParkingMinPercent,
    Feature::WindowsCoreParkingMaxPercent,
    Feature::CpuCcdAffinity,
    Feature::WindowsEnergyMode,
    Feature::WindowsPowerPlan,
    Feature::DisplayRefreshRateHz,
];

const SMU_LIMIT_FEATURES: &[Feature] = &[
    Feature::CpuPptLimitMw,
    Feature::CpuSlowPptLimitMw,
    Feature::CpuTdcMa,
    Feature::CpuEdcMa,
    Feature::CpuTemperatureLimitC,
];

const SMU_VOLATILE_FEATURES: &[Feature] = &[
    Feature::CpuPptLimitMw,
    Feature::CpuSlowPptLimitMw,
    Feature::CpuTdcMa,
    Feature::CpuEdcMa,
    Feature::CpuTemperatureLimitC,
    Feature::CpuPboScalar,
];

fn now_unix_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn primary_cpu(inventory: &Inventory) -> &str {
    inventory.cpus.first().map(String::as_str).unwrap_or("")
}

fn mux_choice(mode: u8) -> Option<omen_core::profiles::Choice> {
    use omen_core::profiles::Choice;
    match mode {
        0 => Some(Choice::Hybrid),
        1 => Some(Choice::DiscreteOnly),
        3 => Some(Choice::IntegratedOnly),
        _ => None,
    }
}

/// Features currently modified by an active applied preset, alongside their
/// captured pre-preset originals from the control journal (if available).
fn applied_originals(store: &ProfileStore) -> (BTreeSet<Feature>, BTreeMap<Feature, Value>) {
    let mut active = BTreeSet::new();
    let mut originals = BTreeMap::new();
    if let Ok(Some((_, _, requested))) = store.applied_settings() {
        for setting in &requested {
            active.insert(setting.feature);
        }
        if let Ok(tx) = store.last_transaction::<RestoreState>() {
            for item in tx.original {
                originals.insert(item.feature, item.value);
            }
        }
    }
    (active, originals)
}

fn factory_zero_readings() -> Vec<Setting> {
    vec![
        Setting {
            feature: Feature::GpuCoreOffsetMhz,
            value: Value::Integer(0),
        },
        Setting {
            feature: Feature::GpuMemoryOffsetMhz,
            value: Value::Integer(0),
        },
        Setting {
            feature: Feature::GpuUnleashed,
            value: Value::Boolean(false),
        },
        Setting {
            feature: Feature::GpuCustomTgp,
            value: Value::Boolean(true),
        },
        Setting {
            feature: Feature::CpuCurveOptimizerAllCore,
            value: Value::Integer(0),
        },
        Setting {
            feature: Feature::CpuCurveOptimizerPerCore,
            value: Value::PerCore(zero_per_core(16)),
        },
    ]
}

fn collect_system_readings(
    inventory: &Inventory,
    store: &ProfileStore,
    existing: Option<&DeviceDefaultsRecord>,
    force_refresh: bool,
) -> Vec<Setting> {
    let (active_features, journal_originals) = applied_originals(store);
    let mut readings = factory_zero_readings();
    let mut push_clean = |feature: Feature, value: Value| {
        if let Some(clean) = sanitize_default(feature, value) {
            readings.retain(|s| s.feature != feature);
            readings.push(Setting {
                feature,
                value: clean,
            });
        }
    };

    for &feature in WINDOWS_DEFAULT_FEATURES {
        let already_stored = existing.is_some_and(|r| r.get(feature).is_some());
        if active_features.contains(&feature) {
            if let Some(orig) = journal_originals.get(&feature) {
                if force_refresh || !already_stored {
                    push_clean(feature, orig.clone());
                }
            }
            continue;
        }
        if (force_refresh || !already_stored)
            && let Ok(value) = windows_settings::read(feature)
        {
            push_clean(feature, value);
        }
    }

    let graphics_stored = existing.is_some_and(|r| r.get(Feature::GraphicsMode).is_some());
    if active_features.contains(&Feature::GraphicsMode) {
        if let Some(orig) = journal_originals.get(&Feature::GraphicsMode)
            && (force_refresh || !graphics_stored)
        {
            push_clean(Feature::GraphicsMode, orig.clone());
        }
    } else if (force_refresh || !graphics_stored)
        && let Ok(mode) = hp::read_hp_graphics_mode(inventory)
        && let Some(choice) = mux_choice(mode)
    {
        push_clean(Feature::GraphicsMode, Value::Choice(choice));
    }

    // Never wake a sleeping dGPU on battery, and never record an active
    // flashless VBIOS TGP override (e.g. 135 W) as the GPU's factory ceiling.
    let tgp_stored = existing.is_some_and(|r| r.get(Feature::GpuDynamicBoostLimitMw).is_some());
    if (force_refresh || !tgp_stored)
        && store.vbios_tgp_state().is_none()
        && dgpu::nvidia_gpu_awake() != Some(false)
        && let Ok((_, _, max_mw)) = nvidia::read_any_nvml_power_limits()
    {
        let whole_mw = i64::from((max_mw + 500) / 1000) * 1000;
        if whole_mw >= 15_000 {
            push_clean(Feature::GpuDynamicBoostLimitMw, Value::Integer(whole_mw));
        }
    }

    readings
}

fn read_smu_power_snapshot(smu: &Smu) -> ModePowerDefaults {
    let read_i64 = |feature: Feature| -> Option<i64> {
        let image = cpu_tuning::capture(smu, feature).ok()?;
        match cpu_tuning::value(feature, &image).ok()? {
            Value::Integer(v) => Some(v),
            _ => None,
        }
    };
    ModePowerDefaults {
        ppt_mw: read_i64(Feature::CpuPptLimitMw),
        slow_ppt_mw: read_i64(Feature::CpuSlowPptLimitMw),
        tdc_ma: read_i64(Feature::CpuTdcMa),
        edc_ma: read_i64(Feature::CpuEdcMa),
        tctl_c: read_i64(Feature::CpuTemperatureLimitC),
    }
}

/// Passive, zero-write defaults discovery invoked automatically when loading
/// presets. Never touches hardware controls and never wakes a sleeping dGPU.
pub fn discover_passive(
    inventory: &Inventory,
    store: &ProfileStore,
    controller: &mut Controller,
) -> DeviceDefaultsRecord {
    let existing = store.device_defaults_record(&inventory.board, &inventory.bios);
    let mut readings = collect_system_readings(inventory, store, existing.as_ref(), false);
    let mut modes = ModeDefaults::default();

    // Record volatile SMU firmware limits automatically while untouched since boot.
    if !store.controlled_since(boot_unix_s()).unwrap_or(true) {
        let mut standard = ModePowerDefaults::default();
        for &feature in SMU_VOLATILE_FEATURES {
            if controller.capability(feature).access == Access::ReadWrite
                && let Ok(value) = controller.read(feature)
            {
                if let Value::Integer(v) = value {
                    match feature {
                        Feature::CpuPptLimitMw => standard.ppt_mw = Some(v),
                        Feature::CpuSlowPptLimitMw => standard.slow_ppt_mw = Some(v),
                        Feature::CpuTdcMa => standard.tdc_ma = Some(v),
                        Feature::CpuEdcMa => standard.edc_ma = Some(v),
                        Feature::CpuTemperatureLimitC => standard.tctl_c = Some(v),
                        _ => {}
                    }
                }
                readings.retain(|s| s.feature != feature);
                readings.push(Setting { feature, value });
            }
        }
        if !standard.is_empty() && existing.as_ref().is_none_or(|r| r.modes.standard.is_none()) {
            modes.standard = Some(standard);
        }
    }

    let modes_opt = (!modes.is_empty()).then_some(&modes);
    let merged = merge_discovered(
        &inventory.board,
        &inventory.bios,
        primary_cpu(inventory),
        existing.as_ref(),
        &readings,
        modes_opt,
        None,
    );
    if existing.as_ref() != Some(&merged) {
        let _ = store.save_device_defaults_record(&merged);
    }
    resolve_defaults(&inventory.board, &inventory.bios, Some(&merged))
}

/// Active elevated discovery for any user (CLI `device-defaults-detect` and
/// Settings -> "Detect device defaults"). Safely reads all Windows/BIOS/GPU
/// defaults plus Standard (Balanced), Performance and (when on AC) Unleashed
/// CPU TDC/EDC/PPT/Slow/Tctl limits, then restores the prior mode and active
/// SMU limits cleanly.
pub fn discover_active_elevated(
    inventory: &Inventory,
    store: &ProfileStore,
) -> Result<DeviceDefaultsRecord, String> {
    if store.pending()? || store.fan_policy_pending()? {
        return Err(omen_core::message::msg("store.err.pending_checkpoint", &[]));
    }
    let _lease = ControlLease::acquire_foreground()?;
    let existing = store.device_defaults_record(&inventory.board, &inventory.bios);
    let mut readings = collect_system_readings(inventory, store, existing.as_ref(), true);
    let (active_features, journal_originals) = applied_originals(store);
    let ac = power_status().ac_connected == Some(true);

    // Determine what HP mode was active before probing so we return to it cleanly.
    let restore_mode = store
        .applied_settings()
        .ok()
        .flatten()
        .and_then(|(_, _, req)| HpMode::of_settings(&req, ac))
        .or_else(|| match store.ac_selection() {
            Some(Selection::Mode(mode)) => Some(mode),
            _ => None,
        })
        .unwrap_or(HpMode::Balanced);

    let prior_gpu_flags = hp::read_hp_gpu_status(inventory).ok();
    let smu = Smu::new(inventory).ok();
    let prior_smu_limits: Vec<(Feature, Value)> = smu
        .as_ref()
        .map(|smu| {
            SMU_LIMIT_FEATURES
                .iter()
                .filter_map(|&f| {
                    let img = cpu_tuning::capture(smu, f).ok()?;
                    let val = cpu_tuning::value(f, &img).ok()?;
                    Some((f, val))
                })
                .collect()
        })
        .unwrap_or_default();

    // If an applied preset overrode TDC/EDC/Tctl (which HP mode switches do not
    // overwrite), temporarily restore the captured pre-preset originals while
    // probing so Standard & Performance read genuine factory limits.
    if let Some(smu) = smu.as_ref() {
        for &f in &[
            Feature::CpuTdcMa,
            Feature::CpuEdcMa,
            Feature::CpuTemperatureLimitC,
        ] {
            if active_features.contains(&f)
                && let Some(orig) = journal_originals.get(&f)
            {
                let _ = cpu_tuning::write(smu, f, orig);
            }
        }
    }

    let mut modes = ModeDefaults::default();
    let mut mode_probed = false;

    if let Some(smu) = smu.as_ref() {
        // Probe Standard (Balanced) mode.
        if hp::apply_hp_mode(inventory, HpMode::Balanced).is_ok() {
            mode_probed = true;
            std::thread::sleep(Duration::from_millis(500));
            let standard = read_smu_power_snapshot(smu);
            if !standard.is_empty() {
                modes.standard = Some(standard);
            }
        }
        // Probe Performance mode.
        if mode_probed && hp::apply_hp_mode(inventory, HpMode::Performance).is_ok() {
            std::thread::sleep(Duration::from_millis(500));
            let performance = read_smu_power_snapshot(smu);
            if !performance.is_empty() {
                modes.performance = Some(performance);
            }
        }
        // Probe Unleashed mode when on AC.
        if mode_probed && ac && hp::apply_hp_mode(inventory, HpMode::Unleashed).is_ok() {
            std::thread::sleep(Duration::from_millis(500));
            let unleashed = read_smu_power_snapshot(smu);
            if !unleashed.is_empty() {
                modes.unleashed = Some(unleashed);
            }
        }
        // Fallback for non-8E35 boards where HP mode switching is unavailable:
        // record the current live SMU limits as the Standard baseline.
        if !mode_probed {
            let current = read_smu_power_snapshot(smu);
            if !current.is_empty() {
                modes.standard = Some(current);
            }
        }
        // Read PBO scalar stock value (default 1× unless overridden by preset).
        if active_features.contains(&Feature::CpuPboScalar) {
            if let Some(orig) = journal_originals.get(&Feature::CpuPboScalar) {
                readings.retain(|s| s.feature != Feature::CpuPboScalar);
                readings.push(Setting {
                    feature: Feature::CpuPboScalar,
                    value: orig.clone(),
                });
            }
        } else if let Ok(img) = cpu_tuning::capture(smu, Feature::CpuPboScalar)
            && let Ok(val) = cpu_tuning::value(Feature::CpuPboScalar, &img)
        {
            readings.retain(|s| s.feature != Feature::CpuPboScalar);
            readings.push(Setting {
                feature: Feature::CpuPboScalar,
                value: val,
            });
        }
    }

    // Restore the user's prior mode, HP GPU flags and active SMU limits cleanly.
    if mode_probed {
        let target_mode = if restore_mode == HpMode::Unleashed && !ac {
            HpMode::Performance
        } else {
            restore_mode
        };
        let _ = hp::apply_hp_mode(inventory, target_mode);
        std::thread::sleep(Duration::from_millis(400));
        if let Some(flags) = prior_gpu_flags {
            let _ = hp::write_hp_gpu_power(inventory, flags.custom_tgp_enabled, flags.ppab_enabled);
        }
    }
    if let Some(smu) = smu.as_ref() {
        for (feature, value) in &prior_smu_limits {
            let _ = cpu_tuning::write(smu, *feature, value);
        }
    }

    // Mirror discovered Standard mode limits into top-level settings so every
    // preset editor field shows this device's exact stock baseline.
    if let Some(standard) = &modes.standard {
        let mut set_int = |feature: Feature, value: Option<i64>| {
            if let Some(v) = value {
                readings.retain(|s| s.feature != feature);
                readings.push(Setting {
                    feature,
                    value: Value::Integer(v),
                });
            }
        };
        set_int(Feature::CpuPptLimitMw, standard.ppt_mw);
        set_int(Feature::CpuSlowPptLimitMw, standard.slow_ppt_mw);
        set_int(Feature::CpuTdcMa, standard.tdc_ma);
        set_int(Feature::CpuEdcMa, standard.edc_ma);
        set_int(Feature::CpuTemperatureLimitC, standard.tctl_c);
    }

    let merged = merge_discovered(
        &inventory.board,
        &inventory.bios,
        primary_cpu(inventory),
        existing.as_ref(),
        &readings,
        (!modes.is_empty()).then_some(&modes),
        Some(now_unix_s()),
    );
    store.save_device_defaults_record(&merged)?;
    Ok(resolve_defaults(
        &inventory.board,
        &inventory.bios,
        Some(&merged),
    ))
}
