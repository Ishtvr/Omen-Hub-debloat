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
                .nvidia_driver
                .as_ref()
                .is_none_or(|id| Some(id) == current.nvidia_driver.as_ref())
            && self
                .nvidia_vbios
                .as_ref()
                .is_none_or(|id| Some(id) == current.nvidia_vbios.as_ref())
    }
    /// CPU/Windows/iGPU-only profiles must not wake a discrete GPU just to check
    /// an incidental NVIDIA identity captured when the profile was created.
    pub fn for_settings(&self, settings: &[Setting]) -> Self {
        let mut relevant = self.clone();
        if !settings
            .iter()
            .any(|s| s.feature.descriptor().category == crate::features::Category::Gpu)
        {
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
            return Err(format!("{:?}: wrong value type", self.feature));
        }
        match &self.value {
            Value::Integer(value) => {
                // Serialization bounds only. Actual hardware limits come from the adapter.
                if *value < i64::from(i32::MIN) || *value > i64::from(u32::MAX) {
                    return Err("Numeric setting cannot be represented by an adapter".into());
                }
                let signed = matches!(
                    self.feature,
                    Feature::GpuCoreOffsetMhz
                        | Feature::GpuMemoryOffsetMhz
                        | Feature::GpuVoltageOffsetMv
                        | Feature::GpuMemoryVoltageOffsetMv
                        | Feature::CpuAllCoreVoltageOffsetMv
                        | Feature::IntelGpuVoltageOffsetMv
                        | Feature::IntelCacheVoltageOffsetMv
                        | Feature::IntelSystemAgentVoltageOffsetMv
                        | Feature::CpuCurveOptimizerAllCore
                        | Feature::GfxCurveOptimizer
                );
                if (!signed && *value < 0) || (signed && *value > i64::from(i32::MAX)) {
                    return Err("Numeric setting has an invalid sign or representation".into());
                }
                if descriptor.unit == Unit::Percent && !(0..=100).contains(value) {
                    return Err("Percentage must be in 0..100".into());
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
                    return Err("Choice does not belong to this feature".into());
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
                    return Err(
                        "V/F points must have increasing voltage and nondecreasing frequency"
                            .into(),
                    );
                }
            }
            Value::PerCore(points) => {
                let unique: BTreeSet<_> = points.iter().map(|p| (p.ccd, p.core)).collect();
                if points.is_empty() || points.len() > 256 || unique.len() != points.len() {
                    return Err("Per-core settings need unique core identities".into());
                }
            }
            Value::CoreRatios(values) => {
                if values.is_empty() || values.len() > 256 || values.contains(&0) {
                    return Err("Core ratios must be nonzero and bounded in count".into());
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
                    return Err(
                        "Fan curve needs ordered temperatures and nondecreasing duty".into(),
                    );
                }
            }
            Value::Colors(values) => {
                if values.is_empty() || values.len() > 128 {
                    return Err("Invalid color zone count".into());
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
        return Err("Profile ID must be 1..64 lowercase letters/digits/hyphens/underscores and cannot be a Windows device name".into());
    }
    Ok(())
}

impl PerformanceProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PROFILE_SCHEMA {
            return Err("Unsupported profile schema; no settings applied".into());
        }
        validate_id(&self.id)?;
        if self.name.trim().is_empty()
            || self.name.chars().count() > 96
            || self.name.chars().any(char::is_control)
        {
            return Err("Profile needs a nonempty name without control characters".into());
        }
        if self.binding.manufacturer.is_empty()
            || self.binding.board.is_empty()
            || self.binding.bios.is_empty()
        {
            return Err("Profile has no complete board/BIOS binding".into());
        }
        if self.settings.iter().any(|s| {
            matches!(
                s.feature.descriptor().category,
                crate::features::Category::Gpu
            )
        }) && [
            self.binding.nvidia_uuid.as_deref(),
            self.binding.nvidia_driver.as_deref(),
            self.binding.nvidia_vbios.as_deref(),
        ]
        .iter()
        .any(|id| id.is_none_or(str::is_empty))
        {
            return Err("NVIDIA settings require UUID, driver and VBIOS binding".into());
        }
        let unique: BTreeSet<_> = self.settings.iter().map(|s| s.feature).collect();
        if self.settings.len() > Feature::all().len() || unique.len() != self.settings.len() {
            return Err("Profile contains duplicate or excessive settings".into());
        }
        for setting in &self.settings {
            setting.validate()?;
        }
        Ok(())
    }

    /// Storage validation preserves readable legacy intent. Hardware preflight
    /// additionally rejects conflicting targets before journaling or any SET.
    pub fn validate_intent(&self) -> Result<(), String> {
        self.validate()?;
        let unique: BTreeSet<_> = self.settings.iter().map(|s| s.feature).collect();
        let legacy_boost = self
            .settings
            .iter()
            .find(|s| s.feature == Feature::GpuUnleashed);
        let boost = self
            .settings
            .iter()
            .find(|s| s.feature == Feature::GpuDynamicBoost);
        if matches!((legacy_boost,boost),(Some(a),Some(b)) if a.value!=b.value) {
            return Err(crate::message::msg(
                "profile.err.dynamic_boost_conflict",
                &[],
            ));
        }
        if unique.contains(&Feature::WindowsEnergyMode)
            && self.settings.iter().any(|s| {
                s.feature == Feature::WindowsPowerPlan && s.value != Value::Choice(Choice::Balanced)
            })
        {
            return Err(crate::message::msg("windows.err.overlay_plan", &[]));
        }
        if unique.contains(&Feature::GpuVfCurve) && unique.contains(&Feature::GpuCoreOffsetMhz) {
            return Err(
                "Choose either a full V/F curve or a global GPU core offset in one profile".into(),
            );
        }
        if unique.contains(&Feature::CpuCurveOptimizerAllCore)
            && unique.contains(&Feature::CpuCurveOptimizerPerCore)
        {
            return Err(crate::message::msg("profile.err.co_conflict", &[]));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_power_mode_conflicts_remain_readable_but_cannot_apply() {
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
        assert!(profile.validate().is_ok());
        assert!(profile.validate_intent().is_err());
        profile.settings[1].value = Value::Choice(Choice::Balanced);
        assert!(profile.validate_intent().is_ok());
    }
    #[test]
    fn all_core_and_per_core_targets_cannot_overwrite_each_other() {
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
        assert!(profile.validate().is_ok());
        assert!(profile.validate_intent().is_err());
        profile.settings.remove(0);
        assert!(profile.validate().is_ok());
    }
    #[test]
    fn a_curve_and_global_core_offset_cannot_address_the_same_domain() {
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
        profile.settings.push(Setting {
            feature: Feature::GpuCoreOffsetMhz,
            value: Value::Integer(0),
        });
        assert!(profile.validate().is_ok());
        assert!(profile.validate_intent().is_err());
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
