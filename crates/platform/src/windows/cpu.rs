//! Public read-only Windows CPU telemetry. No drivers or vendor SDK initialization.
use omen_core::{
    CpuSample,
    cpu::{CpuTimes, busy_percent},
};
use std::time::{Duration, Instant};
use windows_sys::Win32::{
    Foundation::FILETIME,
    System::{
        Power::{CallNtPowerInformation, PROCESSOR_POWER_INFORMATION, ProcessorInformation},
        Threading::{GetActiveProcessorCount, GetSystemTimes},
    },
};

fn ticks(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

fn system_times() -> Result<CpuTimes, String> {
    let mut idle = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut kernel, mut user) = (idle, idle);
    // SAFETY: Three correctly sized writable FILETIME structures.
    if unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } == 0 {
        return Err(format!(
            "CPU timing unavailable: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(CpuTimes {
        idle: ticks(idle),
        kernel: ticks(kernel),
        user: ticks(user),
    })
}

pub struct Cpu {
    count: u32,
    previous: Option<CpuTimes>,
    sampled_at: Instant,
    clocks: Vec<PROCESSOR_POWER_INFORMATION>,
}
impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}
impl Cpu {
    pub fn new() -> Self {
        // SAFETY: ALL_PROCESSOR_GROUPS is the documented sentinel; this only reads a count.
        let count = unsafe { GetActiveProcessorCount(u16::MAX) };
        let supported = (1..=64).contains(&count);
        Self {
            count,
            previous: if supported { system_times().ok() } else { None },
            sampled_at: Instant::now(),
            // Beyond one group GetSystemTimes does not report a global total.
            // Until a group-aware adapter exists, avoid showing a misleading partial result.
            clocks: if supported {
                vec![PROCESSOR_POWER_INFORMATION::default(); count as usize]
            } else {
                vec![]
            },
        }
    }
    pub fn sample(&mut self) -> CpuSample {
        let mut sample = CpuSample {
            logical_processors: (self.count != 0).then_some(self.count),
            ..Default::default()
        };
        if self.clocks.is_empty() {
            sample.warnings.push("CPU telemetry currently requires a single processor group (1–64 logical processors)".into());
            return sample;
        }
        if self.sampled_at.elapsed() >= Duration::from_millis(200) {
            match system_times() {
                Ok(current) => {
                    if let Some(previous) = self.previous.replace(current) {
                        sample.busy_percent = busy_percent(previous, current);
                        if sample.busy_percent.is_none() {
                            sample.warnings.push("CPU timing counters reset or were inconsistent; taking a fresh baseline".into());
                        }
                    }
                }
                Err(error) => {
                    self.previous = None;
                    sample.warnings.push(error);
                }
            }
            self.sampled_at = Instant::now();
        }
        self.clocks.fill(PROCESSOR_POWER_INFORMATION::default());
        // SAFETY: ProcessorInformation is a GET with null input. The allocated
        // output holds one SDK-defined structure for every logical processor.
        let status = unsafe {
            CallNtPowerInformation(
                ProcessorInformation,
                std::ptr::null(),
                0,
                self.clocks.as_mut_ptr().cast(),
                std::mem::size_of_val(self.clocks.as_slice()) as u32,
            )
        };
        if status == 0 {
            let count = self.clocks.iter().filter(|cpu| cpu.CurrentMhz > 0).count();
            if count == self.clocks.len() {
                let total: u64 = self
                    .clocks
                    .iter()
                    .map(|cpu| u64::from(cpu.CurrentMhz))
                    .sum();
                sample.windows_clock_mhz = Some((total / count as u64) as u32);
            } else {
                sample
                    .warnings
                    .push("Windows CPU clock response was incomplete".into());
            }
        } else {
            sample
                .warnings
                .push(format!("Windows CPU clock unavailable: {status:#x}"));
        }
        sample
    }
}
