//! One-shot cooling diagnostics. No writer, registry/profile fallback, vendor
//! SDK, background loop or implicit elevation. All readers have fixed contracts.
use super::{CpuHardwareReader, FanReport, Nvidia, power_status, read_hp_fans};
use omen_core::{CpuHardwareSample, GpuSample, Inventory, PowerStatus, cooling::SampleWindow};
use serde::Serialize;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const MAXIMUM_SAMPLE_AGE_MS: u64 = 2000;

#[derive(Debug, Serialize)]
pub struct TimedRead<T> {
    pub window: SampleWindow,
    pub result: Result<T, String>,
}

#[derive(Debug, Serialize)]
pub struct CoolingSnapshot {
    pub schema: u32,
    pub board: String,
    pub bios: String,
    pub started_unix_ms: u64,
    pub elapsed_ms: u64,
    pub maximum_sample_age_ms: u64,
    pub power_before: PowerStatus,
    pub power_after: PowerStatus,
    pub fans: TimedRead<FanReport>,
    pub cpu: TimedRead<CpuHardwareSample>,
    pub gpu: TimedRead<GpuSample>,
    pub sensor_faults: Vec<String>,
    /// A successful diagnostic batch is not a qualification for fan control.
    pub temperature_batch_valid: bool,
    pub firmware_performance_mode: Option<u8>,
    pub firmware_fan_owner: Option<String>,
    pub software_curve_write_supported: bool,
    pub curve_blockers: Vec<String>,
    pub hardware_applied: bool,
}

fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn timed<T>(start: Instant, read: impl FnOnce() -> Result<T, String>) -> TimedRead<T> {
    let started_ms = elapsed(start);
    let result = read();
    TimedRead {
        window: SampleWindow {
            started_ms,
            completed_ms: elapsed(start),
        },
        result,
    }
}

fn fault<T>(name: &str, read: &TimedRead<T>, now_ms: u64, faults: &mut Vec<String>) {
    if let Err(error) = &read.result {
        faults.push(format!("{name}: {error}"));
    }
    if !read.window.is_fresh_at(now_ms, MAXIMUM_SAMPLE_AGE_MS) {
        faults.push(format!(
            "{name}: acquisition is older than the diagnostic freshness bound"
        ));
    }
}

pub fn read_cooling_snapshot(inventory: &Inventory) -> Result<CoolingSnapshot, String> {
    if inventory.manufacturer != "HP" || inventory.board != "8E35" || inventory.bios != "F.13" {
        return Err("Cooling diagnostics are qualified only for HP 8E35/F.13".into());
    }
    let started_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis()
        .try_into()
        .map_err(|_| "System time is outside the diagnostic timestamp range")?;
    let start = Instant::now();
    let power_before = power_status();
    let fans = timed(start, || read_hp_fans(inventory));
    let cpu = timed(start, || {
        let sample = CpuHardwareReader::new(inventory)?.temperature_sample()?;
        sample.validate()?;
        if sample.temperature_c.is_none() || !sample.warnings.is_empty() {
            return Err("Verified CPU Tctl is unavailable or has sensor warnings".into());
        }
        // A thermal-only read has no energy or effective-clock baseline.
        Ok(sample)
    });
    let gpu = timed(start, || {
        let mut reader = Nvidia::new();
        let sample = reader
            .sample()
            .ok_or_else(|| format!("GPU reader unavailable: {}", reader.warnings().join("; ")))?;
        if !sample.temperature_c.is_some_and(|v| (0..=125).contains(&v)) {
            return Err("NVIDIA temperature is unavailable or implausible".into());
        }
        Ok(sample)
    });
    let power_after = power_status();
    let elapsed_ms = elapsed(start);
    let mut sensor_faults = vec![];
    fault("HP fan/IR", &fans, elapsed_ms, &mut sensor_faults);
    fault("CPU Tctl", &cpu, elapsed_ms, &mut sensor_faults);
    fault("NVIDIA temperature", &gpu, elapsed_ms, &mut sensor_faults);
    if let Ok(report) = &fans.result {
        if let Err(error) = &report.ir_temperature {
            sensor_faults.push(format!("HP IR: {error}"));
        }
    }
    if power_before.ac_connected.is_none() || power_after.ac_connected.is_none() {
        sensor_faults.push("Power source is unavailable".into());
    } else if power_before.ac_connected != power_after.ac_connected {
        sensor_faults.push("Power source changed during acquisition".into());
    }
    let temperature_batch_valid = sensor_faults.is_empty();
    Ok(CoolingSnapshot {
        schema: 1,
        board: inventory.board.clone(),
        bios: inventory.bios.clone(),
        started_unix_ms,
        elapsed_ms,
        maximum_sample_age_ms: MAXIMUM_SAMPLE_AGE_MS,
        power_before,
        power_after,
        fans,
        cpu,
        gpu,
        sensor_faults,
        temperature_batch_valid,
        firmware_performance_mode: None,
        firmware_fan_owner: None,
        software_curve_write_supported: false,
        curve_blockers: vec![
            "Firmware performance mode and fan ownership have no qualified GET/restore contract; the original hub's saved Auto setting is not firmware readback".into(),
            "Coupled CPU/GPU/IR safety limits and software-level SET/readback/restore require model validation".into(),
            "Independent ownership and AC/suspend/resume/reboot failure handling remain unqualified".into(),
        ],
        hardware_applied: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn successful_value_with_slow_acquisition_is_not_a_fresh_batch() {
        let read = TimedRead {
            window: SampleWindow {
                started_ms: 0,
                completed_ms: 2500,
            },
            result: Ok(40),
        };
        let mut faults = vec![];
        fault("sensor", &read, 2500, &mut faults);
        assert_eq!(faults.len(), 1);
        let failed: TimedRead<u8> = TimedRead {
            window: SampleWindow {
                started_ms: 2400,
                completed_ms: 2500,
            },
            result: Err("access denied".into()),
        };
        fault("sensor", &failed, 2500, &mut faults);
        assert_eq!(faults.len(), 2);
        assert!(faults[1].contains("access denied"));
    }
}
