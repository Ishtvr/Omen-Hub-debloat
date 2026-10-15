mod control_lease;
mod controller;
mod cooling;
mod cpu;
mod cpu_hardware;
mod gpu;
mod hp;
mod lighting;
mod nvidia;
mod startup;
mod thermal;
mod user_data;
mod vf;
pub use control_lease::ControlLease;
pub use controller::{Controller, RestoreState};
pub use cooling::{FanReport, MaxFanImage, read_hp_fans};
pub use cpu::Cpu;
pub use cpu_hardware::{CPU_DRIVER_SHA256, CPU_MODULE_SHA256, CpuHardwareReader};
pub use gpu::{GpuControlsReport, NvidiaIdentity, OffsetState, read_gpu_controls};
pub use hp::{
    HpModeReport, apply_hp_mode, read_hp_gpu_status, read_hp_keyboard_state, read_hp_system_design,
};
pub use lighting::DirectLighting;
pub use nvidia::Nvidia;
use omen_core::{Inventory, PowerStatus};
pub use startup::{
    StartupInstallation, authorize_startup_profile, current_user_sid, verify_startup_installation,
    verify_startup_profile,
};
use std::collections::HashMap;
pub use thermal::{CoolingSnapshot, read_cooling_snapshot};
pub(crate) use user_data::profile_directory;
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use wmi::{Variant, WMIConnection};

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
