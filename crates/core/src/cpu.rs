//! CPU telemetry calculations, independent of Windows and hardware adapters.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Per-logical-processor read-only AMD performance counters. These are not
/// requested frequencies, voltage settings or the writable APERF/MPERF aliases.
#[derive(Debug, Clone, Copy)]
pub struct AmdClockCounters {
    pub logical_processor: u32,
    pub actual_cycles: u64,
    pub reference_cycles: u64,
}

/// Actual cycles per wall-clock interval, including time spent halted/idle.
/// This is an interval mean, not an instantaneous or an active-only boost clock.
pub fn amd_effective_mhz(
    previous: AmdClockCounters,
    current: AmdClockCounters,
    elapsed: Duration,
) -> Option<f64> {
    if previous.logical_processor != current.logical_processor
        || current.logical_processor >= 64
        || !(Duration::from_millis(200)..=Duration::from_secs(10)).contains(&elapsed)
        || [
            previous.actual_cycles,
            previous.reference_cycles,
            current.actual_cycles,
            current.reference_cycles,
        ]
        .contains(&u64::MAX)
    {
        return None;
    }
    // Unlike the 32-bit energy counter, a decreasing 64-bit cycle counter is
    // treated as a reset. Do not turn it into a fabricated wraparound frequency.
    let cycles = current.actual_cycles.checked_sub(previous.actual_cycles)?;
    let reference = current
        .reference_cycles
        .checked_sub(previous.reference_cycles)?;
    let mhz = cycles as f64 / elapsed.as_secs_f64() / 1_000_000.0;
    let reference_mhz = reference as f64 / elapsed.as_secs_f64() / 1_000_000.0;
    // A plausibility boundary only; this does not authorize a frequency setter.
    ((0.0..=8000.0).contains(&mhz) && (0.0..=8000.0).contains(&reference_mhz) && reference != 0)
        .then_some(mhz)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuClockSource {
    AmdAperfReadOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuLogicalClock {
    pub logical_processor: u32,
    pub effective_mhz: f64,
    pub interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuEffectiveClocks {
    pub source: CpuClockSource,
    pub logical_processor_count: u32,
    /// Complete coverage of the single qualified processor group, in CPU order.
    /// Partial coverage must not silently become a system maximum or mean.
    pub processors: Vec<CpuLogicalClock>,
}

impl CpuEffectiveClocks {
    pub fn validate(&self) -> Result<(), String> {
        if self.processors.is_empty()
            || self.processors.len() > 64
            || self.processors.len() != self.logical_processor_count as usize
            || self.processors.iter().enumerate().any(|(index, clock)| {
                clock.logical_processor as usize != index
                    || !clock.effective_mhz.is_finite()
                    || !(0.0..=8000.0).contains(&clock.effective_mhz)
                    || !(200..=10_000).contains(&clock.interval_ms)
            })
        {
            return Err("CPU effective clock coverage, values or interval is invalid".into());
        }
        Ok(())
    }

    /// Highest interval mean among the sampled logical processors.
    pub fn max_mhz(&self) -> Option<f64> {
        self.processors
            .iter()
            .map(|p| p.effective_mhz)
            .reduce(f64::max)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CpuTimes {
    pub idle: u64,
    /// Kernel time includes idle time, matching the GetSystemTimes contract.
    pub kernel: u64,
    pub user: u64,
}

pub fn busy_percent(previous: CpuTimes, current: CpuTimes) -> Option<f64> {
    let idle = current.idle.checked_sub(previous.idle)?;
    let kernel = current.kernel.checked_sub(previous.kernel)?;
    let user = current.user.checked_sub(previous.user)?;
    let total = kernel.checked_add(user)?;
    if total == 0 || idle > kernel {
        return None;
    }
    Some((total - idle) as f64 / total as f64 * 100.0)
}

/// Zen's control temperature, including its encoded range adjustment. This is
/// Tctl, not an ACPI zone, CCD average or a claimed per-core die temperature.
pub fn amd_tctl_c(raw: u32) -> Option<f64> {
    if raw == 0 || raw == u32::MAX {
        return None;
    }
    let mut temperature = f64::from(raw >> 21) / 8.0;
    if raw & (1 << 19) != 0 || raw & (3 << 16) == 3 << 16 {
        temperature -= 49.0;
    }
    (0.0..=125.0).contains(&temperature).then_some(temperature)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmdPackageEnergy {
    pub counter: u32,
    pub units: u64,
}

/// Mean package watts from the low 32 bits of the AMD RAPL energy counter.
/// Reset/wrap ambiguity is bounded by interval and a conservative plausibility
/// limit; the limit is not an OEM setting or a supported power-write range.
pub fn amd_package_watts(
    previous: AmdPackageEnergy,
    current: AmdPackageEnergy,
    elapsed: std::time::Duration,
) -> Option<f64> {
    if current.units == 0
        || current.units == u64::MAX
        || current.units != previous.units
        || elapsed < std::time::Duration::from_millis(200)
        || elapsed > std::time::Duration::from_secs(10)
    {
        return None;
    }
    let exponent = ((current.units >> 8) & 31) as i32;
    let joules = f64::from(current.counter.wrapping_sub(previous.counter)) * 2f64.powi(-exponent);
    let watts = joules / elapsed.as_secs_f64();
    (watts.is_finite() && (0.0..=300.0).contains(&watts)).then_some(watts)
}

#[cfg(test)]
mod tests {
    use super::*;
    const BASE: CpuTimes = CpuTimes {
        idle: 100,
        kernel: 200,
        user: 100,
    };
    #[test]
    fn excludes_idle_from_kernel_time() {
        let value = busy_percent(
            BASE,
            CpuTimes {
                idle: 160,
                kernel: 280,
                user: 120,
            },
        )
        .unwrap();
        assert!((value - 40.0).abs() < 0.001);
    }
    #[test]
    fn distinguishes_zero_activity_from_full_activity() {
        assert_eq!(
            busy_percent(
                BASE,
                CpuTimes {
                    idle: 200,
                    kernel: 300,
                    user: 100
                }
            ),
            Some(0.0)
        );
        assert_eq!(
            busy_percent(
                BASE,
                CpuTimes {
                    idle: 100,
                    kernel: 300,
                    user: 100
                }
            ),
            Some(100.0)
        );
    }
    #[test]
    fn rejects_reset_inconsistent_and_overflowed_counters() {
        assert_eq!(busy_percent(BASE, BASE), None);
        assert_eq!(
            busy_percent(
                BASE,
                CpuTimes {
                    idle: 0,
                    kernel: 0,
                    user: 0
                }
            ),
            None
        );
        assert_eq!(
            busy_percent(
                BASE,
                CpuTimes {
                    idle: 180,
                    kernel: 210,
                    user: 120
                }
            ),
            None
        );
        assert_eq!(
            busy_percent(
                CpuTimes {
                    idle: 0,
                    kernel: 0,
                    user: 0
                },
                CpuTimes {
                    idle: 0,
                    kernel: u64::MAX,
                    user: 1
                }
            ),
            None
        );
    }

    #[test]
    fn tctl_decodes_both_range_flags_without_substituting_a_thermal_zone() {
        assert_eq!(amd_tctl_c(2222653440), Some(83.375));
        assert_eq!(amd_tctl_c(400 << 21), Some(50.0));
        assert_eq!(amd_tctl_c((800 << 21) | (3 << 16)), Some(51.0));
        assert_eq!(amd_tctl_c(0), None);
        assert_eq!(amd_tctl_c(u32::MAX), None);
        assert_eq!(amd_tctl_c((8 << 21) | (1 << 19)), None);
    }

    #[test]
    fn energy_watts_use_measured_interval_and_handle_wrap() {
        let a = AmdPackageEnergy {
            counter: u32::MAX - 32767,
            units: 16 << 8,
        };
        let b = AmdPackageEnergy {
            counter: 32768,
            ..a
        };
        assert_eq!(
            amd_package_watts(a, b, std::time::Duration::from_secs(1)),
            Some(1.0)
        );
        assert_eq!(
            amd_package_watts(a, b, std::time::Duration::from_secs(2)),
            Some(0.5)
        );
        assert_eq!(
            amd_package_watts(a, a, std::time::Duration::from_secs(1)),
            Some(0.0)
        );
    }

    #[test]
    fn energy_changes_resets_and_stale_intervals_do_not_become_power_readings() {
        let a = AmdPackageEnergy {
            counter: 2_000_000,
            units: 16 << 8,
        };
        let b = AmdPackageEnergy { counter: 0, ..a };
        assert_eq!(
            amd_package_watts(a, b, std::time::Duration::from_secs(1)),
            None
        );
        assert_eq!(amd_package_watts(a, a, std::time::Duration::ZERO), None);
        assert_eq!(
            amd_package_watts(a, a, std::time::Duration::from_secs(11)),
            None
        );
        assert_eq!(
            amd_package_watts(
                a,
                AmdPackageEnergy {
                    units: 15 << 8,
                    ..a
                },
                std::time::Duration::from_secs(1)
            ),
            None
        );
    }

    #[test]
    fn hardware_provenance_rejects_missing_interval_and_nonfinite_values() {
        use crate::{CpuHardwareSample, CpuHardwareSource, CpuTemperatureKind};
        let mut sample = CpuHardwareSample {
            schema: 1,
            source: CpuHardwareSource::PawnioAmdRapl,
            temperature_kind: CpuTemperatureKind::AmdTctl,
            package_power_w: Some(50.0),
            temperature_c: Some(80.0),
            power_interval_ms: Some(1100),
            effective_clocks: None,
            warnings: vec![],
        };
        assert!(sample.validate().is_ok());
        sample.power_interval_ms = None;
        assert!(sample.validate().is_err());
        sample.package_power_w = None;
        assert!(sample.validate().is_ok());
        sample.temperature_c = Some(f64::NAN);
        assert!(sample.validate().is_err());
    }

    #[test]
    fn effective_clock_counts_cycles_over_wall_time_including_idle() {
        let old = AmdClockCounters {
            logical_processor: 3,
            actual_cycles: 5_000_000_000,
            reference_cycles: 4_000_000_000,
        };
        let current = AmdClockCounters {
            actual_cycles: old.actual_cycles + 2_100_000_000,
            reference_cycles: old.reference_cycles + 1_800_000_000,
            ..old
        };
        assert_eq!(
            amd_effective_mhz(old, current, Duration::from_secs(1)),
            Some(2100.0)
        );
        assert_eq!(
            amd_effective_mhz(old, current, Duration::from_secs(2)),
            Some(1050.0)
        );
        // A running reference counter distinguishes a zero-cycle interval from
        // counters that have stopped or are unavailable.
        let idle = AmdClockCounters {
            reference_cycles: old.reference_cycles + 100_000,
            ..old
        };
        assert_eq!(
            amd_effective_mhz(old, idle, Duration::from_secs(1)),
            Some(0.0)
        );
        assert_eq!(amd_effective_mhz(old, old, Duration::from_secs(1)), None);
    }

    #[test]
    fn effective_clock_rejects_resets_cpu_mismatch_stale_and_impossible_values() {
        let old = AmdClockCounters {
            logical_processor: 0,
            actual_cycles: 10_000_000_000,
            reference_cycles: 8_000_000_000,
        };
        let current = AmdClockCounters {
            actual_cycles: old.actual_cycles + 4_000_000_000,
            reference_cycles: old.reference_cycles + 3_000_000_000,
            ..old
        };
        for bad in [
            AmdClockCounters {
                logical_processor: 1,
                ..current
            },
            AmdClockCounters {
                actual_cycles: 0,
                ..current
            },
            AmdClockCounters {
                reference_cycles: 0,
                ..current
            },
            AmdClockCounters {
                actual_cycles: u64::MAX,
                ..current
            },
            AmdClockCounters {
                reference_cycles: u64::MAX,
                ..current
            },
            AmdClockCounters {
                reference_cycles: old.reference_cycles,
                ..current
            },
            AmdClockCounters {
                actual_cycles: old.actual_cycles + 8_000_000_001,
                ..current
            },
        ] {
            assert!(amd_effective_mhz(old, bad, Duration::from_secs(1)).is_none());
        }
        assert!(amd_effective_mhz(old, current, Duration::from_millis(199)).is_none());
        assert!(amd_effective_mhz(old, current, Duration::from_millis(10_001)).is_none());
    }

    #[test]
    fn effective_clock_provenance_rejects_partial_duplicate_and_invalid_records() {
        let mut clocks = CpuEffectiveClocks {
            source: CpuClockSource::AmdAperfReadOnly,
            logical_processor_count: 2,
            processors: vec![
                CpuLogicalClock {
                    logical_processor: 0,
                    effective_mhz: 125.0,
                    interval_ms: 1000,
                },
                CpuLogicalClock {
                    logical_processor: 1,
                    effective_mhz: 4100.0,
                    interval_ms: 1001,
                },
            ],
        };
        assert!(clocks.validate().is_ok());
        assert_eq!(clocks.max_mhz(), Some(4100.0));
        let last = clocks.processors.pop().unwrap();
        assert!(clocks.validate().is_err());
        clocks.processors.push(last);
        clocks.processors[1].logical_processor = 0;
        assert!(clocks.validate().is_err());
        clocks.processors[1].logical_processor = 2;
        assert!(clocks.validate().is_err());
        clocks.processors[1].logical_processor = 1;
        clocks.processors[1].effective_mhz = f64::NAN;
        assert!(clocks.validate().is_err());
        clocks.processors[1].effective_mhz = 4000.0;
        clocks.processors[1].interval_ms = 10_001;
        assert!(clocks.validate().is_err());
        clocks.processors.clear();
        assert!(clocks.validate().is_err());
    }
}
