mod ccd;
mod control_lease;
mod controller;
mod cooling;
mod cpu;
mod cpu_hardware;
mod cpu_tuning;
mod fan_service;
mod gpu;
mod gpu_support;
mod hp;
mod lighting;
mod nvidia;
mod smu;
mod startup;
mod thermal;
mod user_data;
mod vf;
mod windows_settings;
pub use ccd::{WindowsProcesses, ccd_masks};
pub use control_lease::ControlLease;
pub use controller::{Controller, RestoreState};
pub use cooling::{FanReport, MaxFanImage, read_hp_fans};
pub use cpu::Cpu;
pub use cpu_hardware::{CPU_DRIVER_SHA256, CPU_MODULE_SHA256, CpuHardwareReader};
pub use cpu_tuning::SmuImage;
pub use fan_service::{FanServiceStatus, run as run_fan_service};
pub use fan_service::{request_stop as stop_fan_service, run_until as run_fan_service_until};
pub use gpu::{GpuControlsReport, NvidiaIdentity, OffsetState, read_gpu_controls};
pub use hp::{
    GraphicsModeReport, HpModeReport, apply_graphics_mode, apply_hp_mode, read_hp_gpu_status,
    read_hp_graphics_mode, read_hp_keyboard_state, read_hp_system_design, write_hp_gpu_power,
    write_hp_graphics_mode,
};
pub use lighting::DirectLighting;
pub use nvidia::Nvidia;
use omen_core::{Inventory, PowerStatus};
pub use smu::{
    LimitCalibration, SmuProbe, calibrate_limits as smu_calibrate_limits, probe as smu_probe,
};
pub use startup::{
    StartupInstallation, authorize_startup_profile, current_user_sid, fan_service_installed,
    fan_service_present, verify_startup_installation, verify_startup_profile,
};
use std::collections::HashMap;
pub use thermal::{CoolingSnapshot, read_cooling_snapshot};
pub(crate) use user_data::profile_directory;
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use wmi::{Variant, WMIConnection};

/// Wall-clock time of the current boot, from the system uptime.
pub fn boot_unix_s() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // SAFETY: Parameterless public uptime query.
    let uptime_s =
        unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() } / 1000;
    now.saturating_sub(uptime_s)
}
pub fn power_status() -> PowerStatus {
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    // SAFETY: status is a writable, correctly sized Windows structure.
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return PowerStatus::default();
    }
    PowerStatus {
        ac_connected: match status.ACLineStatus {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        battery_percent: if status.BatteryFlag != 255
            && status.BatteryFlag & 128 == 0
            && status.BatteryLifePercent <= 100
        {
            Some(status.BatteryLifePercent)
        } else {
            None
        },
    }
}

type Row = HashMap<String, Variant>;
fn field(row: &Row, key: &str) -> String {
    match row.get(key) {
        Some(Variant::String(value)) => value.trim().into(),
        _ => String::new(),
    }
}

pub fn inventory() -> Inventory {
    let mut result = Inventory::default();
    let connection = match WMIConnection::new() {
        Ok(c) => c,
        Err(e) => {
            result
                .warnings
                .push(format!("WMI inventory unavailable: {e}"));
            return result;
        }
    };
    let mut query = |sql: &str| -> Vec<Row> {
        match connection.raw_query(sql) {
            Ok(rows) => rows,
            Err(e) => {
                result.warnings.push(format!("{sql}: {e}"));
                vec![]
            }
        }
    };
    if let Some(row) = query("SELECT Manufacturer, Model FROM Win32_ComputerSystem").first() {
        result.manufacturer = field(row, "Manufacturer");
        result.model = field(row, "Model");
    }
    if let Some(row) = query("SELECT Product FROM Win32_BaseBoard").first() {
        result.board = field(row, "Product");
    }
    if let Some(row) = query("SELECT SMBIOSBIOSVersion FROM Win32_BIOS").first() {
        result.bios = field(row, "SMBIOSBIOSVersion");
    }
    result.cpus = query("SELECT Name FROM Win32_Processor")
        .iter()
        .map(|r| field(r, "Name"))
        .collect();
    result.gpus = query("SELECT Name FROM Win32_VideoController")
        .iter()
        .map(|r| field(r, "Name"))
        .collect();
    // Metadata only: no BIOS method is invoked and no opcode is sent.
    match WMIConnection::with_namespace_path("ROOT\\WMI") {
        Ok(hp) => {
            match hp.raw_query::<Row>("SELECT * FROM meta_class WHERE __CLASS = 'hpqBIntM'") {
                Ok(rows) => result.hp_bios_interface_present = !rows.is_empty(),
                Err(e) => result
                    .warnings
                    .push(format!("HP WMI metadata unavailable: {e}")),
            }
        }
        Err(e) => result.warnings.push(format!("ROOT\\WMI unavailable: {e}")),
    }
    result
}
