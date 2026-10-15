//! Fixed HP notebook performance-mode payloads for board 8E35 / BIOS F.13.
//! Values mirror the installed OMEN Gaming Hub 1101.2609.3.0 (Khalilah
//! DRG_N22X4X6 resource). The BIOS has no GET for the thermal mode, so the
//! only restore is an explicit Balanced write; GPU flags are read back.
use serde::{Deserialize, Serialize};

/// `PerformanceControlHelper.SetFanMode`: [0xFF, mode, bios fan owner, 0].
pub const THERMAL_MODE_TYPE: u32 = 0x1A;
/// `PowerControlBase.SetTgpPpabAsync`: [cTGP, PPAB, D-state, GPS 87 °C].
pub const GPU_POWER_TYPE: u32 = 0x22;
/// `PowerControlBase.SetConcurrentTdp`: [FF, FF, FF, shared CPU+GPU watts].
pub const POWER_LIMITS_TYPE: u32 = 0x29;

/// Inner mode bytes the Hub sends on this notebook (no policy-version mapping).
const BALANCED_MODE: u8 = 0x30;
const UNLEASHED_MODE: u8 = 0x04;
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
    Balanced,
    Unleashed,
}

impl HpMode {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "balanced" => Ok(Self::Balanced),
            "unleashed" => Ok(Self::Unleashed),
            _ => Err(format!("Unknown HP mode {text:?}")),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
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
    pub expect_custom_tgp: bool,
    pub expect_ppab: bool,
}

/// `default_shared_budget_w` is SystemDesignData byte 8 (45 W on 8E35/F.13);
/// Balanced writes it back because the Hub never resets TPP itself.
pub fn plan(mode: HpMode, default_shared_budget_w: u8) -> Result<ModePlan, String> {
    if !(SHARED_BUDGET_MIN_W..=SHARED_BUDGET_MAX_W).contains(&default_shared_budget_w) {
        return Err(format!(
            "Firmware shared-budget default {default_shared_budget_w} W is outside {SHARED_BUDGET_MIN_W}…{SHARED_BUDGET_MAX_W} W"
        ));
    }
    Ok(match mode {
        HpMode::Unleashed => ModePlan {
            mode,
            thermal: [0xFF, UNLEASHED_MODE, 0, 0],
            gpu_power: [1, 1, D1, GPS_MAX_TEMPERATURE],
            shared_budget: [0xFF, 0xFF, 0xFF, SHARED_BUDGET_MAX_W],
            expect_custom_tgp: true,
            expect_ppab: true,
        },
        // The Hub's Balanced on AC: cTGP off, Dynamic Boost (PPAB) on.
        HpMode::Balanced => ModePlan {
            mode,
            thermal: [0xFF, BALANCED_MODE, 0, 0],
            gpu_power: [0, 1, D1, GPS_MAX_TEMPERATURE],
            shared_budget: [0xFF, 0xFF, 0xFF, default_shared_budget_w],
            expect_custom_tgp: false,
            expect_ppab: true,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unleashed_matches_the_hub_payloads() {
        let p = plan(HpMode::Unleashed, 45).unwrap();
        assert_eq!(p.thermal, [0xFF, 0x04, 0, 0]);
        assert_eq!(p.gpu_power, [1, 1, 1, 0x57]);
        assert_eq!(p.shared_budget, [0xFF, 0xFF, 0xFF, 72]);
        assert!(p.expect_custom_tgp && p.expect_ppab);
    }

    #[test]
    fn balanced_restores_firmware_budget_and_disables_ctgp() {
        let p = plan(HpMode::Balanced, 45).unwrap();
        assert_eq!(p.thermal, [0xFF, 0x30, 0, 0]);
        assert_eq!(p.gpu_power, [0, 1, 1, 0x57]);
        assert_eq!(p.shared_budget, [0xFF, 0xFF, 0xFF, 45]);
        assert!(!p.expect_custom_tgp && p.expect_ppab);
    }

    #[test]
    fn implausible_firmware_default_is_rejected() {
        assert!(plan(HpMode::Balanced, 0).is_err());
        assert!(plan(HpMode::Unleashed, 200).is_err());
        assert!(HpMode::parse("quiet").is_err());
        assert_eq!(HpMode::parse("unleashed").unwrap().name(), "unleashed");
    }
}
