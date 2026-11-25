//! Fixed HP notebook performance-mode payloads for board 8E35 / BIOS F.13.
//! Values mirror the installed OMEN Gaming Hub 1101.2609.3.0 (Khalilah
//! DRG_N22X4X6 resource, docs/models/8e35-hub-modes.md). The BIOS has no GET
//! for the thermal mode, so the only restore is an explicit Balanced write;
//! GPU flags and the CPU power limit are read back.
use serde::{Deserialize, Serialize};

/// `PerformanceControlHelper.SetFanMode`: [0xFF, mode, bios fan owner, 0].
pub const THERMAL_MODE_TYPE: u32 = 0x1A;
/// `PowerControlBase.SetTgpPpabAsync`: [cTGP, PPAB, D-state, GPS 87 °C].
pub const GPU_POWER_TYPE: u32 = 0x22;
/// `PowerControlBase.SetConcurrentTdp`: [FF, FF, FF, shared CPU+GPU watts];
/// `SetPowerLimit`: [PL1, PL1, FF, FF] (measured: byte 0 slow, byte 1 STAPM).
pub const POWER_LIMITS_TYPE: u32 = 0x29;

/// Inner mode bytes the Hub sends on this notebook (`SetFanMode` with
/// `doMap = false`): Balanced slider L0 and centre L2, Performance L7,
/// Unleashed L8 = Extreme. L2, L7 and L8 appear in the Hub's own logs.
const QUIET_MODE: u8 = 0x10;
const BALANCED_MODE: u8 = 0x30;
const PERFORMANCE_MODE: u8 = 0x31;
const UNLEASHED_MODE: u8 = 0x04;
/// CPU PL1 the Hub writes after each mode change: `PL1DefaultValue` for
/// Balanced (and its slider), `NbPL1UpperBoundPerformance` for Performance,
/// the `UnleashedModePowerLimit1` default for Unleashed.
const BALANCED_PL1_W: u8 = 55;
const PERFORMANCE_PL1_W: u8 = 60;
const UNLEASHED_PL1_W: u8 = 80;
/// Hard-coded GPU temperature target in the Hub's TGP/PPAB write.
const GPS_MAX_TEMPERATURE: u8 = 87;
/// `GpuDState.D1`, the only state the Hub writes outside IR-overheat handling.
const D1: u8 = 1;
/// Upper end of the 8E35 TPP range; the Hub's Unleashed default.
pub const SHARED_BUDGET_MAX_W: u8 = 72;
/// Lowest firmware default we accept as a plausible restore value.
const SHARED_BUDGET_MIN_W: u8 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HpMode {
    /// The Hub's Balanced slider at its quietest step (L0); 8E35 has no
    /// separate Quiet/Cool byte.
    Quiet,
    Balanced,
    Performance,
    Unleashed,
}

impl HpMode {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "quiet" => Ok(Self::Quiet),
            "balanced" => Ok(Self::Balanced),
            "performance" => Ok(Self::Performance),
            "unleashed" => Ok(Self::Unleashed),
            _ => Err(format!("Unknown HP mode {text:?}")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
            Self::Unleashed => "unleashed",
        }
    }
    /// The mode a preset writes before its own settings (user decision, 30
    /// October 2026): Dynamic Boost on switches to Unleashed, where the NPCF
    /// table gives the GPU its cTGP offset and the preset's TPP (measured
    /// 100–112 W); on battery, where the Hub offers no Unleashed, Performance.
    /// Off selects Balanced (80 W; Balanced with Dynamic Boost enforces 65 W).
    /// A preset without a Dynamic Boost field leaves the mode unchanged.
    pub fn of_preset(profile: &crate::profiles::PerformanceProfile, ac: bool) -> Option<Self> {
        Self::of_settings(&profile.effective().settings, ac)
    }
    /// `of_preset` for already effective (applied) settings.
    pub fn of_settings(settings: &[crate::profiles::Setting], ac: bool) -> Option<Self> {
        use crate::{features::Feature, profiles::Value};
        let boost = [Feature::GpuDynamicBoost, Feature::GpuUnleashed]
            .iter()
            .find_map(|f| settings.iter().find(|s| s.feature == *f))?;
        Some(match (&boost.value, ac) {
            (Value::Boolean(true), true) => Self::Unleashed,
            (Value::Boolean(true), false) => Self::Performance,
            _ => Self::Balanced,
        })
    }
    /// The preset's sustained CPU limit as the firmware PL1 of a mode, in
    /// whole watts. A mode's `0x29` PL1 is kept by the firmware and would
    /// overwrite the preset's SMU value (measured 30 October 2026: Unleashed
    /// 80 W against a 60 W preset), so the preset's own value is written there
    /// too. `None` when the preset leaves Slow PPT to the mode.
    pub fn cpu_pl1_of_profile(profile: &crate::profiles::PerformanceProfile) -> Option<u8> {
        profile
            .effective()
            .settings
            .iter()
            .find(|s| s.feature == crate::features::Feature::CpuSlowPptLimitMw)
            .and_then(|s| match s.value {
                crate::profiles::Value::Integer(mw)
                    if mw % 1000 == 0 && (1..=254).contains(&(mw / 1000)) =>
                {
                    Some((mw / 1000) as u8)
                }
                _ => None,
            })
    }
    /// Default firmware PL1 in whole watts for each HP mode.
    fn default_pl1_w(mode: Self) -> u8 {
        match mode {
            Self::Quiet | Self::Balanced => BALANCED_PL1_W,
            Self::Performance => PERFORMANCE_PL1_W,
            Self::Unleashed => UNLEASHED_PL1_W,
        }
    }
    /// Effective firmware PL1 for a preset: explicit Slow PPT if set; otherwise
    /// if Fast PPT is below the mode's default PL1 (e.g. 30 W in Balanced/55 W),
    /// cap PL1 to Fast PPT so the mode's default does not invert Slow > Fast.
    pub fn effective_pl1_for_preset(
        profile: &crate::profiles::PerformanceProfile,
        mode: Self,
    ) -> Option<u8> {
        if let Some(w) = Self::cpu_pl1_of_profile(profile) {
            return Some(w);
        }
        profile
            .effective()
            .settings
            .iter()
            .find(|s| s.feature == crate::features::Feature::CpuPptLimitMw)
            .and_then(|s| match s.value {
                crate::profiles::Value::Integer(mw)
                    if mw % 1000 == 0 && (15..=254).contains(&(mw / 1000)) =>
                {
                    let w = (mw / 1000) as u8;
                    (w < Self::default_pl1_w(mode)).then_some(w)
                }
                _ => None,
            })
    }
    /// The shared CPU+GPU budget (TPP) a preset selects, in whole watts. It is
    /// written after the HP mode and before the transaction; the firmware has
    /// no readback. Only implausible values are refused.
    pub fn shared_budget_of_profile(
        profile: &crate::profiles::PerformanceProfile,
    ) -> Result<Option<u8>, String> {
        let Some(setting) = profile
            .settings
            .iter()
            .find(|s| s.feature == crate::features::Feature::HpSharedPowerMw)
        else {
            return Ok(None);
        };
        match setting.value {
            crate::profiles::Value::Integer(mw)
                if mw % 1000 == 0 && (1..=254).contains(&(mw / 1000)) =>
            {
                Ok(Some((mw / 1000) as u8))
            }
            _ => Err(crate::message::msg("hp.err.tpp_range", &[])),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModePlan {
    pub mode: HpMode,
    pub thermal: [u8; 4],
    pub gpu_power: [u8; 4],
    pub shared_budget: [u8; 4],
    /// CPU PL1 in whole watts as [slow, STAPM, FF, FF].
    pub cpu_power: [u8; 4],
    pub expect_custom_tgp: bool,
    pub expect_ppab: bool,
}

impl ModePlan {
    /// The PL1 this mode writes, in watts.
    pub fn pl1_w(&self) -> u8 {
        self.cpu_power[0]
    }
}

/// The `0x1A` payload of `mode` alone; "Keep preset" rewrites only this part
/// of a Dynamic Boost preset.
pub fn thermal_payload(mode: HpMode) -> [u8; 4] {
    let inner = match mode {
        HpMode::Quiet => QUIET_MODE,
        HpMode::Balanced => BALANCED_MODE,
        HpMode::Performance => PERFORMANCE_MODE,
        HpMode::Unleashed => UNLEASHED_MODE,
    };
    [0xFF, inner, 0, 0]
}

/// Highest enforced GPU limit of a locked GPU: Balanced enforces 65 W with
/// Dynamic Boost on and the 80 W driver default without (measured 30 October
/// 2026); Performance gives at least 85 W and Unleashed up to 115 W.
pub const LOCKED_GPU_LIMIT_MW: u32 = 80_000;

/// Whether a GPU under load runs at a locked limit, i.e. the mode of a
/// Dynamic Boost preset was lost. Idle readings never count.
pub fn gpu_release_lost(enforced_limit_mw: u32, draw_mw: u32) -> bool {
    enforced_limit_mw <= LOCKED_GPU_LIMIT_MW && draw_mw.saturating_add(5_000) >= enforced_limit_mw
}

/// Diagnostic only: the `0x1A` payload of the Hub's Performance slider steps
/// L5 (`0x11`) and L6 (`0x21`) from its `SetFanMode` code. They are not UI
/// modes; GPU flags, TPP and PL1 stay as they are.
pub fn performance_step_payload(step: &str) -> Result<[u8; 4], String> {
    match step {
        "l5" => Ok([0xFF, 0x11, 0, 0]),
        "l6" => Ok([0xFF, 0x21, 0, 0]),
        _ => Err("Unknown Performance step; use l5 or l6".into()),
    }
}

/// `default_shared_budget_w` is SystemDesignData byte 8 (45 W on 8E35/F.13);
/// every mode but Unleashed writes it because the Hub never resets TPP itself
/// (Performance's own TPP, minimum plus offset 0, is the same 45 W).
/// `ac` selects the Hub's battery variant of the GPU flags.
pub fn plan(mode: HpMode, default_shared_budget_w: u8, ac: bool) -> Result<ModePlan, String> {
    if !(SHARED_BUDGET_MIN_W..=SHARED_BUDGET_MAX_W).contains(&default_shared_budget_w) {
        return Err(format!(
            "Firmware shared-budget default {default_shared_budget_w} W is outside {SHARED_BUDGET_MIN_W}…{SHARED_BUDGET_MAX_W} W"
        ));
    }
    let budget = [0xFF, 0xFF, 0xFF, default_shared_budget_w];
    let pl1 = |w: u8| [w, w, 0xFF, 0xFF];
    Ok(match mode {
        HpMode::Unleashed => ModePlan {
            mode,
            thermal: thermal_payload(mode),
            gpu_power: [1, 1, D1, GPS_MAX_TEMPERATURE],
            shared_budget: [0xFF, 0xFF, 0xFF, SHARED_BUDGET_MAX_W],
            cpu_power: pl1(UNLEASHED_PL1_W),
            expect_custom_tgp: true,
            expect_ppab: true,
        },
        // cTGP and Dynamic Boost on, on AC and on battery.
        HpMode::Performance => ModePlan {
            mode,
            thermal: thermal_payload(mode),
            gpu_power: [1, 1, D1, GPS_MAX_TEMPERATURE],
            shared_budget: budget,
            cpu_power: pl1(PERFORMANCE_PL1_W),
            expect_custom_tgp: true,
            expect_ppab: true,
        },
        // Balanced: Dynamic Boost (PPAB) on as in the Hub, and cTGP on too
        // (user decision, 30 October 2026; the Hub's Balanced leaves it off).
        // Only Quiet keeps cTGP off.
        HpMode::Balanced => ModePlan {
            mode,
            thermal: thermal_payload(mode),
            gpu_power: [1, 1, D1, GPS_MAX_TEMPERATURE],
            shared_budget: budget,
            cpu_power: pl1(BALANCED_PL1_W),
            expect_custom_tgp: true,
            expect_ppab: true,
        },
        // Slider steps L0/L1/L3/L4 turn Dynamic Boost off on AC only.
        HpMode::Quiet => ModePlan {
            mode,
            thermal: thermal_payload(mode),
            gpu_power: [0, u8::from(!ac), D1, GPS_MAX_TEMPERATURE],
            shared_budget: budget,
            cpu_power: pl1(BALANCED_PL1_W),
            expect_custom_tgp: false,
            expect_ppab: !ac,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_shared_budget_is_whole_plausible_watts() {
        use crate::{
            features::Feature,
            profiles::{DeviceBinding, PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
        };
        let preset = |value| PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "tpp".into(),
            name: "TPP".into(),
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
                feature: Feature::HpSharedPowerMw,
                value: Value::Integer(value),
            }],
        };
        assert_eq!(
            HpMode::shared_budget_of_profile(&preset(72_000)),
            Ok(Some(72))
        );
        assert_eq!(
            HpMode::shared_budget_of_profile(&preset(100_000)),
            Ok(Some(100))
        );
        for absurd in [0, 255_000, 72_500] {
            assert!(HpMode::shared_budget_of_profile(&preset(absurd)).is_err());
            assert!(preset(absurd).validate_intent().is_err());
        }
        assert!(preset(72_000).effective().settings.is_empty());
    }

    #[test]
    fn dynamic_boost_selects_unleashed_and_off_selects_balanced() {
        use crate::{
            features::Feature,
            profiles::{DeviceBinding, PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
        };
        let preset = |settings: Vec<(Feature, bool)>| PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "boost".into(),
            name: "Boost".into(),
            binding: DeviceBinding {
                manufacturer: "HP".into(),
                board: "8E35".into(),
                bios: "F.13".into(),
                cpu_names: vec![],
                nvidia_uuid: None,
                nvidia_driver: None,
                nvidia_vbios: None,
            },
            settings: settings
                .into_iter()
                .map(|(feature, on)| Setting {
                    feature,
                    value: Value::Boolean(on),
                })
                .collect(),
        };
        let on = preset(vec![(Feature::GpuUnleashed, true)]);
        assert_eq!(HpMode::of_preset(&on, true), Some(HpMode::Unleashed));
        assert_eq!(HpMode::of_preset(&on, false), Some(HpMode::Performance));
        let off = preset(vec![(Feature::GpuUnleashed, false)]);
        assert_eq!(HpMode::of_preset(&off, true), Some(HpMode::Balanced));
        // cTGP alone selects nothing.
        let ctgp = preset(vec![(Feature::GpuCustomTgp, true)]);
        assert_eq!(HpMode::of_preset(&ctgp, true), None);
        // The newer key wins over a conflicting legacy one.
        let both = preset(vec![
            (Feature::GpuUnleashed, true),
            (Feature::GpuDynamicBoost, false),
        ]);
        assert_eq!(HpMode::of_preset(&both, true), Some(HpMode::Balanced));
        // The preset's Slow PPT becomes the firmware PL1; fractions are left to the SMU.
        let mut slow = preset(vec![(Feature::GpuUnleashed, true)]);
        assert_eq!(HpMode::cpu_pl1_of_profile(&slow), None);
        for (mw, pl1) in [(60_000, Some(60)), (60_500, None), (300_000, None)] {
            slow.settings
                .retain(|s| s.feature != Feature::CpuSlowPptLimitMw);
            slow.settings.push(Setting {
                feature: Feature::CpuSlowPptLimitMw,
                value: Value::Integer(mw),
            });
            assert_eq!(HpMode::cpu_pl1_of_profile(&slow), pl1);
        }
    }

    #[test]
    fn only_a_capped_locked_limit_counts_as_a_lost_mode() {
        // Balanced with Dynamic Boost (65 W) and the driver default (80 W) under load.
        assert!(gpu_release_lost(65_000, 64_100));
        assert!(gpu_release_lost(80_000, 76_200));
        // Idle at a locked limit, Performance (85 W) and Unleashed are not lost.
        assert!(!gpu_release_lost(65_000, 12_000));
        assert!(!gpu_release_lost(85_000, 84_000));
        assert!(!gpu_release_lost(107_000, 106_000));
        for mode in [
            HpMode::Quiet,
            HpMode::Balanced,
            HpMode::Performance,
            HpMode::Unleashed,
        ] {
            assert_eq!(plan(mode, 45, true).unwrap().thermal, thermal_payload(mode));
        }
    }

    #[test]
    fn performance_steps_are_only_the_hub_slider_bytes() {
        assert_eq!(performance_step_payload("l5"), Ok([0xFF, 0x11, 0, 0]));
        assert_eq!(performance_step_payload("l6"), Ok([0xFF, 0x21, 0, 0]));
        for other in ["l7", "l8", "0x04", ""] {
            assert!(performance_step_payload(other).is_err());
        }
    }

    #[test]
    fn unleashed_matches_the_hub_payloads() {
        let p = plan(HpMode::Unleashed, 45, true).unwrap();
        assert_eq!(p.thermal, [0xFF, 0x04, 0, 0]);
        assert_eq!(p.gpu_power, [1, 1, 1, 0x57]);
        assert_eq!(p.shared_budget, [0xFF, 0xFF, 0xFF, 72]);
        assert_eq!(p.cpu_power, [80, 80, 0xFF, 0xFF]);
        assert!(p.expect_custom_tgp && p.expect_ppab);
    }

    #[test]
    fn balanced_restores_firmware_budget_with_ctgp_on() {
        let p = plan(HpMode::Balanced, 45, true).unwrap();
        assert_eq!(p.thermal, [0xFF, 0x30, 0, 0]);
        assert_eq!(p.gpu_power, [1, 1, 1, 0x57]);
        assert_eq!(p.shared_budget, [0xFF, 0xFF, 0xFF, 45]);
        assert_eq!(p.cpu_power, [55, 55, 0xFF, 0xFF]);
        assert!(p.expect_custom_tgp && p.expect_ppab);
    }

    #[test]
    fn performance_is_l7_with_60_watt_pl1() {
        for ac in [true, false] {
            let p = plan(HpMode::Performance, 45, ac).unwrap();
            assert_eq!(p.thermal, [0xFF, 0x31, 0, 0]);
            assert_eq!(p.gpu_power, [1, 1, 1, 0x57]);
            assert_eq!(p.shared_budget, [0xFF, 0xFF, 0xFF, 45]);
            assert_eq!(p.pl1_w(), 60);
        }
    }

    #[test]
    fn quiet_is_balanced_l0_without_dynamic_boost_on_ac() {
        let p = plan(HpMode::Quiet, 45, true).unwrap();
        assert_eq!(p.thermal, [0xFF, 0x10, 0, 0]);
        assert_eq!(p.gpu_power, [0, 0, 1, 0x57]);
        assert_eq!(p.cpu_power, [55, 55, 0xFF, 0xFF]);
        assert!(!p.expect_custom_tgp && !p.expect_ppab);
        let battery = plan(HpMode::Quiet, 45, false).unwrap();
        assert_eq!(battery.gpu_power, [0, 1, 1, 0x57]);
        assert!(battery.expect_ppab);
    }

    #[test]
    fn implausible_firmware_default_is_rejected() {
        assert!(plan(HpMode::Balanced, 0, true).is_err());
        assert!(plan(HpMode::Unleashed, 200, true).is_err());
        assert!(HpMode::parse("cool").is_err());
        for mode in [
            HpMode::Quiet,
            HpMode::Balanced,
            HpMode::Performance,
            HpMode::Unleashed,
        ] {
            assert_eq!(HpMode::parse(mode.name()).unwrap(), mode);
        }
    }
}
