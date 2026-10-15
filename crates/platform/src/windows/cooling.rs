//! Board-bound cooling transport. Fixed GETs and one independent max-fan SET;
//! no performance-mode, software level, EC, power or voltage setter.
use super::hp::{HpBios, Read};
use omen_core::{
    Inventory,
    cooling::{self, FanSpeeds, FanTable, FanTopology, MaxFanState},
    hp::SystemDesignData,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FAN_TABLE_SHA256: &str =
    "a8e624c6573881f444e36d94a29ca0af320248855377531f02a499437bee57f2";
#[derive(Debug, Serialize)]
pub struct FanReport {
    pub board: String,
    pub bios: String,
    pub max_fan: MaxFanState,
    pub speeds: FanSpeeds,
    pub topology: FanTopology,
    pub table: FanTable,
    pub table_sha256: String,
    pub system_design: SystemDesignData,
    pub surface_sensor_raw: Vec<u8>,
    pub software_curve_write_supported: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaxFanImage {
    pub board: String,
    pub bios: String,
    pub table_sha256: String,
    pub state: MaxFanState,
}
pub(super) struct FanControl {
    bios: HpBios,
    board: String,
    version: String,
}
impl FanControl {
    pub fn new(inv: &Inventory) -> Result<Self, String> {
        Ok(Self {
            bios: HpBios::new(inv).map_err(|e| e.to_string())?,
            board: inv.board.clone(),
            version: inv.bios.clone(),
        })
    }
    fn get(&self, read: Read) -> Result<(u32, Vec<u8>), String> {
        self.bios.get(read).map_err(|e| e.to_string())
    }
    pub fn max_state(&self) -> Result<MaxFanState, String> {
        let (c, b) = self.get(Read::MaxFan)?;
        MaxFanState::decode(c, &b)
    }
    pub fn report(&self) -> Result<FanReport, String> {
        let (c, b) = self.get(Read::SystemDesign)?;
        let system_design = omen_core::hp::decode_system_design(c, &b)?;
        let max_fan = self.max_state()?;
        let (c, b) = self.get(Read::FanSpeeds)?;
        let speeds = cooling::decode_speeds(c, &b)?;
        let (c, b) = self.get(Read::FanTypes)?;
        let topology = cooling::decode_topology(c, &b)?;
        let (c, b) = self.get(Read::FanTable)?;
        let table = cooling::decode_table(c, &b)?;
        let table_sha256 = format!("{:x}", Sha256::digest(&table.raw));
        let (_, surface_sensor_raw) = self.get(Read::SurfaceSensor)?;
        Ok(FanReport {
            board: self.board.clone(),
            bios: self.version.clone(),
            max_fan,
            speeds,
            topology,
            table,
            table_sha256,
            system_design,
            surface_sensor_raw,
            software_curve_write_supported: false,
        })
    }
    pub fn capture(&self) -> Result<MaxFanImage, String> {
        let r = self.report()?;
        if r.board != "8E35"
            || r.bios != "F.13"
            || r.table_sha256 != FAN_TABLE_SHA256
            || !r.system_design.software_fan_supported
            || r.system_design.raw[3] != 1
            || r.topology.slots_raw != [1, 2, 0, 0, 0, 0, 0]
            || r.topology.capabilities_raw != [0, 0]
        {
            return Err("Fan layout/identity differs from measured8E35/F.13".into());
        }
        r.max_fan.validate_restore()?;
        Ok(MaxFanImage {
            board: self.board.clone(),
            bios: self.version.clone(),
            table_sha256: r.table_sha256,
            state: r.max_fan,
        })
    }
    pub fn program(&self, expected: &MaxFanImage, enabled: bool) -> Result<MaxFanState, String> {
        expected.state.validate_restore()?;
        let current = self.capture()?;
        if current != *expected {
            return Err("Max-fan state changed before dispatch; no SET sent".into());
        }
        let target = expected.state.target(enabled)?;
        if target != expected.state {
            self.bios.set_max_fan(enabled).map_err(|e| e.to_string())?;
        }
        let actual = self.max_state()?;
        if actual != target {
            return Err("Max-fan readback differs from requested complete flag state".into());
        }
        Ok(actual)
    }
}
pub fn read_hp_fans(inv: &Inventory) -> Result<FanReport, String> {
    FanControl::new(inv)?.report()
}
