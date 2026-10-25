//! Hardware-independent policy for Omen Hub++. No OS access or firmware writes.
pub mod ccd;
pub mod cooling;
pub mod cpu;
pub mod curve_editor;
pub mod curve_input;
pub mod curve_view;
pub mod fan;
pub mod features;
pub mod guardian;
pub mod hp;
pub mod hp_mode;
pub mod keyboard;
pub mod message;
pub mod ownership;
pub mod power;
pub mod preset;
pub mod profiles;
pub mod quick_switch;
pub mod readback;
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
    /// CPU hardware temperature; see hardware.temperature_kind (Tctl here).
    /// An ACPI thermal zone is never substituted.
    pub temperature_c: Option<f64>,
    /// Explicit provenance/interval for optional, verified hardware readings.
    pub hardware: Option<CpuHardwareSample>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuHardwareSample {
    pub schema: u32,
    pub source: CpuHardwareSource,
    pub temperature_kind: CpuTemperatureKind,
    pub package_power_w: Option<f64>,
    pub temperature_c: Option<f64>,
    pub power_interval_ms: Option<u64>,
    /// Read-only per-logical-processor interval means; never Windows MHz.
    #[serde(default)]
    pub effective_clocks: Option<cpu::CpuEffectiveClocks>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuHardwareSource {
    PawnioAmdRapl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuTemperatureKind {
    AmdTctl,
}

impl CpuHardwareSample {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1
            || self
                .package_power_w
                .is_some_and(|v| !v.is_finite() || !(0.0..=300.0).contains(&v))
            || self
                .temperature_c
                .is_some_and(|v| !v.is_finite() || !(0.0..=125.0).contains(&v))
            || self
                .power_interval_ms
                .is_some_and(|v| !(200..=10_000).contains(&v))
            || self.package_power_w.is_some() != self.power_interval_ms.is_some()
            || self.warnings.len() > 8
            || self.warnings.iter().any(|w| w.len() > 512)
        {
            return Err(
                "CPU sensor response schema, values or measured interval is invalid".into(),
            );
        }
        if let Some(clocks) = &self.effective_clocks {
            clocks.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub inventory: Inventory,
    pub power: PowerStatus,
    #[serde(default)]
    pub cpu: Option<CpuSample>,
    #[serde(default)]
    pub cpu_hardware_status: CpuHardwareStatus,
    /// Diagnostic identity of our approved reader, never a hardware control.
    pub cpu_reader_pid: Option<u32>,
    pub gpu: Option<GpuSample>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum CpuHardwareStatus {
    #[default]
    Off,
    Starting,
    Active,
    Paused,
    Error(String),
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
