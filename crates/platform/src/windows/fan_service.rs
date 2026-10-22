//! Background fan control that replaces the OMEN Hub's software fan loop.
//! It runs elevated from the user's logon task, follows the saved shared
//! curve and stops sending when paused or shut down. Firmware ownership has
//! no qualified GET; stopping alone is not proof of a BIOS handover.
//! The same service pins the foreground program to the saved CCD selection.
use super::{
    CpuHardwareReader,
    hp::{HpBios, Read},
    nvidia::GpuTemperature,
};
use omen_core::{
    Inventory,
    cooling::FirmwareIrTemperature,
    fan::{self, FanCurve, Smoother},
    profiles::FanPoint,
    quick_switch::FanMode,
};
use serde::Serialize;
use std::time::{Duration, Instant};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, HANDLE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
        Threading::{
            CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, OpenEventW, ReleaseMutex, ResetEvent,
            SetEvent, WaitForSingleObject,
        },
    },
};

const CYCLE: Duration = Duration::from_secs(2);
/// The Hub sends its heartbeat every 30 s; stay well inside that window.
const HEARTBEAT: Duration = Duration::from_secs(20);
/// While the original Hub runs it owns the fans; the service waits.
const HUB_PROCESSES: [&str; 2] = [
    "omencommandcenterbackground.exe",
    "hp.omen.application.background.tasks.container.exe",
];

/// One controller per interactive user/session, with an interruptible wait.
/// Installers request a graceful stop so CCD originals can be released before
/// replacing the executable. This is independent of the short hardware lease.
struct Lifecycle {
    mutex: HANDLE,
    stop: HANDLE,
}
fn object_name(kind: &str) -> Result<Vec<u16>, String> {
    Ok(
        format!("Local\\OmenHubPP.Fan.{kind}.{}", super::current_user_sid()?)
            .encode_utf16()
            .chain(Some(0))
            .collect(),
    )
}
impl Lifecycle {
    fn start() -> Result<Self, String> {
        let name = object_name("Owner")?;
        // SAFETY: Terminated per-user name, default security, no initial owner.
        let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        if mutex.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // SAFETY: Valid mutex, bounded acquisition on this thread.
        let wait = unsafe { WaitForSingleObject(mutex, 0) };
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            unsafe {
                CloseHandle(mutex);
            }
            return Err("An Omen Hub++ fan controller is already running in this session".into());
        }
        let mut owner = Self {
            mutex,
            stop: std::ptr::null_mut(),
        };
        let name = object_name("Stop")?;
        // SAFETY: Manual-reset event, fixed name and default security.
        owner.stop = unsafe { CreateEventW(std::ptr::null(), 1, 0, name.as_ptr()) };
        if owner.stop.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // A completed prior instance may have left the named event signalled.
        if unsafe { ResetEvent(owner.stop) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(owner)
    }
    fn wait(&self, timeout: Duration) -> Result<bool, String> {
        // SAFETY: Owned event; each wait is bounded and interruptible.
        match unsafe {
            WaitForSingleObject(
                self.stop,
                timeout.as_millis().min(u128::from(u32::MAX - 1)) as u32,
            )
        } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(std::io::Error::last_os_error().to_string()),
        }
    }
}
impl Drop for Lifecycle {
    fn drop(&mut self) {
        // SAFETY: This thread acquired the mutex and owns both handles.
        unsafe {
            if !self.stop.is_null() {
                CloseHandle(self.stop);
            }
            ReleaseMutex(self.mutex);
            CloseHandle(self.mutex);
        }
    }
}
pub fn request_stop() -> Result<bool, String> {
    let name = object_name("Stop")?;
    // SAFETY: Opens only the per-user fixed stop event, without inheritance.
    let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, 0, name.as_ptr()) };
    if event.is_null() {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(2) {
            Ok(false)
        } else {
            Err(error.to_string())
        };
    }
    let success = unsafe { SetEvent(event) } != 0;
    let error = std::io::Error::last_os_error();
    unsafe {
        CloseHandle(event);
    }
    if success {
        Ok(true)
    } else {
        Err(error.to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct FanServiceStatus {
    pub schema: u32,
    pub state: &'static str,
    pub detail: String,
    pub cpu_c: Option<f64>,
    pub gpu_c: Option<i32>,
    pub ir_c: Option<u8>,
    pub protection: u8,
    pub cpu_level: u8,
    pub gpu_level: u8,
    /// Programs currently pinned to the saved CCD selection.
    pub ccd_pinned: usize,
    pub updated_unix_s: u64,
}

fn hub_running() -> bool {
    // SAFETY: Snapshot handle is closed below; the entry is sized as required.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            // Unknown ownership must not start a second controller.
            return true;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut ok = Process32FirstW(snapshot, &mut entry) != 0;
        if !ok {
            CloseHandle(snapshot);
            return true;
        }
        while ok && !found {
            let length = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..length]).to_lowercase();
            found = HUB_PROCESSES.contains(&name.as_str());
            ok = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        found
    }
}

/// Gives every pinned program its affinity back when the service ends,
/// including an early error return.
struct Pinning {
    enforcer: omen_core::ccd::Enforcer,
    processes: super::ccd::WindowsProcesses,
}
impl Drop for Pinning {
    fn drop(&mut self) {
        self.enforcer.release(&mut self.processes);
    }
}

fn curve_of(points: &[FanPoint]) -> Result<FanCurve, String> {
    FanCurve::new(
        points
            .iter()
            .map(|p| fan::Point {
                temperature_c: i32::from(p.temperature_c),
                percent: p.duty_percent,
            })
            .collect(),
    )
    .map_err(String::from)
}

fn requested_level(
    mode: FanMode,
    curve: Option<&FanCurve>,
    full: bool,
    temperature: Option<i32>,
) -> u8 {
    if mode == FanMode::Max || full {
        return fan::MAX_LEVEL;
    }
    curve.map_or(fan::MAX_LEVEL, |curve| {
        fan::level(curve.target(temperature))
    })
}

/// Runs until the process ends. `mode` is the dashboard fan switch: Auto
/// leaves the BIOS table in charge, Curve follows `load` (the active shared
/// curve; None also leaves the BIOS in charge) and Max runs both fans at
/// maximum. `ccd` returns the saved CCD selection (None: every CCD).
/// `report` persists each status.
pub fn run(
    inventory: &Inventory,
    mode: impl FnMut() -> FanMode,
    load: impl FnMut() -> Result<Option<Vec<FanPoint>>, String>,
    ccd: impl FnMut() -> Result<Option<u8>, String>,
    report: impl FnMut(&FanServiceStatus),
) -> Result<(), String> {
    let lifecycle = Lifecycle::start()?;
    run_until(inventory, mode, load, ccd, report, |wait| {
        lifecycle.wait(wait)
    })
}

/// Same production loop with a bounded stop source for qualification. Stopping
/// releases CCD affinities and ceases firmware heartbeat/level writes.
pub fn run_until(
    inventory: &Inventory,
    mut mode: impl FnMut() -> FanMode,
    mut load: impl FnMut() -> Result<Option<Vec<FanPoint>>, String>,
    mut ccd: impl FnMut() -> Result<Option<u8>, String>,
    mut report: impl FnMut(&FanServiceStatus),
    mut stop: impl FnMut(Duration) -> Result<bool, String>,
) -> Result<(), String> {
    if inventory.manufacturer != "HP" || inventory.board != "8E35" || inventory.bios != "F.13" {
        return Err("Fan control is qualified only for HP8E35/F.13".into());
    }
    let bios = HpBios::new(inventory).map_err(|e| e.to_string())?;
    // Idle BIOS/Hub-waiting states do not open a kernel reader or NVIDIA DLL.
    let mut cpu = None;
    let mut gpu: Option<GpuTemperature> = None;
    let mut smoother = (Smoother::default(), Smoother::default());
    let mut last_heartbeat: Option<Instant> = None;
    let mut protection = 0u8;
    let ccd_masks = super::ccd::ccd_masks();
    let mut pinning = Pinning {
        enforcer: omen_core::ccd::Enforcer::default(),
        processes: super::ccd::WindowsProcesses::default(),
    };
    loop {
        if stop(Duration::ZERO)? {
            return Ok(());
        }
        let started = Instant::now();
        // CCD pinning is independent of the fans and of the original Hub. An
        // unreadable selection or topology releases every pinned program.
        let target = match (&ccd_masks, ccd()) {
            (Ok(masks), Ok(Some(selection))) => {
                omen_core::ccd::affinity(selection, masks).unwrap_or(None)
            }
            _ => None,
        };
        let Pinning {
            enforcer,
            processes,
        } = &mut pinning;
        enforcer.tick(processes, target);
        let ccd_pinned = enforcer.pinned();
        let now = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        };
        let idle = |state: &'static str, detail: String| FanServiceStatus {
            schema: 1,
            state,
            detail,
            cpu_c: None,
            gpu_c: None,
            ir_c: None,
            protection: 0,
            cpu_level: 0,
            gpu_level: 0,
            ccd_pinned,
            updated_unix_s: now(),
        };
        // Pause our controller by sending nothing. A BIOS handover cannot be
        // asserted from this status: ownership has no qualified readback.
        if hub_running() {
            last_heartbeat = None;
            cpu = None;
            gpu = None;
            report(&idle(
                "waiting_for_hub",
                omen_core::message::msg("fan.status.hub_running", &[]),
            ));
            if stop(Duration::from_secs(2))? {
                return Ok(());
            }
            continue;
        }
        let mode = mode();
        if mode == FanMode::Bios {
            last_heartbeat = None;
            cpu = None;
            gpu = None;
            report(&idle(
                "bios",
                omen_core::message::msg("fan.status.bios", &[]),
            ));
            if stop(Duration::from_secs(2))? {
                return Ok(());
            }
            continue;
        }
        let max = mode == FanMode::Max;
        let curve = match load().and_then(|p| p.map(|p| curve_of(&p)).transpose()) {
            Ok(curve) if max => curve,
            Ok(Some(curve)) => Some(curve),
            Ok(None) => {
                last_heartbeat = None;
                cpu = None;
                gpu = None;
                report(&idle(
                    "bios",
                    omen_core::message::msg("fan.status.no_curve", &[]),
                ));
                if stop(Duration::from_secs(2))? {
                    return Ok(());
                }
                continue;
            }
            Err(_) if max => None,
            Err(e) => {
                last_heartbeat = None;
                cpu = None;
                gpu = None;
                report(&idle(
                    "bios",
                    omen_core::message::msg("fan.status.invalid_curve", &[&e]),
                ));
                if stop(Duration::from_secs(2))? {
                    return Ok(());
                }
                continue;
            }
        };
        if last_heartbeat.is_none() {
            // Qualify the complete measured fan layout before the first
            // heartbeat/level SET, including a new arm after BIOS/Hub waiting.
            super::cooling::FanControl::new(inventory)?.capture()?;
        }
        if last_heartbeat.is_none_or(|t| t.elapsed() >= HEARTBEAT) {
            protection = bios.heartbeat().map_err(|e| e.to_string())?;
            last_heartbeat = Some(Instant::now());
        }
        if max {
            cpu = None;
            gpu = None;
        }
        if !max && cpu.is_none() {
            cpu = CpuHardwareReader::new(inventory).ok();
        }
        let cpu_c = cpu
            .as_mut()
            .and_then(|r| r.temperature_sample().ok())
            .and_then(|s| s.temperature_c);
        if !max && gpu.is_none() {
            gpu = GpuTemperature::new().ok();
        }
        let gpu_c = gpu.as_ref().and_then(|reader| reader.sample().ok());
        if gpu_c.is_none() {
            gpu = None;
        }
        let ir_c = bios
            .get(Read::SurfaceSensor)
            .ok()
            .and_then(|(code, bytes)| FirmwareIrTemperature::decode(code, &bytes).ok())
            .map(|t| t.celsius);
        // Maximum is unconditional and works without loading sensor drivers.
        // A curve may never lower cooling when its GPU reader is unavailable.
        let full = !max
            && (gpu_c.is_none()
                || fan::emergency(
                    cpu_c.map(|t| t as f32),
                    gpu_c.map(|t| t as f32),
                    ir_c,
                    protection,
                ));
        let target = |t: Option<i32>| requested_level(mode, curve.as_ref(), full, t);
        let cpu_t = cpu_c.map(|t| t.round() as i32);
        // The GPU fan follows the hotter of GPU and CPU when the GPU sleeps.
        let gpu_t = gpu_c.or(cpu_t);
        let cpu_level = smoother.0.step(target(cpu_t));
        let gpu_level = smoother.1.step(target(gpu_t));
        bios.set_fan_levels(cpu_level, gpu_level)
            .map_err(|e| e.to_string())?;
        report(&FanServiceStatus {
            schema: 1,
            state: if full {
                "full_cooling"
            } else if max {
                "max"
            } else {
                "curve"
            },
            detail: String::new(),
            cpu_c,
            gpu_c,
            ir_c,
            protection,
            cpu_level,
            gpu_level,
            ccd_pinned,
            updated_unix_s: now(),
        });
        if stop(CYCLE.saturating_sub(started.elapsed()))? {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maximum_and_emergency_ignore_a_saved_quiet_curve() {
        let curve = curve_of(&[
            FanPoint {
                temperature_c: 30,
                duty_percent: 25,
            },
            FanPoint {
                temperature_c: 80,
                duty_percent: 70,
            },
        ])
        .unwrap();
        assert!(requested_level(FanMode::Curve, Some(&curve), false, Some(35)) < fan::MAX_LEVEL);
        assert_eq!(
            requested_level(FanMode::Max, Some(&curve), false, Some(35)),
            fan::MAX_LEVEL
        );
        assert_eq!(
            requested_level(FanMode::Curve, Some(&curve), true, Some(35)),
            fan::MAX_LEVEL
        );
        assert_eq!(
            requested_level(FanMode::Curve, None, false, None),
            fan::MAX_LEVEL
        );
    }

    #[test]
    fn curve_of_validates_and_interpolates() {
        let valid = vec![
            FanPoint {
                temperature_c: 40,
                duty_percent: 30,
            },
            FanPoint {
                temperature_c: 80,
                duty_percent: 90,
            },
        ];
        let curve = curve_of(&valid).unwrap();
        assert_eq!(curve.target(Some(60)), 60);

        let single = vec![FanPoint {
            temperature_c: 40,
            duty_percent: 30,
        }];
        assert!(curve_of(&single).is_err());

        let non_monotonic = vec![
            FanPoint {
                temperature_c: 80,
                duty_percent: 90,
            },
            FanPoint {
                temperature_c: 40,
                duty_percent: 30,
            },
        ];
        assert!(curve_of(&non_monotonic).is_err());
    }

    #[test]
    fn fan_service_status_serializes_with_schema() {
        let status = FanServiceStatus {
            schema: 1,
            state: "curve",
            detail: String::new(),
            cpu_c: Some(65.5),
            gpu_c: Some(58),
            ir_c: Some(42),
            protection: 0,
            cpu_level: 25,
            gpu_level: 25,
            ccd_pinned: 1,
            updated_unix_s: 1234567890,
        };
        let text = serde_json::to_string(&status).unwrap();
        assert!(text.contains(r#""schema":1"#));
        assert!(text.contains(r#""state":"curve""#));
        assert!(text.contains(r#""cpu_level":25"#));
    }
}
