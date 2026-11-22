//! A shared cooling policy is a request to the existing helper, never a direct
//! firmware transaction. Snapshots remain readable by older profile decoders.
use crate::{
    features::Feature,
    message::msg,
    profiles::{FanPoint, PerformanceProfile, Setting, Value, validate_id},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanPreset {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub board: String,
    pub bios: String,
    pub points: Vec<FanPoint>,
}
impl FanPreset {
    pub fn validate(&self) -> Result<(), String> {
        validate_id(&self.id)?;
        if self.schema != 1
            || self.name.trim().is_empty()
            || self.name.chars().count() > 96
            || self.name.chars().any(char::is_control)
            || self.board.is_empty()
            || self.bios.is_empty()
            || self.board.len() > 128
            || self.bios.len() > 128
        {
            return Err(msg("fan_profile.err.library", &[]));
        }
        validate_curve(&self.points)
    }
}
pub fn validate_curve(points: &[FanPoint]) -> Result<(), String> {
    Setting {
        feature: Feature::CpuFanCurve,
        value: Value::FanCurve(points.to_vec()),
    }
    .validate()
}
#[derive(Debug, Clone)]
pub struct Parts {
    pub direct: PerformanceProfile,
    pub shared: Option<Vec<FanPoint>>,
}
/// Only an equal CPU/GPU pair denotes the shared helper policy. A lone or
/// differing curve is unsupported intent; silently dropping it is not safe.
pub fn shared_curve(profile: &PerformanceProfile) -> Result<Option<Vec<FanPoint>>, String> {
    let value = |feature| {
        profile
            .settings
            .iter()
            .find(|s| s.feature == feature)
            .map(|s| &s.value)
    };
    let curve = match (value(Feature::CpuFanCurve), value(Feature::GpuFanCurve)) {
        (None, None) => None,
        (Some(Value::FanCurve(cpu)), Some(Value::FanCurve(gpu))) if cpu == gpu => {
            validate_curve(cpu)?;
            Some(cpu.clone())
        }
        _ => return Err(msg("fan_profile.err.shared_pair", &[])),
    };
    Ok(curve)
}
pub fn split(profile: &PerformanceProfile) -> Result<Parts, String> {
    profile.validate_intent()?;
    let shared = shared_curve(profile)?;
    let mut direct = profile.clone();
    direct
        .settings
        .retain(|s| !matches!(s.feature, Feature::CpuFanCurve | Feature::GpuFanCurve));
    if shared.is_some() && direct.effective().settings.is_empty() {
        return Err(msg("fan_profile.err.direct_required", &[]));
    }
    // Keep the canonical binding of unlinked legacy profiles unchanged. The
    // transaction's own for_settings matching scopes incidental GPU identity.
    Ok(Parts { direct, shared })
}
/// Sign-in applies a linked curve as the same helper request as a manual
/// Apply (user decision, 22 November 2026). Only a curve the helper cannot
/// take, a lone or differing CPU/GPU curve, is refused.
pub fn startup_allowed(profile: &PerformanceProfile, enabled: bool) -> Result<(), String> {
    if enabled && shared_curve(profile).is_err() {
        return Err(msg("fan_profile.err.startup", &[]));
    }
    Ok(())
}
/// The part of a profile the sign-in transaction plans and applies: the
/// direct controls without the linked curve, which is a helper request.
pub fn direct_part(profile: &PerformanceProfile) -> Result<PerformanceProfile, String> {
    Ok(split(profile)?.direct)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{DeviceBinding, PROFILE_SCHEMA};
    fn profile() -> PerformanceProfile {
        PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "fixture".into(),
            name: "Fixture".into(),
            binding: DeviceBinding {
                manufacturer: "HP".into(),
                board: "8E35".into(),
                bios: "F.13".into(),
                cpu_names: vec![],
                nvidia_uuid: None,
                nvidia_driver: None,
                nvidia_vbios: None,
            },
            settings: vec![Setting {
                feature: Feature::CpuTemperatureLimitC,
                value: Value::Integer(85),
            }],
        }
    }
    fn points() -> Vec<FanPoint> {
        vec![
            FanPoint {
                temperature_c: 30,
                duty_percent: 30,
            },
            FanPoint {
                temperature_c: 90,
                duty_percent: 100,
            },
        ]
    }
    #[test]
    fn paired_snapshot_is_separate_from_direct_controls_and_old_schema_still_decodes() {
        let mut p = profile();
        let baseline = p.clone();
        for feature in [Feature::CpuFanCurve, Feature::GpuFanCurve] {
            p.settings.push(Setting {
                feature,
                value: Value::FanCurve(points()),
            });
        }
        let decoded: PerformanceProfile =
            serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        let parts = split(&decoded).unwrap();
        assert_eq!(parts.direct, baseline);
        assert_eq!(parts.shared, Some(points()));
        // A valid shared pair is applied at sign-in like any other value.
        assert!(startup_allowed(&p, true).is_ok());
        assert!(startup_allowed(&baseline, true).is_ok());
        assert!(startup_allowed(&p, false).is_ok());
        assert_eq!(direct_part(&p).unwrap().settings, baseline.settings);
        // A lone curve has no helper meaning and stays refused.
        let mut lone = baseline.clone();
        lone.settings.push(Setting {
            feature: Feature::CpuFanCurve,
            value: Value::FanCurve(points()),
        });
        assert!(startup_allowed(&lone, true).is_err());
        assert!(startup_allowed(&lone, false).is_ok());
    }
    #[test]
    fn partial_mismatched_and_fan_only_profiles_fail_before_dispatch() {
        let mut p = profile();
        p.settings.push(Setting {
            feature: Feature::CpuFanCurve,
            value: Value::FanCurve(points()),
        });
        assert!(split(&p).is_err());
        let mut other = points();
        other[0].duty_percent = 40;
        p.settings.push(Setting {
            feature: Feature::GpuFanCurve,
            value: Value::FanCurve(other),
        });
        assert!(split(&p).is_err());
        p.settings[2].value = Value::FanCurve(points());
        p.settings.remove(0);
        assert!(split(&p).is_err());
    }
    #[test]
    fn unlinked_legacy_bindings_are_not_rewritten_by_partitioning() {
        let mut p = profile();
        p.binding.nvidia_uuid = Some("incidental legacy GPU".into());
        p.binding.nvidia_driver = Some("old driver".into());
        p.binding.nvidia_vbios = Some("old VBIOS".into());
        assert_eq!(split(&p).unwrap().direct, p);
    }
}
