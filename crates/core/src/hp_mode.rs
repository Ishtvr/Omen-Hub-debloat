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
            thermal: [0xFF, UNLEASHED_MODE, 0, 0],
            gpu_power: [1, 1, D1, GPS_MAX_TEMPERATURE],
            shared_budget: [0xFF, 0xFF, 0xFF, SHARED_BUDGET_MAX_W],
            cpu_power: pl1(UNLEASHED_PL1_W),
            expect_custom_tgp: true,
            expect_ppab: true,
        },
        // cTGP and Dynamic Boost on, on AC and on battery.
        HpMode::Performance => ModePlan {
            mode,
            thermal: [0xFF, PERFORMANCE_MODE, 0, 0],
            gpu_power: [1, 1, D1, GPS_MAX_TEMPERATURE],
            shared_budget: budget,
            cpu_power: pl1(PERFORMANCE_PL1_W),
            expect_custom_tgp: true,
            expect_ppab: true,
        },
        // The Hub's Balanced on AC: cTGP off, Dynamic Boost (PPAB) on.
        HpMode::Balanced => ModePlan {
            mode,
            thermal: [0xFF, BALANCED_MODE, 0, 0],
            gpu_power: [0, 1, D1, GPS_MAX_TEMPERATURE],
            shared_budget: budget,
            cpu_power: pl1(BALANCED_PL1_W),
            expect_custom_tgp: false,
            expect_ppab: true,
        },
        // Slider steps L0/L1/L3/L4 turn Dynamic Boost off on AC only.
        HpMode::Quiet => ModePlan {
            mode,
            thermal: [0xFF, QUIET_MODE, 0, 0],
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
    fn unleashed_matches_the_hub_payloads() {
        let p = plan(HpMode::Unleashed, 45, true).unwrap();
        assert_eq!(p.thermal, [0xFF, 0x04, 0, 0]);
        assert_eq!(p.gpu_power, [1, 1, 1, 0x57]);
        assert_eq!(p.shared_budget, [0xFF, 0xFF, 0xFF, 72]);
        assert_eq!(p.cpu_power, [80, 80, 0xFF, 0xFF]);
        assert!(p.expect_custom_tgp && p.expect_ppab);
    }

    #[test]
    fn balanced_restores_firmware_budget_and_disables_ctgp() {
        let p = plan(HpMode::Balanced, 45, true).unwrap();
        assert_eq!(p.thermal, [0xFF, 0x30, 0, 0]);
        assert_eq!(p.gpu_power, [0, 1, 1, 0x57]);
        assert_eq!(p.shared_budget, [0xFF, 0xFF, 0xFF, 45]);
        assert_eq!(p.cpu_power, [55, 55, 0xFF, 0xFF]);
        assert!(!p.expect_custom_tgp && p.expect_ppab);
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
