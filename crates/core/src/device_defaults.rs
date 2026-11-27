//! Discovered per-device factory defaults and mode power/current envelopes.
//! Stored per board/BIOS in `device-defaults.json` and used across Presets,
//! Reset and Settings so unticked fields always reflect this device's own
//! values instead of stale static guesses.
use crate::{
    features::{Feature, ValueKind},
    profiles::{Choice, CoreDelta, Setting, Value},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULTS_SCHEMA: u32 = 1;

/// Power, current and temperature limits observed in one HP performance mode.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModePowerDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppt_mw: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slow_ppt_mw: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tdc_ma: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edc_ma: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tctl_c: Option<i64>,
}

impl ModePowerDefaults {
    pub fn is_empty(&self) -> bool {
        self.ppt_mw.is_none()
            && self.slow_ppt_mw.is_none()
            && self.tdc_ma.is_none()
            && self.edc_ma.is_none()
            && self.tctl_c.is_none()
    }

    pub fn merge_over(&self, newer: &Self) -> Self {
        Self {
            ppt_mw: newer.ppt_mw.or(self.ppt_mw),
            slow_ppt_mw: newer.slow_ppt_mw.or(self.slow_ppt_mw),
            tdc_ma: newer.tdc_ma.or(self.tdc_ma),
            edc_ma: newer.edc_ma.or(self.edc_ma),
            tctl_c: newer.tctl_c.or(self.tctl_c),
        }
    }
}

/// Standard (Balanced), Performance, Unleashed and Quiet firmware limits.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub standard: Option<ModePowerDefaults>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance: Option<ModePowerDefaults>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unleashed: Option<ModePowerDefaults>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet: Option<ModePowerDefaults>,
}

impl ModeDefaults {
    pub fn is_empty(&self) -> bool {
        self.standard.as_ref().is_none_or(ModePowerDefaults::is_empty)
            && self
                .performance
                .as_ref()
                .is_none_or(ModePowerDefaults::is_empty)
            && self
                .unleashed
                .as_ref()
                .is_none_or(ModePowerDefaults::is_empty)
            && self.quiet.as_ref().is_none_or(ModePowerDefaults::is_empty)
    }

    pub fn merge_over(&self, newer: &Self) -> Self {
        let combine = |a: &Option<ModePowerDefaults>, b: &Option<ModePowerDefaults>| match (a, b) {
            (Some(a), Some(b)) => Some(a.merge_over(b)),
            (None, Some(b)) => Some(b.clone()),
            (Some(a), None) => Some(a.clone()),
            (None, None) => None,
        };
        Self {
            standard: combine(&self.standard, &newer.standard),
            performance: combine(&self.performance, &newer.performance),
            unleashed: combine(&self.unleashed, &newer.unleashed),
            quiet: combine(&self.quiet, &newer.quiet),
        }
    }
}

/// Complete persisted defaults for one exact board/BIOS pair.
/// Backwards-compatible with older `device-defaults.json` files that stored
/// only `schema`, `board`, `bios` and `settings`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceDefaultsRecord {
    pub schema: u32,
    pub board: String,
    pub bios: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cpu: String,
    #[serde(default)]
    pub settings: Vec<Setting>,
    #[serde(default, skip_serializing_if = "ModeDefaults::is_empty")]
    pub modes: ModeDefaults,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_unix_s: Option<u64>,
}

impl DeviceDefaultsRecord {
    pub fn new(board: impl Into<String>, bios: impl Into<String>) -> Self {
        Self {
            schema: DEFAULTS_SCHEMA,
            board: board.into(),
            bios: bios.into(),
            cpu: String::new(),
            settings: Vec::new(),
            modes: ModeDefaults::default(),
            detected_unix_s: None,
        }
    }

    pub fn map(&self) -> BTreeMap<Feature, Value> {
        self.settings
            .iter()
            .map(|s| (s.feature, s.value.clone()))
            .collect()
    }

    pub fn get(&self, feature: Feature) -> Option<&Value> {
        self.settings
            .iter()
            .find(|s| s.feature == feature)
            .map(|s| &s.value)
    }

    /// Standard (Balanced) or Performance TDC/EDC in mA, falling back to the
    /// general device default if per-mode measurements are absent.
    pub fn mode_tdc_edc(&self, performance: bool) -> (Option<i64>, Option<i64>) {
        let mode = if performance {
            self.modes.performance.as_ref().or(self.modes.standard.as_ref())
        } else {
            self.modes.standard.as_ref()
        };
        let top_int = |f| match self.get(f) {
            Some(Value::Integer(v)) => Some(*v),
            _ => None,
        };
        let tdc = mode.and_then(|m| m.tdc_ma).or_else(|| top_int(Feature::CpuTdcMa));
        let edc = mode.and_then(|m| m.edc_ma).or_else(|| top_int(Feature::CpuEdcMa));
        (tdc, edc)
    }

    /// Slow/PL1 limit in mW for Standard (Balanced) or Dynamic Boost (Unleashed).
    pub fn mode_slow_ppt_mw(&self, unleashed: bool) -> Option<i64> {
        let mode = if unleashed {
            self.modes
                .unleashed
                .as_ref()
                .or(self.modes.performance.as_ref())
        } else {
            self.modes.standard.as_ref()
        };
        mode.and_then(|m| m.slow_ppt_mw).or_else(|| match self.get(Feature::CpuSlowPptLimitMw) {
            Some(Value::Integer(v)) => Some(*v),
            _ => None,
        })
    }
}

/// Zeroed 16-core Curve Optimizer factory image (CCD0 C0..7, CCD1 C0..7).
pub fn zero_per_core(cores: u8) -> Vec<CoreDelta> {
    let count = cores.clamp(1, 64);
    (0..count)
        .map(|i| CoreDelta {
            ccd: i / 8,
            core: i % 8,
            steps: 0,
        })
        .collect()
}

/// Normalizes a candidate default reading so a previously applied tuning offset
/// (e.g. CO +10 or GPU +150 MHz from another tool) never becomes a stock default,
/// and unrecognized Windows `Choice::Manual` states are omitted.
pub fn sanitize_default(feature: Feature, value: Value) -> Option<Value> {
    let normalized = match feature {
        Feature::CpuCurveOptimizerAllCore => Value::Integer(0),
        Feature::CpuCurveOptimizerPerCore => match value {
            Value::PerCore(cores) if !cores.is_empty() => Value::PerCore(
                cores
                    .into_iter()
                    .map(|c| CoreDelta {
                        ccd: c.ccd,
                        core: c.core,
                        steps: 0,
                    })
                    .collect(),
            ),
            _ => Value::PerCore(zero_per_core(16)),
        },
        Feature::GpuCoreOffsetMhz | Feature::GpuMemoryOffsetMhz => Value::Integer(0),
        Feature::WindowsPowerPlan | Feature::WindowsEnergyMode | Feature::GraphicsMode => {
            if value == Value::Choice(Choice::Manual) {
                return None;
            }
            value
        }
        _ => value,
    };
    if feature.descriptor().value_kind == ValueKind::Choice && normalized == Value::Choice(Choice::Manual) {
        return None;
    }
    let setting = Setting {
        feature,
        value: normalized,
    };
    setting.validate().ok()?;
    Some(setting.value)
}

/// Verified factory defaults for measured reference boards/BIOS versions.
/// On HP 8E35 / BIOS F.13 (AMD Ryzen 9 8940HX + RTX 5070 Laptop), SMU PM-table
/// measurements (25 November 2026) and HP OMEN Gaming Hub's `Khalilah_DRG_N22X4X6`
/// table confirm identical fused/firmware TDC (108 A) and EDC (160 A) across
/// Standard (Balanced), Performance, Unleashed and Quiet, while Slow/PL1 steps
/// across 55 W (Quiet/Standard), 60 W (Performance) and 80 W (Unleashed).
pub fn known_board_defaults(board: &str, bios: &str) -> Option<DeviceDefaultsRecord> {
    if board != "8E35" || bios != "F.13" {
        return None;
    }
    use crate::hp_mode::HpMode;
    let modes = ModeDefaults {
        standard: Some(HpMode::Balanced.default_power_limits()),
        performance: Some(HpMode::Performance.default_power_limits()),
        unleashed: Some(HpMode::Unleashed.default_power_limits()),
        quiet: Some(HpMode::Quiet.default_power_limits()),
    };
    let settings = vec![
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
            feature: Feature::GpuDynamicBoostLimitMw,
            value: Value::Integer(i64::from(crate::profiles::GPU_DYNAMIC_BOOST_LIMIT_MIN_W) * 1000),
        },
        Setting {
            feature: Feature::HpSharedPowerMw,
            value: Value::Integer(i64::from(crate::hp_mode::SHARED_BUDGET_MAX_W) * 1000),
        },
        Setting {
            feature: Feature::CpuSlowPptLimitMw,
            value: Value::Integer(HpMode::Balanced.default_pl1_mw()),
        },
        Setting {
            feature: Feature::CpuTemperatureLimitC,
            value: Value::Integer(HpMode::Balanced.default_tctl_c()),
        },
        Setting {
            feature: Feature::CpuPptLimitMw,
            value: Value::Integer(HpMode::Balanced.default_ppt_mw()),
        },
        Setting {
            feature: Feature::CpuTdcMa,
            value: Value::Integer(HpMode::Balanced.default_tdc_ma()),
        },
        Setting {
            feature: Feature::CpuEdcMa,
            value: Value::Integer(HpMode::Balanced.default_edc_ma()),
        },
        Setting {
            feature: Feature::CpuPboScalar,
            value: Value::Integer(1),
        },
        Setting {
            feature: Feature::CpuCurveOptimizerAllCore,
            value: Value::Integer(0),
        },
        Setting {
            feature: Feature::CpuCurveOptimizerPerCore,
            value: Value::PerCore(zero_per_core(16)),
        },
        Setting {
            feature: Feature::WindowsBoostMode,
            value: Value::Choice(Choice::Aggressive),
        },
        Setting {
            feature: Feature::WindowsMinimumProcessorPercent,
            value: Value::Integer(5),
        },
        Setting {
            feature: Feature::WindowsMaximumProcessorPercent,
            value: Value::Integer(100),
        },
        Setting {
            feature: Feature::WindowsProcessorEppPercent,
            value: Value::Integer(50),
        },
        Setting {
            feature: Feature::WindowsCoreParkingMinPercent,
            value: Value::Integer(0),
        },
        Setting {
            feature: Feature::WindowsCoreParkingMaxPercent,
            value: Value::Integer(100),
        },
        Setting {
            feature: Feature::CpuCcdAffinity,
            value: Value::Integer(3),
        },
        Setting {
            feature: Feature::WindowsEnergyMode,
            value: Value::Choice(Choice::Balanced),
        },
        Setting {
            feature: Feature::WindowsPowerPlan,
            value: Value::Choice(Choice::Balanced),
        },
        Setting {
            feature: Feature::GraphicsMode,
            value: Value::Choice(Choice::Hybrid),
        },
    ];
    Some(DeviceDefaultsRecord {
        schema: DEFAULTS_SCHEMA,
        board: board.into(),
        bios: bios.into(),
        cpu: "AMD Ryzen 9 8940HX".into(),
        settings,
        modes,
        detected_unix_s: None,
    })
}

/// Resolves effective defaults for `board`/`bios`:
/// 1. Starts from verified reference defaults if this board/BIOS is known.
/// 2. Overlays every sanitized user/hardware-discovered setting and mode limit
///    from `stored`, so live measurements on any laptop always win.
/// 3. Fills top-level CPU limits from `modes.standard` if missing.
pub fn resolve_defaults(
    board: &str,
    bios: &str,
    stored: Option<&DeviceDefaultsRecord>,
) -> DeviceDefaultsRecord {
    let mut record =
        known_board_defaults(board, bios).unwrap_or_else(|| DeviceDefaultsRecord::new(board, bios));
    let mut map = record.map();
    if let Some(stored) = stored
        && stored.schema == DEFAULTS_SCHEMA
        && stored.board == board
        && stored.bios == bios
    {
        if !stored.cpu.is_empty() {
            record.cpu = stored.cpu.clone();
        }
        record.modes = record.modes.merge_over(&stored.modes);
        record.detected_unix_s = stored.detected_unix_s.or(record.detected_unix_s);
        for setting in &stored.settings {
            if let Some(clean) = sanitize_default(setting.feature, setting.value.clone()) {
                map.insert(setting.feature, clean);
            }
        }
    }
    // Keep top-level CPU default fields aligned with discovered Standard mode limits
    // whenever not explicitly populated yet.
    if let Some(standard) = &record.modes.standard {
        if let Some(v) = standard.ppt_mw {
            map.entry(Feature::CpuPptLimitMw)
                .or_insert(Value::Integer(v));
        }
        if let Some(v) = standard.slow_ppt_mw {
            map.entry(Feature::CpuSlowPptLimitMw)
                .or_insert(Value::Integer(v));
        }
        if let Some(v) = standard.tdc_ma {
            map.entry(Feature::CpuTdcMa).or_insert(Value::Integer(v));
        }
        if let Some(v) = standard.edc_ma {
            map.entry(Feature::CpuEdcMa).or_insert(Value::Integer(v));
        }
        if let Some(v) = standard.tctl_c {
            map.entry(Feature::CpuTemperatureLimitC)
                .or_insert(Value::Integer(v));
        }
    }
    // Always ensure zeroed factory offsets exist even on unknown boards.
    map.entry(Feature::CpuCurveOptimizerAllCore)
        .or_insert(Value::Integer(0));
    map.entry(Feature::CpuCurveOptimizerPerCore)
        .or_insert_with(|| Value::PerCore(zero_per_core(16)));
    map.entry(Feature::GpuCoreOffsetMhz)
        .or_insert(Value::Integer(0));
    map.entry(Feature::GpuMemoryOffsetMhz)
        .or_insert(Value::Integer(0));
    record.settings = map
        .into_iter()
        .map(|(feature, value)| Setting { feature, value })
        .collect();
    record
}

/// Merges newly discovered readings into an existing record (preserving fields
/// not re-read in this pass, sanitizing offsets, and updating mode limits).
pub fn merge_discovered(
    board: &str,
    bios: &str,
    cpu: &str,
    existing: Option<&DeviceDefaultsRecord>,
    readings: &[Setting],
    modes: Option<&ModeDefaults>,
    detected_unix_s: Option<u64>,
) -> DeviceDefaultsRecord {
    let mut record = existing
        .filter(|r| r.schema == DEFAULTS_SCHEMA && r.board == board && r.bios == bios)
        .cloned()
        .unwrap_or_else(|| DeviceDefaultsRecord::new(board, bios));
    if !cpu.is_empty() {
        record.cpu = cpu.to_owned();
    }
    let mut map: BTreeMap<Feature, Value> = record
        .settings
        .into_iter()
        .filter_map(|s| sanitize_default(s.feature, s.value).map(|v| (s.feature, v)))
        .collect();
    for reading in readings {
        if let Some(clean) = sanitize_default(reading.feature, reading.value.clone()) {
            map.insert(reading.feature, clean);
        }
    }
    if let Some(new_modes) = modes {
        record.modes = record.modes.merge_over(new_modes);
    }
    if detected_unix_s.is_some() {
        record.detected_unix_s = detected_unix_s;
    }
    record.settings = map
        .into_iter()
        .map(|(feature, value)| Setting { feature, value })
        .collect();
    record
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_neutralizes_polluted_tuning_offsets_and_manual_choices() {
        assert_eq!(
            sanitize_default(Feature::CpuCurveOptimizerAllCore, Value::Integer(10)),
            Some(Value::Integer(0))
        );
        assert_eq!(
            sanitize_default(Feature::GpuCoreOffsetMhz, Value::Integer(150)),
            Some(Value::Integer(0))
        );
        assert_eq!(
            sanitize_default(Feature::GpuMemoryOffsetMhz, Value::Integer(-200)),
            Some(Value::Integer(0))
        );
        let dirty_cores = Value::PerCore(vec![CoreDelta {
            ccd: 0,
            core: 0,
            steps: -25,
        }]);
        assert_eq!(
            sanitize_default(Feature::CpuCurveOptimizerPerCore, dirty_cores),
            Some(Value::PerCore(vec![CoreDelta {
                ccd: 0,
                core: 0,
                steps: 0,
            }]))
        );
        assert_eq!(
            sanitize_default(Feature::WindowsPowerPlan, Value::Choice(Choice::Manual)),
            None
        );
        assert_eq!(
            sanitize_default(Feature::CpuTdcMa, Value::Integer(108_000)),
            Some(Value::Integer(108_000))
        );
    }

    #[test]
    fn known_8e35_defaults_include_verified_standard_and_performance_tdc_edc() {
        let resolved = resolve_defaults("8E35", "F.13", None);
        assert_eq!(resolved.mode_tdc_edc(false), (Some(108_000), Some(160_000)));
        assert_eq!(resolved.mode_tdc_edc(true), (Some(108_000), Some(160_000)));
        assert_eq!(resolved.mode_slow_ppt_mw(false), Some(55_000));
        assert_eq!(resolved.mode_slow_ppt_mw(true), Some(80_000));
        assert_eq!(
            resolved.get(Feature::CpuTemperatureLimitC),
            Some(&Value::Integer(100))
        );
        assert_eq!(
            resolved.get(Feature::WindowsPowerPlan),
            Some(&Value::Choice(Choice::Balanced))
        );
        assert_eq!(
            resolved.get(Feature::CpuCurveOptimizerAllCore),
            Some(&Value::Integer(0))
        );
    }

    #[test]
    fn unknown_board_uses_live_discovered_standard_and_performance_tdc_edc() {
        let mut stored = DeviceDefaultsRecord::new("9999", "F.01");
        stored.modes = ModeDefaults {
            standard: Some(ModePowerDefaults {
                ppt_mw: Some(65_000),
                slow_ppt_mw: Some(45_000),
                tdc_ma: Some(75_000),
                edc_ma: Some(120_000),
                tctl_c: Some(95),
            }),
            performance: Some(ModePowerDefaults {
                ppt_mw: Some(90_000),
                slow_ppt_mw: Some(65_000),
                tdc_ma: Some(95_000),
                edc_ma: Some(150_000),
                tctl_c: Some(100),
            }),
            unleashed: None,
            quiet: None,
        };
        stored.settings.push(Setting {
            feature: Feature::DisplayRefreshRateHz,
            value: Value::Integer(165),
        });
        let resolved = resolve_defaults("9999", "F.01", Some(&stored));
        assert_eq!(resolved.mode_tdc_edc(false), (Some(75_000), Some(120_000)));
        assert_eq!(resolved.mode_tdc_edc(true), (Some(95_000), Some(150_000)));
        assert_eq!(resolved.get(Feature::CpuTdcMa), Some(&Value::Integer(75_000)));
        assert_eq!(resolved.get(Feature::CpuEdcMa), Some(&Value::Integer(120_000)));
        assert_eq!(
            resolved.get(Feature::DisplayRefreshRateHz),
            Some(&Value::Integer(165))
        );
    }

    #[test]
    fn legacy_device_defaults_json_deserializes_and_sanitizes_dirty_co() {
        let legacy = r#"{
            "schema": 1,
            "board": "8E35",
            "bios": "F.13",
            "settings": [
                {"feature": "cpu_tdc_ma", "value": {"kind": "integer", "value": 108000}},
                {"feature": "cpu_curve_optimizer_all_core", "value": {"kind": "integer", "value": 10}}
            ]
        }"#;
        let parsed: DeviceDefaultsRecord = serde_json::from_str(legacy).unwrap();
        let resolved = resolve_defaults("8E35", "F.13", Some(&parsed));
        assert_eq!(
            resolved.get(Feature::CpuCurveOptimizerAllCore),
            Some(&Value::Integer(0))
        );
        assert_eq!(resolved.get(Feature::CpuTdcMa), Some(&Value::Integer(108_000)));
    }
}
