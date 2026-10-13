//! Hardware-independent policy for Omen Hub++. No OS access or firmware writes.
pub mod cpu;
pub mod curve_editor;
pub mod curve_input;
pub mod curve_view;
pub mod fan;
pub mod features;
pub mod hp;
pub mod keyboard;
pub mod profiles;
pub mod supervisor;
pub mod transaction;
pub mod vf;

use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "Omen Hub++";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub manufacturer: String,
    pub model: String,
    pub board: String,
    pub bios: String,
    pub cpus: Vec<String>,
    pub gpus: Vec<String>,
    pub hp_bios_interface_present: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PowerStatus {
    /// None means Windows could not determine the power source.
    pub ac_connected: Option<bool>,
    pub battery_percent: Option<u8>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GpuSample {
    pub name: String,
    pub temperature_c: Option<i32>,
    pub clock_mhz: Option<u32>,
    pub power_w: Option<f64>,
    pub default_limit_w: Option<f64>,
    /// A driver-reported constraint, NOT a measured sustainable TGP.
    pub maximum_limit_w: Option<f64>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CpuSample {
    /// Busy CPU time over the sampling interval; this is not package wattage.
    pub busy_percent: Option<f64>,
    /// Average CurrentMhz reported by Windows. Not a measured boost/effective clock.
    pub windows_clock_mhz: Option<u32>,
    pub logical_processors: Option<u32>,
    /// Requires a verified CPU-family-specific hardware reader; never estimated from load.
    pub package_power_w: Option<f64>,
    /// CPU die/package temperature only; an ACPI zone is not substituted.
    pub temperature_c: Option<f64>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub inventory: Inventory,
    pub power: PowerStatus,
    #[serde(default)]
    pub cpu: Option<CpuSample>,
    pub gpu: Option<GpuSample>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    Quiet,
    Balanced,
    Performance,
    Unleashed,
}

impl Profile {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "quiet" => Ok(Self::Quiet),
            "balanced" => Ok(Self::Balanced),
            "performance" => Ok(Self::Performance),
            "unleashed" => Ok(Self::Unleashed),
            _ => Err(format!("Unknown profile: {value}")),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ProfilePlan {
    pub profile: Profile,
    pub board: String,
    pub bios: String,
    pub applicable: bool,
    pub requested_features: Vec<&'static str>,
    pub blockers: Vec<&'static str>,
}

/// Phase-1 planning only. There is deliberately no apply method or raw-write escape hatch.
pub fn preview_profile(inventory: &Inventory, profile: Profile) -> ProfilePlan {
    let mut requested_features = vec!["HP performance/thermal profile"];
    if profile == Profile::Unleashed {
        requested_features.extend([
            "OEM-supported custom TGP and Dynamic Boost",
            "Model-specific power budget and heartbeat",
        ]);
    }
    ProfilePlan {
        profile,
        board: inventory.board.clone(),
        bios: inventory.bios.clone(),
        applicable: false,
        requested_features,
        blockers: vec![
            "No model/BIOS combination has validated write support yet",
            "Firmware readback, rollback and AC/resume behavior must be verified first",
        ],
    }
}
