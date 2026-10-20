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

/// Plausibility bound for a measured Zen core clock; not a supported setting.
const AMD_CLOCK_MAX_MHZ: f64 = 7000.0;

/// Core frequency of an enabled Zen 2…4 P-state definition (MSRC001_0064…6B):
/// PstateEn bit 63, CpuFid bits 7:0, CpuDfsId bits 13:8, 200 MHz × Fid / Dfs.
pub fn amd_pstate_mhz(definition: u64) -> Option<u32> {
    if definition & (1 << 63) == 0 {
        return None;
    }
    let fid = (definition & 0xFF) as u32;
    let dfs = ((definition >> 8) & 0x3F) as u32;
    if dfs < 8 {
        return None;
    }
    let mhz = 200 * fid / dfs;
    (400..=AMD_CLOCK_MAX_MHZ as u32)
        .contains(&mhz)
        .then_some(mhz)
}

/// Read-only APERF/MPERF copies of one logical processor (MSRC000_00E8/E7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmdCoreCounters {
    pub aperf: u64,
    pub mperf: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmdCoreClock {
    /// Mean clock while in C0: P0 × ΔAPERF/ΔMPERF (AMD APM "effective
    /// frequency"). None when the processor was almost never awake.
    pub active_mhz: Option<f64>,
    /// Cycles over wall time, idle counted as zero (HWiNFO "effective clock").
    pub effective_mhz: f64,
}

/// One logical processor's clocks over its own measured interval. Counter
/// resets, a missing P0 or an implausible result yield no reading.
pub fn amd_core_clock(
    p0_mhz: u32,
    previous: AmdCoreCounters,
    current: AmdCoreCounters,
    elapsed: std::time::Duration,
) -> Option<AmdCoreClock> {
    if elapsed < std::time::Duration::from_millis(200)
        || elapsed > std::time::Duration::from_secs(10)
    {
        return None;
    }
    let aperf = current.aperf.checked_sub(previous.aperf)? as f64;
    let mperf = current.mperf.checked_sub(previous.mperf)? as f64;
    let micros = elapsed.as_secs_f64() * 1e6;
    let effective_mhz = aperf / micros;
    // MPERF counts at P0 only while awake; under 1 % of the interval in C0
    // leaves too few ticks for a meaningful ratio.
    let awake = mperf / (f64::from(p0_mhz) * micros);
    let active_mhz = (awake >= 0.01).then(|| f64::from(p0_mhz) * aperf / mperf);
    let plausible = |v: f64| v.is_finite() && (0.0..=AMD_CLOCK_MAX_MHZ).contains(&v);
    (plausible(effective_mhz) && awake <= 1.05 && active_mhz.is_none_or(plausible)).then_some(
        AmdCoreClock {
            active_mhz,
            effective_mhz,
        },
    )
}

/// Highest active clock (the boost clock actually reached) and the mean
/// effective clock over all logical processors.
pub fn amd_clock_summary(clocks: &[AmdCoreClock]) -> Option<(Option<u32>, u32)> {
    if clocks.is_empty() {
        return None;
    }
    let max = clocks
        .iter()
        .filter_map(|c| c.active_mhz)
        .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
    let mean = clocks.iter().map(|c| c.effective_mhz).sum::<f64>() / clocks.len() as f64;
    Some((max.map(|v| v.round() as u32), mean.round() as u32))
}

/// One CCD's temperature from THM_TCON (SMN 0x59800 + 0x308 + 4 × CCD on
/// family 19h models 60h…7Fh, as Linux k10temp): valid bit 11, 0.125 °C
/// steps from −49 °C.
pub fn amd_ccd_temperature_c(raw: u32) -> Option<f64> {
    if raw == u32::MAX || raw & (1 << 11) == 0 {
        return None;
    }
    let temperature = f64::from(raw & 0x7FF) * 0.125 - 49.0;
    (0.0..=125.0).contains(&temperature).then_some(temperature)
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
            max_core_clock_mhz: Some(5200),
            effective_clock_mhz: Some(1400),
            ccd_temperatures_c: vec![71.5, 66.0],
            warnings: vec![],
        };
        assert!(sample.validate().is_ok());
        sample.ccd_temperatures_c = vec![130.0];
        assert!(sample.validate().is_err());
        sample.ccd_temperatures_c.clear();
        sample.max_core_clock_mhz = Some(9000);
        assert!(sample.validate().is_err());
        sample.max_core_clock_mhz = None;
        sample.power_interval_ms = None;
        assert!(sample.validate().is_err());
        sample.package_power_w = None;
        assert!(sample.validate().is_ok());
        sample.temperature_c = Some(f64::NAN);
        assert!(sample.validate().is_err());
    }

    #[test]
    fn pstate_frequency_needs_an_enabled_definition_and_a_valid_divider() {
        // Fid 0x60, Dfs 8: 2400 MHz, the 8940HX base clock.
        assert_eq!(amd_pstate_mhz((1 << 63) | (8 << 8) | 0x60), Some(2400));
        assert_eq!(amd_pstate_mhz((8 << 8) | 0x60), None);
        assert_eq!(amd_pstate_mhz((1 << 63) | 0x60), None);
        assert_eq!(amd_pstate_mhz((1 << 63) | (8 << 8)), None);
    }

    #[test]
    fn core_clock_separates_active_and_effective_clock() {
        let a = AmdCoreCounters { aperf: 0, mperf: 0 };
        let second = std::time::Duration::from_secs(1);
        // Awake half the second at 5.0 GHz with P0 = 2.4 GHz.
        let half = AmdCoreCounters {
            aperf: 2_500_000_000,
            mperf: 1_200_000_000,
        };
        let c = amd_core_clock(2400, a, half, second).unwrap();
        assert!((c.active_mhz.unwrap() - 5000.0).abs() < 0.01);
        assert!((c.effective_mhz - 2500.0).abs() < 0.01);
        // Asleep: no active ratio, effective clock zero.
        let idle = amd_core_clock(2400, a, a, second).unwrap();
        assert_eq!(idle.active_mhz, None);
        assert_eq!(idle.effective_mhz, 0.0);
        let summary = amd_clock_summary(&[c, idle]).unwrap();
        assert_eq!(summary, (Some(5000), 1250));
        assert_eq!(amd_clock_summary(&[]), None);
    }

    #[test]
    fn core_clock_rejects_resets_wrong_p0_and_stale_intervals() {
        let a = AmdCoreCounters {
            aperf: 1000,
            mperf: 1000,
        };
        let second = std::time::Duration::from_secs(1);
        assert_eq!(
            amd_core_clock(
                2400,
                a,
                AmdCoreCounters {
                    aperf: 0,
                    mperf: 2000
                },
                second
            ),
            None
        );
        // MPERF faster than the claimed P0 means the P0 decode is wrong.
        let fast = AmdCoreCounters {
            aperf: 4_000_000_000,
            mperf: 3_000_000_000,
        };
        assert_eq!(amd_core_clock(2400, a, fast, second), None);
        assert_eq!(
            amd_core_clock(2400, a, a, std::time::Duration::from_millis(50)),
            None
        );
        assert_eq!(
            amd_core_clock(2400, a, a, std::time::Duration::from_secs(11)),
            None
        );
    }

    #[test]
    fn ccd_temperature_requires_the_valid_bit() {
        // (8 × 49 + 8 × 70) = 952 steps: 70 °C.
        assert_eq!(amd_ccd_temperature_c((1 << 11) | 952), Some(70.0));
        assert_eq!(amd_ccd_temperature_c(952), None);
        assert_eq!(amd_ccd_temperature_c(u32::MAX), None);
        assert_eq!(amd_ccd_temperature_c(1 << 11), None);
    }
}
