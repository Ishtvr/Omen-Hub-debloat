//! HP fan response decoding. RPM observations are never restore targets.
use serde::{Deserialize, Serialize};

/// The manufacturer's 25C1 notebook path names selector 0 an IR temperature
/// in Celsius. This is firmware provenance, not a physical sensor calibration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FirmwareIrTemperature {
    pub celsius: u8,
    pub selector: u8,
    pub raw: [u8; 4],
}
impl FirmwareIrTemperature {
    pub fn decode(code: u32, bytes: &[u8]) -> Result<Self, String> {
        let raw: [u8; 4] = bytes.try_into().map_err(|_| "IR GET requires four bytes")?;
        // Conservatively reject zero/sentinels, unknown flags and implausible
        // values. This range is an input guard, not an operating-temperature limit.
        if code != 0 || !(1..=125).contains(&raw[0]) || raw[1..] != [0; 3] {
            return Err(
                "IR temperature is unavailable or has an unqualified response layout".into(),
            );
        }
        Ok(Self {
            celsius: raw[0],
            selector: 0,
            raw,
        })
    }
}

/// Both endpoints use the same monotonic clock for one acquisition. Using the
/// start bounds the age of values read early during a slow multi-read query.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SampleWindow {
    pub started_ms: u64,
    pub completed_ms: u64,
}
impl SampleWindow {
    pub fn is_fresh_at(self, now_ms: u64, maximum_age_ms: u64) -> bool {
        self.started_ms <= self.completed_ms
            && self.completed_ms <= now_ms
            && now_ms - self.started_ms <= maximum_age_ms
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaxFanState {
    pub enabled: bool,
    pub raw: [u8; 4],
}
impl MaxFanState {
    pub fn decode(code: u32, bytes: &[u8]) -> Result<Self, String> {
        let raw: [u8; 4] = bytes
            .try_into()
            .map_err(|_| "Max-fan GET requires four bytes")?;
        if code != 0 || raw[0] > 1 {
            return Err("Unknown or failed max-fan reply".into());
        }
        Ok(Self {
            enabled: raw[0] == 1,
            raw,
        })
    }
    pub fn validate_restore(&self) -> Result<(), String> {
        if Self::decode(0, &self.raw)? != *self || self.raw[1..] != [0; 3] {
            return Err("Max-fan image differs from its qualified boolean/opaque state".into());
        }
        Ok(())
    }
    pub fn target(&self, enabled: bool) -> Result<Self, String> {
        self.validate_restore()?;
        let mut target = self.clone();
        target.enabled = enabled;
        target.raw[0] = u8::from(enabled);
        Ok(target)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanSpeeds {
    pub cpu_rpm: Option<u32>,
    pub gpu_rpm: Option<u32>,
    /// Retain flags/unsupported slots without treating them as positive RPM.
    pub raw: Vec<u8>,
}
pub fn decode_speeds(code: u32, bytes: &[u8]) -> Result<FanSpeeds, String> {
    if code != 0 || bytes.len() != 128 {
        return Err("Fan-speed GET requires a successful128-byte reply".into());
    }
    let rpm = |b: u8| {
        if b < 128 {
            Some(u32::from(b) * 100)
        } else {
            None
        }
    };
    Ok(FanSpeeds {
        cpu_rpm: rpm(bytes[0]),
        gpu_rpm: rpm(bytes[1]),
        raw: bytes.into(),
    })
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanTableRow {
    pub cpu_raw: u8,
    pub gpu_raw: u8,
    pub acoustic_raw: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanTable {
    pub fan_count: u8,
    /// Manufacturer parser does not interpret byte1. It is not a row count.
    pub header_byte_raw: u8,
    pub rows: Vec<FanTableRow>,
    pub raw: Vec<u8>,
}
pub fn decode_table(code: u32, bytes: &[u8]) -> Result<FanTable, String> {
    if code != 0 || bytes.len() != 128 || bytes[0] != 2 {
        return Err("Only complete two-fan table replies are qualified".into());
    }
    let mut rows: Vec<FanTableRow> = vec![];
    let mut cursor = 2;
    let mut terminated = false;
    while cursor + 3 <= bytes.len() {
        let r = &bytes[cursor..cursor + 3];
        if r == [0, 0, 0] {
            terminated = true;
            break;
        }
        if r[0] == 0
            || r[1] == 0
            || r.iter().any(|v| *v >= 128)
            || rows.last().is_some_and(|old| {
                r[0] <= old.cpu_raw || r[1] < old.gpu_raw || r[2] < old.acoustic_raw
            })
        {
            return Err("Malformed/nonmonotonic fan table".into());
        }
        rows.push(FanTableRow {
            cpu_raw: r[0],
            gpu_raw: r[1],
            acoustic_raw: r[2],
        });
        cursor += 3;
    }
    if !terminated || rows.len() < 2 || bytes[cursor..].iter().any(|b| *b != 0) {
        return Err(
            "Fan table has no bounded zero terminator or contains unknown trailing data".into(),
        );
    }
    Ok(FanTable {
        fan_count: bytes[0],
        header_byte_raw: bytes[1],
        rows,
        raw: bytes.into(),
    })
}
impl FanTable {
    /// Pure planning helper for the observed paired-fan mapping. Does not write
    /// hardware or qualify software curves/percentages for a particular board.
    pub fn paired_raw(&self, cpu: u8) -> Result<[u8; 2], String> {
        let parsed = decode_table(0, &self.raw)?;
        if parsed != *self {
            return Err("Fan table does not match its raw evidence".into());
        }
        let first = self.rows.first().ok_or("Empty fan table")?;
        let last = self.rows.last().ok_or("Empty fan table")?;
        if cpu < first.cpu_raw || cpu > last.cpu_raw {
            return Err("CPU fan target is outside the observed table".into());
        }
        let row = self
            .rows
            .iter()
            .rev()
            .find(|r| r.cpu_raw <= cpu)
            .ok_or("Fan mapping is missing")?;
        let gpu = i16::from(cpu) + i16::from(row.gpu_raw) - i16::from(row.cpu_raw);
        if gpu < i16::from(first.gpu_raw) || gpu > i16::from(last.gpu_raw) {
            return Err("Paired fan mapping exceeds observed bounds".into());
        }
        Ok([cpu, gpu as u8])
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FanTopology {
    pub slots_raw: Vec<u8>,
    pub capabilities_raw: [u8; 2],
    pub raw: Vec<u8>,
}
pub fn decode_topology(code: u32, bytes: &[u8]) -> Result<FanTopology, String> {
    if code != 0 || bytes.len() != 128 {
        return Err("Fan topology requires a successful128-byte reply".into());
    }
    let mut slots = vec![];
    for b in &bytes[..4] {
        slots.push(b & 15);
        slots.push(b >> 4);
    }
    slots.pop();
    Ok(FanTopology {
        slots_raw: slots,
        capabilities_raw: [bytes[8], bytes[9]],
        raw: bytes.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ir_temperature_retains_provenance_and_rejects_faults_without_zero_fallback() {
        let value = FirmwareIrTemperature::decode(0, &[39, 0, 0, 0]).unwrap();
        assert_eq!(
            (value.celsius, value.selector, value.raw),
            (39, 0, [39, 0, 0, 0])
        );
        for raw in [[0, 0, 0, 0], [126, 0, 0, 0], [255, 0, 0, 0], [39, 1, 0, 0]] {
            assert!(FirmwareIrTemperature::decode(0, &raw).is_err());
        }
        assert!(FirmwareIrTemperature::decode(1, &[39, 0, 0, 0]).is_err());
        assert!(FirmwareIrTemperature::decode(0, &[39, 0, 0]).is_err());
    }
    #[test]
    fn freshness_rejects_slow_reads_old_data_future_and_reversed_windows() {
        let w = SampleWindow {
            started_ms: 100,
            completed_ms: 200,
        };
        assert!(w.is_fresh_at(2100, 2000));
        assert!(!w.is_fresh_at(2101, 2000));
        assert!(!w.is_fresh_at(199, 2000));
        assert!(
            !SampleWindow {
                started_ms: 100,
                completed_ms: 2200
            }
            .is_fresh_at(2200, 2000)
        );
        assert!(
            !SampleWindow {
                started_ms: 200,
                completed_ms: 100
            }
            .is_fresh_at(300, 2000)
        );
        assert!(
            !SampleWindow {
                started_ms: u64::MAX,
                completed_ms: u64::MAX
            }
            .is_fresh_at(0, 2000)
        );
    }
    #[test]
    fn max_state_rejects_bad_status_lengths_boolean_and_forged_restore() {
        for b in [&[][..], &[0, 0, 0][..], &[0, 0, 0, 0, 0][..]] {
            assert!(MaxFanState::decode(0, b).is_err());
        }
        assert!(MaxFanState::decode(1, &[0; 4]).is_err());
        assert!(MaxFanState::decode(0, &[2, 0, 0, 0]).is_err());
        let original = MaxFanState::decode(0, &[0; 4]).unwrap();
        assert_eq!(original.target(true).unwrap().raw, [1, 0, 0, 0]);
        let mut forged = original.clone();
        forged.enabled = true;
        assert!(forged.validate_restore().is_err());
        assert!(
            MaxFanState::decode(0, &[0, 1, 0, 0])
                .unwrap()
                .validate_restore()
                .is_err()
        );
    }
    #[test]
    fn speed_zero_and_unknown_flags_do_not_truncate_the_other_fan() {
        let mut b = [0; 128];
        b[1] = 31;
        let s = decode_speeds(0, &b).unwrap();
        assert_eq!(s.cpu_rpm, Some(0));
        assert_eq!(s.gpu_rpm, Some(3100));
        b[0] = 255;
        b[1] = 128;
        let s = decode_speeds(0, &b).unwrap();
        assert_eq!((s.cpu_rpm, s.gpu_rpm), (None, None));
        assert!(decode_speeds(1, &b).is_err());
        assert!(decode_speeds(0, &b[..127]).is_err());
    }
    fn table() -> [u8; 128] {
        let mut b = [0; 128];
        b[..11].copy_from_slice(&[2, 99, 20, 18, 25, 30, 32, 35, 40, 43, 45]);
        b
    }
    #[test]
    fn table_preserves_header_and_uses_piecewise_paired_mapping() {
        let t = decode_table(0, &table()).unwrap();
        assert_eq!(t.header_byte_raw, 99);
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.paired_raw(25).unwrap(), [25, 23]);
        assert_eq!(t.paired_raw(35).unwrap(), [35, 37]);
        assert!(t.paired_raw(19).is_err());
        assert!(t.paired_raw(41).is_err());
        let mut forged = t;
        forged.rows[0].gpu_raw = 21;
        assert!(forged.paired_raw(25).is_err());
    }
    #[test]
    fn table_rejects_unknown_counts_truncation_and_opaque_tail() {
        let mut b = table();
        b[0] = 3;
        assert!(decode_table(0, &b).is_err());
        b = table();
        b[12] = 1;
        assert!(decode_table(0, &b).is_err());
        b = table();
        b[8] = 19;
        assert!(decode_table(0, &b).is_err());
        assert!(decode_table(0, &b[..127]).is_err());
        assert!(decode_table(0, &[0; 128]).is_err());
    }
    #[test]
    fn topology_keeps_unknown_slots_and_capability_bits() {
        let mut b = [0; 128];
        b[0] = 0x21;
        b[1] = 0xf0;
        b[8] = 0x80;
        let t = decode_topology(0, &b).unwrap();
        assert_eq!(t.slots_raw[..4], [1, 2, 0, 15]);
        assert_eq!(t.capabilities_raw, [128, 0]);
        assert!(decode_topology(1, &b).is_err());
    }
}
