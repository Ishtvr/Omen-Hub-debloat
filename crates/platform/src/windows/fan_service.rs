//! Background fan control that replaces the OMEN Hub's software fan loop.
//! It runs elevated from the user's logon task, follows the saved shared
//! curve and stops sending when paused or shut down. Firmware ownership has
//! no qualified GET; stopping alone is not proof of a BIOS handover.
//! The same service pins the foreground program to the saved CCD selection.
use super::{
    ControlLease, CpuHardwareReader,
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
        CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    },
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
        Threading::{
            CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, GetCurrentProcess, GetProcessTimes,
            OpenEventW, ReleaseMutex, ResetEvent, SetEvent, WaitForSingleObject,
        },
    },
};

const CYCLE: Duration = Duration::from_secs(2);
/// The Hub sends its heartbeat every 30 s; stay well inside that window.
const HEARTBEAT: Duration = Duration::from_secs(20);
const LEASE_RETRY: Duration = Duration::from_millis(50);
const CYCLE_LEASE_ATTEMPTS: usize = 6;
const RELEASE_LEASE_ATTEMPTS: usize = 41;
const HARDWARE_BUSY: &str = super::control_lease::HARDWARE_BUSY;

enum WriteAccess<T> {
    Acquired(T),
    Busy,
    Stop,
}
/// Every acquisition is nonblocking. Bound only the explicit retry waits;
/// failed access/handle creation remains an error, not presumed contention.
fn acquire_write_access<T>(
    attempts: usize,
    mut acquire: impl FnMut() -> Result<T, String>,
    mut wait: impl FnMut(Duration) -> Result<bool, String>,
) -> Result<WriteAccess<T>, String> {
    for attempt in 0..attempts {
        match acquire() {
            Ok(lease) => return Ok(WriteAccess::Acquired(lease)),
            Err(error) if error == HARDWARE_BUSY => {
                if attempt + 1 < attempts && wait(LEASE_RETRY)? {
                    return Ok(WriteAccess::Stop);
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(WriteAccess::Busy)
}

#[derive(Clone, Copy)]
struct ProcessIdentity {
    id: u32,
    started: u64,
}
impl ProcessIdentity {
    fn current() -> Result<Self, String> {
        let mut created: FILETIME = unsafe { std::mem::zeroed() };
        let mut exited = created;
        let mut kernel = created;
        let mut user = created;
        // SAFETY: Current-process pseudo-handle and exact writable FILETIMEs.
        if unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let started = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        if started == 0 {
            return Err("Fan process creation identity is unavailable".into());
        }
        Ok(Self {
            id: std::process::id(),
            started,
        })
    }
}
fn heartbeat_needed(
    previous: Option<FanMode>,
    mode: FanMode,
    elapsed: Option<Duration>,
    blocked: bool,
) -> bool {
    (mode == FanMode::Max && previous != Some(FanMode::Max))
        || blocked
        || elapsed.is_none_or(|elapsed| elapsed >= HEARTBEAT)
}
fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|time| time.as_millis().try_into().ok())
        .unwrap_or(0)
}

#[derive(Debug, PartialEq, Eq)]
enum CoolingRelease {
    NoWrite,
    Maximum,
    Vendor,
}
fn release_locked<T>(
    control: &mut fan::SoftwareControl,
    acquire: impl FnOnce() -> Result<T, String>,
    vendor: impl FnOnce() -> Result<bool, String>,
    maximum: impl FnOnce() -> Result<(), String>,
) -> Result<CoolingRelease, String> {
    let mut vendor_took_over = false;
    let issued = control.release(|| {
        let _lease = acquire()?;
        if vendor()? {
            vendor_took_over = true;
            Ok(())
        } else {
            maximum()
        }
    })?;
    if vendor_took_over {
        control.vendor_active();
        Ok(CoolingRelease::Vendor)
    } else if issued {
        Ok(CoolingRelease::Maximum)
    } else {
        Ok(CoolingRelease::NoWrite)
    }
}
fn release_maximum(
    control: &mut fan::SoftwareControl,
    bios: &HpBios,
) -> Result<CoolingRelease, String> {
    release_locked(
        control,
        || match acquire_write_access(RELEASE_LEASE_ATTEMPTS, ControlLease::acquire, |delay| {
            std::thread::sleep(delay);
            Ok(false)
        })? {
            WriteAccess::Acquired(lease) => Ok(lease),
            _ => Err(
                "Hardware transaction stayed busy; maximum-cooling handover is unconfirmed".into(),
            ),
        },
        || {
            hub_running().ok_or(
                "Controller ownership is unknown; safe cooling release cannot be dispatched".into(),
            )
        },
        || {
            bios.set_fan_levels(fan::MAX_LEVEL, fan::MAX_LEVEL)
                .map_err(|e| e.to_string())
        },
    )
}
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
    pub process_id: u32,
    /// Kernel process creation FILETIME, paired with PID to reject stale files.
    pub process_started: u64,
    /// Last successful heartbeat in this process, never a status refresh time.
    pub last_heartbeat_unix_ms: Option<u64>,
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
    pub updated_unix_ms: u64,
}

/// None means enumeration failed, not that a vendor actually took ownership.
fn hub_running() -> Option<bool> {
    // SAFETY: Snapshot handle is closed below; the entry is sized as required.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            // Unknown ownership must not start a second controller.
            return None;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut ok = Process32FirstW(snapshot, &mut entry) != 0;
        if !ok {
            CloseHandle(snapshot);
            return None;
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
        // A failure in the middle of enumeration must not masquerade as an
        // absent controller. ERROR_NO_MORE_FILES is the normal end marker.
        let complete = found || std::io::Error::last_os_error().raw_os_error() == Some(18);
        CloseHandle(snapshot);
        complete.then_some(found)
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

/// Runs until the process ends. Auto pauses the software loop after a safe
/// release; it does not assert a firmware ownership GET. Curve follows `load`
/// and Max runs both fans at maximum. `ccd` returns the saved CCD selection.
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
    let instance = ProcessIdentity::current()?;
    let bios = HpBios::new(inventory).map_err(|e| e.to_string())?;
    // Idle BIOS/Hub-waiting states do not open a kernel reader or NVIDIA DLL.
    let mut cpu = None;
    let mut gpu: Option<GpuTemperature> = None;
    let mut smoother = (Smoother::default(), Smoother::default());
    let mut last_heartbeat: Option<Instant> = None;
    let mut last_heartbeat_unix_ms = None;
    let mut last_written_mode = None;
    let mut write_was_blocked = false;
    let mut control = fan::SoftwareControl::default();
    let mut protection = 0u8;
    let ccd_masks = super::ccd::ccd_masks();
    let mut pinning = Pinning {
        enforcer: omen_core::ccd::Enforcer::default(),
        processes: super::ccd::WindowsProcesses::default(),
    };
    let result = (|| -> Result<(), String> {
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
            let heartbeat_for_idle = last_heartbeat_unix_ms;
            let idle = |state: &'static str, detail: String| {
                let updated_unix_ms = unix_ms();
                FanServiceStatus {
                    schema: 1,
                    process_id: instance.id,
                    process_started: instance.started,
                    last_heartbeat_unix_ms: heartbeat_for_idle,
                    state,
                    detail,
                    cpu_c: None,
                    gpu_c: None,
                    ir_c: None,
                    protection: 0,
                    cpu_level: 0,
                    gpu_level: 0,
                    ccd_pinned,
                    updated_unix_s: updated_unix_ms / 1000,
                    updated_unix_ms,
                }
            };
            let pause = |control: &mut fan::SoftwareControl,
                         detail: String|
             -> Result<FanServiceStatus, String> {
                let release = release_maximum(control, &bios)?;
                if release == CoolingRelease::Vendor {
                    return Ok(idle(
                        "waiting_for_hub",
                        omen_core::message::msg("fan.status.hub_running", &[]),
                    ));
                }
                if control.handover_pending() {
                    Ok(FanServiceStatus {
                        cpu_level: fan::MAX_LEVEL,
                        gpu_level: fan::MAX_LEVEL,
                        ..idle(
                            "handover_pending",
                            omen_core::message::msg("fan.status.handover", &[]),
                        )
                    })
                } else {
                    Ok(idle("bios", detail))
                }
            };
            // Pause our controller by sending nothing. A BIOS handover cannot be
            // asserted from this status: ownership has no qualified readback.
            let vendor = hub_running()
                .ok_or("Original controller enumeration failed; ownership is unknown")?;
            if vendor {
                control.vendor_active();
                last_heartbeat = None;
                last_written_mode = None;
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
                report(&pause(
                    &mut control,
                    omen_core::message::msg("fan.status.bios", &[]),
                )?);
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
                    report(&pause(
                        &mut control,
                        omen_core::message::msg("fan.status.no_curve", &[]),
                    )?);
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
                    report(&pause(
                        &mut control,
                        omen_core::message::msg("fan.status.invalid_curve", &[&e]),
                    )?);
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
            // All sensor/layout reads above happen without the hardware lease.
            // A bounded busy cycle preserves responsibility and stays alive;
            // it must not publish a fresh Max acknowledgement or renew OMCC.
            let write_lease = match acquire_write_access(
                CYCLE_LEASE_ATTEMPTS,
                ControlLease::acquire,
                &mut stop,
            )? {
                WriteAccess::Acquired(lease) => lease,
                WriteAccess::Stop => return Ok(()),
                WriteAccess::Busy => {
                    write_was_blocked = true;
                    report(&idle("waiting_for_control", "Hardware transaction is busy; cooling output is pending and maximum will be established when the lease returns".into()));
                    if stop(CYCLE.saturating_sub(started.elapsed()))? {
                        return Ok(());
                    }
                    continue;
                }
            };
            if stop(Duration::ZERO)? {
                return Ok(());
            }
            if hub_running().ok_or("Original controller enumeration failed before fan write")? {
                control.vendor_active();
                last_heartbeat = None;
                last_written_mode = None;
                drop(write_lease);
                report(&idle(
                    "waiting_for_hub",
                    omen_core::message::msg("fan.status.hub_running", &[]),
                ));
                if stop(CYCLE.saturating_sub(started.elapsed()))? {
                    return Ok(());
                }
                continue;
            }
            if heartbeat_needed(
                last_written_mode,
                mode,
                last_heartbeat.map(|time| time.elapsed()),
                write_was_blocked,
            ) {
                control.arm();
                protection = bios.heartbeat().map_err(|e| e.to_string())?;
                last_heartbeat = Some(Instant::now());
                last_heartbeat_unix_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .and_then(|time| time.as_millis().try_into().ok());
            }
            // Maximum is unconditional and works without loading sensor drivers.
            // A curve may never lower cooling when its GPU reader is unavailable.
            let full = !max
                && (write_was_blocked
                    || gpu_c.is_none()
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
            control.arm();
            bios.set_fan_levels(cpu_level, gpu_level)
                .map_err(|e| e.to_string())?;
            last_written_mode = Some(mode);
            write_was_blocked = false;
            drop(write_lease);
            let updated_unix_ms = unix_ms();
            report(&FanServiceStatus {
                schema: 1,
                process_id: instance.id,
                process_started: instance.started,
                last_heartbeat_unix_ms,
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
                updated_unix_s: updated_unix_ms / 1000,
                updated_unix_ms,
            });
            if stop(CYCLE.saturating_sub(started.elapsed()))? {
                return Ok(());
            }
        }
    })();
    // A clean stop and a failed attempt use the same conservative release.
    // Never overwrite a positively identified vendor controller. If ownership
    // cannot be enumerated, retain an explicit error instead of claiming Auto.
    let release = match hub_running() {
        Some(true) => {
            control.vendor_active();
            Ok(CoolingRelease::Vendor)
        }
        Some(false) => release_maximum(&mut control, &bios),
        None => Err("Controller ownership is unknown; safe release could not be dispatched".into()),
    };
    drop(pinning);
    let updated_unix_ms = unix_ms();
    if control.handover_pending() && release.is_ok() {
        report(&FanServiceStatus {
            schema: 1,
            process_id: instance.id,
            process_started: instance.started,
            last_heartbeat_unix_ms,
            state: "handover_pending",
            detail: omen_core::message::msg("fan.status.handover", &[]),
            cpu_c: None,
            gpu_c: None,
            ir_c: None,
            protection: 0,
            cpu_level: fan::MAX_LEVEL,
            gpu_level: fan::MAX_LEVEL,
            ccd_pinned: 0,
            updated_unix_s: updated_unix_ms / 1000,
            updated_unix_ms,
        });
    } else if let Err(error) = &release {
        report(&FanServiceStatus {
            schema: 1,
            process_id: instance.id,
            process_started: instance.started,
            last_heartbeat_unix_ms,
            state: "handover_failed",
            detail: error.clone(),
            cpu_c: None,
            gpu_c: None,
            ir_c: None,
            protection: 0,
            cpu_level: 0,
            gpu_level: 0,
            ccd_pinned: 0,
            updated_unix_s: updated_unix_ms / 1000,
            updated_unix_ms,
        });
    }
    match (result, release) {
        (Ok(()), Ok(_)) => Ok(()),
        (Err(error), Ok(_)) => Err(error),
        (Ok(()), Err(error)) => Err(format!("Safe fan release failed: {error}")),
        (Err(error), Err(release)) => Err(format!("{error}; safe fan release failed: {release}")),
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
            process_id: 42,
            process_started: 1234,
            last_heartbeat_unix_ms: Some(1234567889000),
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
            updated_unix_ms: 1234567890123,
        };
        let text = serde_json::to_string(&status).unwrap();
        assert!(text.contains(r#""schema":1"#));
        assert!(text.contains(r#""state":"curve""#));
        assert!(text.contains(r#""cpu_level":25"#));
        assert!(text.contains(r#""process_id":42"#));
        assert!(text.contains(r#""process_started":1234"#));
        assert!(text.contains(r#""last_heartbeat_unix_ms":1234567889000"#));
        assert!(text.contains(r#""updated_unix_ms":1234567890123"#));
    }

    #[test]
    fn write_contention_has_bounded_wait_and_honors_stop() {
        let mut calls = 0;
        let mut waited = Duration::ZERO;
        let access = acquire_write_access(
            CYCLE_LEASE_ATTEMPTS,
            || {
                calls += 1;
                Err::<(), _>(HARDWARE_BUSY.into())
            },
            |duration| {
                waited += duration;
                Ok(false)
            },
        )
        .unwrap();
        assert!(matches!(access, WriteAccess::Busy));
        assert_eq!(calls, CYCLE_LEASE_ATTEMPTS);
        assert_eq!(waited, Duration::from_millis(250));
        let access = acquire_write_access(
            CYCLE_LEASE_ATTEMPTS,
            || Err::<(), _>(HARDWARE_BUSY.into()),
            |_| Ok(true),
        )
        .unwrap();
        assert!(matches!(access, WriteAccess::Stop));
        assert!(
            acquire_write_access(
                CYCLE_LEASE_ATTEMPTS,
                || Err::<(), _>("access denied".into()),
                |_| panic!("permission failure must not retry")
            )
            .is_err()
        );
    }

    #[test]
    fn cooling_release_holds_lease_and_rechecks_vendor_before_writing() {
        use std::{cell::Cell, rc::Rc};
        struct Lease(Rc<Cell<bool>>);
        impl Drop for Lease {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        let held = Rc::new(Cell::new(false));
        let mut control = fan::SoftwareControl::default();
        assert_eq!(
            release_locked::<()>(
                &mut control,
                || panic!("idle must not acquire"),
                || panic!("idle must not query"),
                || panic!("idle must not write")
            )
            .unwrap(),
            CoolingRelease::NoWrite
        );
        control.arm();
        assert_eq!(
            release_locked(
                &mut control,
                || {
                    held.set(true);
                    Ok(Lease(held.clone()))
                },
                || {
                    assert!(held.get());
                    Ok(false)
                },
                || {
                    assert!(held.get());
                    Ok(())
                }
            )
            .unwrap(),
            CoolingRelease::Maximum
        );
        assert!(!held.get());
        control.arm();
        assert_eq!(
            release_locked(
                &mut control,
                || {
                    held.set(true);
                    Ok(Lease(held.clone()))
                },
                || {
                    assert!(held.get());
                    Ok(true)
                },
                || panic!("do not overwrite a newly active vendor")
            )
            .unwrap(),
            CoolingRelease::Vendor
        );
        assert!(!held.get());
        assert!(!control.handover_pending());
    }

    #[test]
    fn failed_lease_does_not_acknowledge_maximum_or_drop_responsibility() {
        let mut control = fan::SoftwareControl::default();
        control.arm();
        assert!(
            release_locked::<()>(
                &mut control,
                || Err("busy".into()),
                || panic!("no lease"),
                || panic!("no lease")
            )
            .is_err()
        );
        assert!(!control.handover_pending());
        assert_eq!(
            release_locked(&mut control, || Ok(()), || Ok(false), || Ok(())).unwrap(),
            CoolingRelease::Maximum
        );
        assert!(control.handover_pending());
    }

    #[test]
    fn entering_max_and_resuming_after_contention_require_a_fresh_heartbeat() {
        let recent = Some(Duration::from_secs(1));
        assert!(heartbeat_needed(
            Some(FanMode::Curve),
            FanMode::Max,
            recent,
            false
        ));
        assert!(heartbeat_needed(None, FanMode::Max, recent, false));
        assert!(!heartbeat_needed(
            Some(FanMode::Max),
            FanMode::Max,
            recent,
            false
        ));
        assert!(heartbeat_needed(
            Some(FanMode::Max),
            FanMode::Max,
            Some(HEARTBEAT),
            false
        ));
        assert!(heartbeat_needed(
            Some(FanMode::Curve),
            FanMode::Curve,
            recent,
            true
        ));
        assert!(heartbeat_needed(
            Some(FanMode::Max),
            FanMode::Max,
            None,
            false
        ));
    }
}
