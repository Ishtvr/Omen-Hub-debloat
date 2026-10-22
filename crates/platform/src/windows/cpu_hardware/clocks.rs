//! Optional read-only APERF/MPERF telemetry. Own-thread affinity only; no HWCR
//! writes, counter enable/reset, requested clocks or CPU tuning entry points.
use super::{Affinity, Handle, Read, read_cell};
use omen_core::cpu::{
    AmdClockCounters, CpuClockSource, CpuEffectiveClocks, CpuLogicalClock, amd_effective_mhz,
};
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Threading::{
    GetActiveProcessorCount, GetActiveProcessorGroupCount, GetCurrentProcess,
    GetProcessAffinityMask,
};

fn processor_count() -> Result<u32, String> {
    #[cfg(target_arch = "x86_64")]
    #[allow(unused_unsafe)]
    {
        // AMD CPUID Fn8000_0007_EDX[10] (EffFreqRO) advertises the read-only
        // aliases. Do not enable the counters by changing HWCR if unavailable.
        let max = unsafe { std::arch::x86_64::__cpuid(0x80000000) }.eax;
        if max < 0x80000007
            || unsafe { std::arch::x86_64::__cpuid(0x80000007) }.edx & (1 << 10) == 0
        {
            return Err("AMD read-only effective-frequency counters are not advertised".into());
        }
    }
    let mut process_mask = 0;
    let mut system_mask = 0;
    if unsafe { GetActiveProcessorGroupCount() } != 1
        || unsafe {
            GetProcessAffinityMask(GetCurrentProcess(), &mut process_mask, &mut system_mask)
        } == 0
    {
        return Err("CPU clock processor-group/affinity coverage is unavailable".into());
    }
    let count = unsafe { GetActiveProcessorCount(0) };
    // Only the measured 8940HX topology is qualified. Restricted process masks
    // or holes cannot silently turn a subset into a system-wide measurement.
    if count != 32 || system_mask != 0xffff_ffff || process_mask != system_mask {
        return Err("CPU clock sampling requires all 32 qualified logical processors".into());
    }
    Ok(count)
}

struct Reading {
    counters: AmdClockCounters,
    at: Instant,
}

#[derive(Default)]
pub(super) struct ClockReader {
    baseline: Option<Vec<Reading>>,
}

impl ClockReader {
    pub(super) fn sample(&mut self, driver: &Handle) -> Result<Option<CpuEffectiveClocks>, String> {
        let result = self.read(driver);
        if result.is_err() {
            // A failure resets only the clock baseline; package watts/Tctl have
            // their own contracts and remain available in the parent reader.
            self.baseline = None;
        }
        result
    }

    fn read(&mut self, driver: &Handle) -> Result<Option<CpuEffectiveClocks>, String> {
        let count = processor_count()?;
        let sweep_start = Instant::now();
        let mut current = Vec::with_capacity(count as usize);
        for logical_processor in 0..count {
            let affinity = Affinity::pin(logical_processor)?;
            let reference_cycles = read_cell(driver, Read::ReferenceCycles)?;
            let start = Instant::now();
            let actual_cycles = read_cell(driver, Read::ActualCycles)?;
            let acquisition = start.elapsed();
            affinity.restore()?;
            if acquisition > Duration::from_millis(10) {
                return Err("CPU clock counter acquisition exceeded its timing bound".into());
            }
            if reference_cycles == u64::MAX || actual_cycles == u64::MAX {
                return Err("CPU clock counter returned an unavailable value".into());
            }
            current.push(Reading {
                counters: AmdClockCounters {
                    logical_processor,
                    actual_cycles,
                    reference_cycles,
                },
                at: start + acquisition / 2,
            });
        }
        if sweep_start.elapsed() > Duration::from_millis(100) {
            return Err("CPU clock acquisition sweep exceeded its timing bound".into());
        }
        self.observe(current)
    }

    fn observe(&mut self, current: Vec<Reading>) -> Result<Option<CpuEffectiveClocks>, String> {
        let count = current.len() as u32;
        let Some(previous) = self.baseline.take() else {
            self.baseline = Some(current);
            return Ok(None);
        };
        if previous.len() != current.len() {
            return Err("CPU clock processor coverage changed".into());
        }
        let processors = previous
            .iter()
            .zip(&current)
            .map(|(old, new)| {
                let elapsed = new
                    .at
                    .checked_duration_since(old.at)
                    .ok_or("CPU clock monotonic sample time changed")?;
                let effective_mhz = amd_effective_mhz(old.counters, new.counters, elapsed)
                    .ok_or("CPU clock counter reset, inconsistent values or stale interval")?;
                Ok(CpuLogicalClock {
                    logical_processor: new.counters.logical_processor,
                    effective_mhz,
                    interval_ms: elapsed.as_millis() as u64,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let clocks = CpuEffectiveClocks {
            source: CpuClockSource::AmdAperfReadOnly,
            logical_processor_count: count,
            processors,
        };
        clocks.validate()?;
        self.baseline = Some(current);
        Ok(Some(clocks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn readings(at: Instant, cycles: u64) -> Vec<Reading> {
        (0..32)
            .map(|logical_processor| Reading {
                counters: AmdClockCounters {
                    logical_processor,
                    actual_cycles: cycles,
                    reference_cycles: cycles,
                },
                at: at + Duration::from_micros(u64::from(logical_processor) * 10),
            })
            .collect()
    }

    #[test]
    fn first_sample_and_recreated_reader_require_a_fresh_baseline() {
        let now = Instant::now();
        let mut reader = ClockReader::default();
        assert!(reader.observe(readings(now, 1_000_000)).unwrap().is_none());
        let measured = reader
            .observe(readings(now + Duration::from_secs(1), 2_001_000_000))
            .unwrap()
            .unwrap();
        assert_eq!(measured.processors.len(), 32);
        assert_eq!(measured.max_mhz(), Some(2000.0));
        let mut resumed = ClockReader::default();
        assert!(
            resumed
                .observe(readings(now + Duration::from_secs(60), 50_000_000_000))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn one_bad_cpu_or_stale_interval_discards_the_entire_clock_baseline() {
        let now = Instant::now();
        let mut reader = ClockReader::default();
        reader.observe(readings(now, 5_000_000)).unwrap();
        let mut reset = readings(now + Duration::from_secs(1), 2_005_000_000);
        reset[17].counters.actual_cycles = 0;
        assert!(reader.observe(reset).is_err());
        assert!(reader.baseline.is_none());
        assert!(
            reader
                .observe(readings(now + Duration::from_secs(2), 4_000_000_000))
                .unwrap()
                .is_none()
        );
        assert!(
            reader
                .observe(readings(now + Duration::from_secs(20), 8_000_000_000))
                .is_err()
        );
        assert!(reader.baseline.is_none());
    }
}
