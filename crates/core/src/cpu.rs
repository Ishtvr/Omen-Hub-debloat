//! CPU busy-time calculation, independent of Windows and hardware adapters.

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
}
