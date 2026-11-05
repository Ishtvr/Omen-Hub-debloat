//! Decode-only compatibility for removed settings. These entries are never
//! advertised in the catalog, offered by the UI or accepted for new CLI edits.
//! Keeping their original spelling preserves old profile/journal digests until
//! the user explicitly saves a cleaned preset.
use super::{Unit, ValueKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetiredFeature {
    IntelPl1Mw,
    IntelPl2Mw,
    IntelPl4Mw,
    IntelIccMaxMa,
    IntelCpuPowerBalance,
    IntelGpuPowerBalance,
    IntelGpuVoltageOffsetMv,
    IntelCacheVoltageOffsetMv,
    IntelSystemAgentVoltageOffsetMv,
    IntelCoreRatios,
}

impl RetiredFeature {
    pub(crate) const COUNT: usize = 10;

    pub(super) fn kind(self) -> ValueKind {
        if self == Self::IntelCoreRatios {
            ValueKind::CoreRatios
        } else {
            ValueKind::Integer
        }
    }

    pub(super) fn unit(self) -> Unit {
        match self {
            Self::IntelPl1Mw | Self::IntelPl2Mw | Self::IntelPl4Mw => Unit::Milliwatt,
            Self::IntelIccMaxMa => Unit::Milliampere,
            Self::IntelGpuVoltageOffsetMv
            | Self::IntelCacheVoltageOffsetMv
            | Self::IntelSystemAgentVoltageOffsetMv => Unit::Millivolt,
            _ => Unit::None,
        }
    }

    pub(crate) fn signed(self) -> bool {
        self.unit() == Unit::Millivolt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        features::Feature,
        profiles::{Setting, Value},
    };

    #[test]
    fn retired_settings_round_trip_exactly_but_are_absent_from_the_catalog() {
        let cases = [
            ("intel_pl1_mw", Value::Integer(45000)),
            ("intel_pl2_mw", Value::Integer(65000)),
            ("intel_pl4_mw", Value::Integer(90000)),
            ("intel_icc_max_ma", Value::Integer(90000)),
            ("intel_cpu_power_balance", Value::Integer(0)),
            ("intel_gpu_power_balance", Value::Integer(0)),
            ("intel_gpu_voltage_offset_mv", Value::Integer(-20)),
            ("intel_cache_voltage_offset_mv", Value::Integer(-20)),
            ("intel_system_agent_voltage_offset_mv", Value::Integer(-20)),
            ("intel_core_ratios", Value::CoreRatios(vec![48, 47, 46])),
        ];
        assert_eq!(cases.len(), RetiredFeature::COUNT);
        for (name, value) in cases {
            let raw = format!(
                "{{\"feature\":\"{name}\",\"value\":{}}}",
                serde_json::to_string(&value).unwrap()
            );
            let setting: Setting = serde_json::from_str(&raw).unwrap();
            assert!(setting.feature.retired());
            assert_eq!(setting.value, value);
            setting.validate().unwrap();
            assert_eq!(serde_json::to_string(&setting).unwrap(), raw);
            assert!(!Feature::all().contains(&setting.feature));
        }
        for feature in Feature::all() {
            assert!(!feature.retired());
            let raw = serde_json::to_string(feature).unwrap();
            assert_eq!(serde_json::from_str::<Feature>(&raw).unwrap(), *feature);
            assert!(!raw.contains("intel_"));
        }
    }

    #[test]
    fn compatibility_does_not_accept_unknown_keys_or_invalid_legacy_values() {
        for raw in [
            "\"intel_pl3_mw\"",
            "\"unknown\"",
            "{\"retired\":\"intel_pl1_mw\"}",
        ] {
            assert!(serde_json::from_str::<Feature>(raw).is_err());
        }
        let feature = serde_json::from_str::<Feature>("\"intel_pl1_mw\"").unwrap();
        assert!(
            Setting {
                feature,
                value: Value::Integer(-1)
            }
            .validate()
            .is_err()
        );
        let feature = serde_json::from_str::<Feature>("\"intel_core_ratios\"").unwrap();
        assert!(
            Setting {
                feature,
                value: Value::CoreRatios(vec![0])
            }
            .validate()
            .is_err()
        );
    }
}
