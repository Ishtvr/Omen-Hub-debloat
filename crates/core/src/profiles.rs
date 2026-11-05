//! Versioned, hardware-bound profile data. Saving a profile never applies it.
use crate::features::{Feature, Unit, ValueKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const PROFILE_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceBinding {
    pub manufacturer: String,
    pub board: String,
    pub bios: String,
    pub cpu_names: Vec<String>,
    pub nvidia_uuid: Option<String>,
    pub nvidia_driver: Option<String>,
    pub nvidia_vbios: Option<String>,
}

impl DeviceBinding {
    /// The NVIDIA driver version is recorded but not compared: driver updates
    /// must not block a preset. Each GPU adapter still gates writes on its own
    /// per-driver evidence and verifies readback with restore.
    pub fn matches(&self, current: &Self) -> bool {
        self.manufacturer == current.manufacturer
            && self.board == current.board
            && self.bios == current.bios
            && self.cpu_names == current.cpu_names
            && self
                .nvidia_uuid
                .as_ref()
                .is_none_or(|id| Some(id) == current.nvidia_uuid.as_ref())
            && self
                .nvidia_vbios
                .as_ref()
                .is_none_or(|id| Some(id) == current.nvidia_vbios.as_ref())
    }
    /// CPU/Windows/iGPU-only profiles must not wake a discrete GPU just to check
    /// an incidental NVIDIA identity captured when the profile was created.
    pub fn for_settings(&self, settings: &[Setting]) -> Self {
        let mut relevant = self.clone();
        if !settings.iter().any(|s| s.feature.needs_nvidia()) {
            relevant.nvidia_uuid = None;
            relevant.nvidia_driver = None;
            relevant.nvidia_vbios = None;
        }
        relevant
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    IntegratedOnly,
    Hybrid,
    DiscreteOnly,
    Disabled,
    Enabled,
    Aggressive,
    EfficientEnabled,
    EfficientAggressive,
    AggressiveAtGuaranteed,
    EfficientAggressiveAtGuaranteed,
    BestEfficiency,
    Balanced,
    BestPerformance,
    PowerSaver,
    HighPerformance,
    Quiet,
    Performance,
    Unleashed,
    Auto,
    Manual,
    Maximum,
    Static,
    FirmwareDefault,
    Breathing,
    Wave,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VfPoint {
    pub voltage_mv: u32,
    pub frequency_mhz: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreDelta {
    pub ccd: u8,
    pub core: u8,
    pub steps: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanPoint {
    pub temperature_c: u8,
    pub duty_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Value {
    Integer(i64),
    Boolean(bool),
    Choice(Choice),
    VfCurve(Vec<VfPoint>),
    PerCore(Vec<CoreDelta>),
    CoreRatios(Vec<u8>),
    FanCurve(Vec<FanPoint>),
    Colors(Vec<[u8; 3]>),
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Integer(_) => ValueKind::Integer,
            Self::Boolean(_) => ValueKind::Boolean,
            Self::Choice(_) => ValueKind::Choice,
            Self::VfCurve(_) => ValueKind::VfCurve,
            Self::PerCore(_) => ValueKind::PerCore,
            Self::CoreRatios(_) => ValueKind::CoreRatios,
            Self::FanCurve(_) => ValueKind::FanCurve,
            Self::Colors(_) => ValueKind::Colors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    pub feature: Feature,
    pub value: Value,
}

impl Setting {
    pub fn validate(&self) -> Result<(), String> {
        let descriptor = self.feature.descriptor();
        if self.value.kind() != descriptor.value_kind {
            return Err(crate::message::msg(
                "profile.err.value_type",
                &[&format!("{:?}", self.feature)],
            ));
        }
        match &self.value {
            Value::Integer(value) => {
                // Serialization bounds only. Actual hardware limits come from the adapter.
                if *value < i64::from(i32::MIN) || *value > i64::from(u32::MAX) {
                    return Err(crate::message::msg(
                        "profile.err.numeric_representation",
                        &[],
                    ));
                }
                let signed = matches!(self.feature, Feature::Retired(feature) if feature.signed())
                    || matches!(
                        self.feature,
                        Feature::GpuCoreOffsetMhz
                            | Feature::GpuMemoryOffsetMhz
                            | Feature::GpuVoltageOffsetMv
                            | Feature::GpuMemoryVoltageOffsetMv
                            | Feature::CpuAllCoreVoltageOffsetMv
                            | Feature::CpuCurveOptimizerAllCore
                            | Feature::GfxCurveOptimizer
                    );
                if (!signed && *value < 0) || (signed && *value > i64::from(i32::MAX)) {
                    return Err(crate::message::msg("profile.err.numeric_sign", &[]));
                }
                if descriptor.unit == Unit::Percent && !(0..=100).contains(value) {
                    return Err(crate::message::msg("profile.err.percentage", &[]));
                }
            }
            Value::Choice(value) => {
                let valid = match self.feature {
                    Feature::GraphicsMode => matches!(
                        value,
                        Choice::IntegratedOnly | Choice::Hybrid | Choice::DiscreteOnly
                    ),
                    Feature::WindowsBoostMode => matches!(
                        value,
                        Choice::Disabled
                            | Choice::Enabled
                            | Choice::Aggressive
                            | Choice::EfficientEnabled
                            | Choice::EfficientAggressive
                            | Choice::AggressiveAtGuaranteed
                            | Choice::EfficientAggressiveAtGuaranteed
                    ),
                    Feature::WindowsEnergyMode => matches!(
                        value,
                        Choice::BestEfficiency | Choice::Balanced | Choice::BestPerformance
                    ),
                    Feature::WindowsPowerPlan => matches!(
                        value,
                        Choice::PowerSaver | Choice::Balanced | Choice::HighPerformance
                    ),
                    Feature::CpuBoostProfile => matches!(
                        value,
                        Choice::Auto | Choice::PowerSaver | Choice::Performance
                    ),
                    Feature::OmenPerformanceMode => matches!(
                        value,
                        Choice::Quiet | Choice::Balanced | Choice::Performance | Choice::Unleashed
                    ),
                    Feature::OmenThermalMode => {
                        matches!(value, Choice::Auto | Choice::Manual | Choice::Maximum)
                    }
                    Feature::KeyboardEffect => matches!(
                        value,
                        Choice::Static
                            | Choice::FirmwareDefault
                            | Choice::Breathing
                            | Choice::Wave
                            | Choice::Off
                    ),
                    _ => false,
                };
                if !valid {
                    return Err(crate::message::msg("profile.err.choice", &[]));
                }
            }
            Value::VfCurve(points) => {
                if points.is_empty()
                    || points.len() > 2048
                    || points
                        .iter()
                        .any(|p| p.voltage_mv == 0 || p.frequency_mhz == 0)
                    || points.windows(2).any(|p| {
                        p[0].voltage_mv >= p[1].voltage_mv
                            || p[0].frequency_mhz > p[1].frequency_mhz
                    })
                {
                    return Err(crate::message::msg("profile.err.vf_order", &[]));
                }
            }
            Value::PerCore(points) => {
                let unique: BTreeSet<_> = points.iter().map(|p| (p.ccd, p.core)).collect();
                if points.is_empty() || points.len() > 256 || unique.len() != points.len() {
                    return Err(crate::message::msg("profile.err.per_core_unique", &[]));
                }
            }
            Value::CoreRatios(values) => {
                if values.is_empty() || values.len() > 256 || values.contains(&0) {
                    return Err(crate::message::msg("profile.err.ratios", &[]));
                }
            }
            Value::FanCurve(points) => {
                if points.len() < 2
                    || points.len() > crate::curve_editor::MAX_FAN_POINTS
                    || points
                        .iter()
                        .any(|p| p.duty_percent > 100 || !(0..=110).contains(&p.temperature_c))
                    || points.windows(2).any(|p| {
                        p[0].temperature_c >= p[1].temperature_c
                            || p[0].duty_percent > p[1].duty_percent
                    })
                {
                    return Err(crate::message::msg("profile.err.fan_order", &[]));
                }
            }
            Value::Colors(values) => {
                if values.is_empty() || values.len() > 128 {
                    return Err(crate::message::msg("profile.err.color_count", &[]));
                }
            }
            Value::Boolean(_) => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceProfile {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub binding: DeviceBinding,
    pub settings: Vec<Setting>,
}

pub fn validate_id(id: &str) -> Result<(), String> {
    let reserved = matches!(id, "con" | "prn" | "aux" | "nul")
        || (id.len() == 4
            && (id.starts_with("com") || id.starts_with("lpt"))
            && id.as_bytes()[3].is_ascii_digit());
    if id.is_empty()
        || id.len() > 64
        || reserved
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
    {
        return Err(crate::message::msg("profile.err.id", &[]));
    }
    Ok(())
}

impl PerformanceProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PROFILE_SCHEMA {
            return Err(crate::message::msg("profile.err.schema", &[]));
        }
        validate_id(&self.id)?;
        if self.name.trim().is_empty()
            || self.name.chars().count() > 96
            || self.name.chars().any(char::is_control)
        {
            return Err(crate::message::msg("profile.err.name", &[]));
        }
        if self.binding.manufacturer.is_empty()
            || self.binding.board.is_empty()
            || self.binding.bios.is_empty()
        {
            return Err(crate::message::msg("profile.err.binding", &[]));
        }
        if self.settings.iter().any(|s| s.feature.needs_nvidia())
            && [
                self.binding.nvidia_uuid.as_deref(),
                self.binding.nvidia_driver.as_deref(),
                self.binding.nvidia_vbios.as_deref(),
            ]
            .iter()
            .any(|id| id.is_none_or(str::is_empty))
        {
            return Err(crate::message::msg("profile.err.gpu_binding", &[]));
        }
        let unique: BTreeSet<_> = self.settings.iter().map(|s| s.feature).collect();
        if self.settings.len() > Feature::all().len() + crate::features::RetiredFeature::COUNT
            || unique.len() != self.settings.len()
        {
            return Err(crate::message::msg("profile.err.duplicates", &[]));
        }
        for setting in &self.settings {
            setting.validate()?;
        }
        Ok(())
    }

    /// Storage validation preserves readable legacy intent. Hardware preflight
    /// additionally rejects retired controls before journaling or any SET.
    /// Overlapping targets are not errors: see `effective`.
    pub fn validate_intent(&self) -> Result<(), String> {
        self.validate()?;
        if self.settings.iter().any(|s| s.feature.retired()) {
            return Err(crate::message::msg("profile.err.removed_intel", &[]));
        }
        crate::hp_mode::HpMode::shared_budget_of_profile(self)?;
        Ok(())
    }

    /// The settings a transaction applies. Overlapping targets follow a fixed
    /// precedence instead of blocking the preset (user decision, 29 October
    /// 2026): a full V/F curve wins over a global core offset, per-core Curve
    /// Optimizer over all-core and Dynamic Boost over its legacy key. Windows
    /// applies a power mode only on the Balanced plan, so a different plan in
    /// the preset wins over the power mode, and a power mode without a plan
    /// selects Balanced; the transaction captures and restores the original.
    pub fn effective(&self) -> PerformanceProfile {
        let mut profile = self.clone();
        let find = |feature| self.settings.iter().find(|s| s.feature == feature);
        // The shared budget (TPP) has no readback; Apply writes it beforehand,
        // outside the transaction. The HP mode is no longer a preset field
        // (30 October 2026); legacy entries are discarded.
        let mut overridden = vec![Feature::OmenPerformanceMode, Feature::HpSharedPowerMw];
        if find(Feature::GpuVfCurve).is_some() {
            overridden.push(Feature::GpuCoreOffsetMhz);
        }
        if find(Feature::CpuCurveOptimizerPerCore).is_some() {
            overridden.push(Feature::CpuCurveOptimizerAllCore);
        }
        if let (Some(legacy), Some(boost)) =
            (find(Feature::GpuUnleashed), find(Feature::GpuDynamicBoost))
        {
            if legacy.value != boost.value {
                overridden.push(Feature::GpuUnleashed);
            }
        }
        if find(Feature::WindowsEnergyMode).is_some() {
            match find(Feature::WindowsPowerPlan) {
                Some(plan) if plan.value != Value::Choice(Choice::Balanced) => {
                    overridden.push(Feature::WindowsEnergyMode);
                }
                Some(_) => {}
                None => profile.settings.push(Setting {
                    feature: Feature::WindowsPowerPlan,
                    value: Value::Choice(Choice::Balanced),
                }),
            }
        }
        profile
            .settings
            .retain(|s| !overridden.contains(&s.feature));
        profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_non_balanced_plan_wins_and_a_lone_power_mode_selects_balanced() {
        let mut profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "legacy".into(),
            name: "Legacy".into(),
            binding: DeviceBinding {
                manufacturer: "HP".into(),
                board: "8E35".into(),
                bios: "F.13".into(),
                cpu_names: vec![],
                nvidia_uuid: None,
                nvidia_driver: None,
                nvidia_vbios: None,
            },
            settings: vec![
                Setting {
                    feature: Feature::WindowsEnergyMode,
                    value: Value::Choice(Choice::BestEfficiency),
                },
                Setting {
                    feature: Feature::WindowsPowerPlan,
                    value: Value::Choice(Choice::HighPerformance),
                },
            ],
        };
        assert!(profile.validate_intent().is_ok());
        let features =
            |p: &PerformanceProfile| p.settings.iter().map(|s| s.feature).collect::<Vec<_>>();
        assert_eq!(features(&profile.effective()), [Feature::WindowsPowerPlan]);
        profile.settings[1].value = Value::Choice(Choice::Balanced);
        assert_eq!(profile.effective(), profile);
        profile.settings.pop();
        let lone = profile.effective();
        assert_eq!(
            lone.settings,
            [
                profile.settings[0].clone(),
                Setting {
                    feature: Feature::WindowsPowerPlan,
                    value: Value::Choice(Choice::Balanced),
                }
            ]
        );
        assert!(lone.validate().is_ok());
        assert_eq!(lone.effective(), lone);
    }
    #[test]
    fn per_core_curve_optimizer_wins_over_all_core() {
        let mut profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "cpu".into(),
            name: "CPU".into(),
            binding: DeviceBinding {
                manufacturer: "HP".into(),
                board: "8E35".into(),
                bios: "F.13".into(),
                cpu_names: vec!["CPU".into()],
                nvidia_uuid: None,
                nvidia_driver: None,
                nvidia_vbios: None,
            },
            settings: vec![Setting {
                feature: Feature::CpuCurveOptimizerAllCore,
                value: Value::Integer(-19),
            }],
        };
        assert!(profile.validate().is_ok());
        profile.settings.push(Setting {
            feature: Feature::CpuCurveOptimizerPerCore,
            value: Value::PerCore(vec![CoreDelta {
                ccd: 0,
                core: 0,
                steps: -19,
            }]),
        });
        assert!(profile.validate_intent().is_ok());
        let effective = profile.effective();
        assert_eq!(effective.settings, profile.settings[1..]);
        profile.settings.remove(0);
        assert_eq!(profile.effective(), profile);
    }
    #[test]
    fn a_curve_wins_over_a_global_core_offset() {
        let mut profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "curve".into(),
            name: "Curve".into(),
            binding: DeviceBinding {
                manufacturer: "HP".into(),
                board: "8E35".into(),
                bios: "F.13".into(),
                cpu_names: vec![],
                nvidia_uuid: Some("GPU-test".into()),
                nvidia_driver: Some("driver".into()),
                nvidia_vbios: Some("vbios".into()),
            },
            settings: vec![Setting {
                feature: Feature::GpuVfCurve,
                value: Value::VfCurve(vec![VfPoint {
                    voltage_mv: 900,
                    frequency_mhz: 1200,
                }]),
            }],
        };
        assert!(profile.validate().is_ok());
        profile.settings.push(Setting {
            feature: Feature::GpuMemoryOffsetMhz,
            value: Value::Integer(-24),
        });
        assert!(profile.validate().is_ok());
        let without_core = profile.settings.clone();
        profile.settings.push(Setting {
            feature: Feature::GpuCoreOffsetMhz,
            value: Value::Integer(0),
        });
        assert!(profile.validate_intent().is_ok());
        assert_eq!(profile.effective().settings, without_core);
        profile.settings.push(Setting {
            feature: Feature::GpuUnleashed,
            value: Value::Boolean(true),
        });
        profile.settings.push(Setting {
            feature: Feature::GpuDynamicBoost,
            value: Value::Boolean(false),
        });
        let boost = profile.effective();
        assert!(
            boost
                .settings
                .iter()
                .all(|s| s.feature != Feature::GpuUnleashed)
        );
        assert!(
            boost
                .settings
                .iter()
                .any(|s| s.feature == Feature::GpuDynamicBoost)
        );
        // A legacy HP mode entry is discarded; cTGP is its own field.
        profile.settings.push(Setting {
            feature: Feature::OmenPerformanceMode,
            value: Value::Choice(Choice::Unleashed),
        });
        profile.settings.push(Setting {
            feature: Feature::GpuCustomTgp,
            value: Value::Boolean(true),
        });
        assert!(profile.validate_intent().is_ok());
        let effective = profile.effective();
        assert!(
            effective
                .settings
                .iter()
                .all(|s| s.feature != Feature::OmenPerformanceMode)
        );
        assert!(
            effective
                .settings
                .iter()
                .any(|s| s.feature == Feature::GpuCustomTgp)
        );
    }
    #[test]
    fn non_gpu_profiles_do_not_require_an_awake_nvidia_device() {
        let bound = DeviceBinding {
            manufacturer: "HP".into(),
            board: "8E35".into(),
            bios: "F.13".into(),
            cpu_names: vec!["CPU".into()],
            nvidia_uuid: Some("GPU-test".into()),
            nvidia_driver: Some("driver".into()),
            nvidia_vbios: Some("vbios".into()),
        };
        let sleeping = DeviceBinding {
            nvidia_uuid: None,
            nvidia_driver: None,
            nvidia_vbios: None,
            ..bound.clone()
        };
        let windows = [Setting {
            feature: Feature::WindowsBoostMode,
            value: Value::Choice(Choice::EfficientEnabled),
        }];
        assert!(bound.for_settings(&windows).matches(&sleeping));
        // cTGP and Dynamic Boost are HP firmware flags, not NVIDIA writes.
        let flags = [
            Setting {
                feature: Feature::GpuCustomTgp,
                value: Value::Boolean(true),
            },
            Setting {
                feature: Feature::GpuUnleashed,
                value: Value::Boolean(false),
            },
        ];
        assert!(bound.for_settings(&flags).matches(&sleeping));
        let flagged = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "flags".into(),
            name: "Flags".into(),
            binding: sleeping.clone(),
            settings: flags.to_vec(),
        };
        assert!(flagged.validate().is_ok());
        let gpu = [Setting {
            feature: Feature::GpuCoreOffsetMhz,
            value: Value::Integer(0),
        }];
        assert!(!bound.for_settings(&gpu).matches(&sleeping));
        assert!(!bound.for_settings(&windows).matches(&DeviceBinding {
            bios: "F.new".into(),
            ..sleeping
        }));
    }
    #[test]
    fn driver_updates_keep_gpu_presets_bound_but_hardware_changes_do_not() {
        let bound = DeviceBinding {
            manufacturer: "HP".into(),
            board: "8E35".into(),
            bios: "F.13".into(),
            cpu_names: vec!["CPU".into()],
            nvidia_uuid: Some("GPU-test".into()),
            nvidia_driver: Some("617.14".into()),
            nvidia_vbios: Some("vbios".into()),
        };
        let gpu = [Setting {
            feature: Feature::GpuCoreOffsetMhz,
            value: Value::Integer(0),
        }];
        let updated = DeviceBinding {
            nvidia_driver: Some("617.42".into()),
            ..bound.clone()
        };
        assert!(bound.for_settings(&gpu).matches(&updated));
        for changed in [
            DeviceBinding {
                bios: "F.14".into(),
                ..updated.clone()
            },
            DeviceBinding {
                nvidia_vbios: Some("other".into()),
                ..updated.clone()
            },
            DeviceBinding {
                nvidia_uuid: Some("GPU-other".into()),
                ..updated.clone()
            },
        ] {
            assert!(!bound.for_settings(&gpu).matches(&changed));
        }
    }

    #[test]
    fn traversal_reserved_names_and_case_collisions_are_rejected() {
        for id in [
            "../other", "x/y", "x\\y", "con", "lpt1", "com9", "Test", "a.json", "",
        ] {
            assert!(validate_id(id).is_err(), "{id}");
        }
        assert!(validate_id("my-gaming_2").is_ok());
    }

    #[test]
    fn choice_numeric_types_and_units_are_not_interchangeable() {
        for setting in [
            Setting {
                feature: Feature::GpuCoreOffsetMhz,
                value: Value::Boolean(true),
            },
            Setting {
                feature: Feature::WindowsBoostMode,
                value: Value::Choice(Choice::IntegratedOnly),
            },
            Setting {
                feature: Feature::KeyboardBrightness,
                value: Value::Integer(228),
            },
            Setting {
                feature: Feature::CpuPptLimitMw,
                value: Value::Integer(-1),
            },
            Setting {
                feature: Feature::GpuCoreOffsetMhz,
                value: Value::Integer(i64::from(u32::MAX)),
            },
        ] {
            assert!(setting.validate().is_err());
        }
        assert!(
            Setting {
                feature: Feature::CpuCurveOptimizerAllCore,
                value: Value::Integer(-20)
            }
            .validate()
            .is_ok()
        );
        assert!(
            Setting {
                feature: Feature::CpuAllCoreVoltageOffsetMv,
                value: Value::Integer(-20)
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn curve_optimizer_steps_and_millivolts_are_distinct_features() {
        assert_eq!(
            Feature::CpuCurveOptimizerAllCore.descriptor().unit,
            Unit::CurveStep
        );
        assert_eq!(
            Feature::CpuAllCoreVoltageOffsetMv.descriptor().unit,
            Unit::Millivolt
        );
        assert_ne!(Feature::GpuCoreOffsetMhz, Feature::GpuVoltageOffsetMv);
    }
}
