//! Background fan control that replaces the OMEN Hub's software fan loop.
//! It runs elevated from the user's logon task, follows the saved shared
//! curve and hands control back to the BIOS whenever it stops sending.
//! The same service pins the foreground program to the saved CCD selection.
use super::{
    CpuHardwareReader, Nvidia,
    hp::{HpBios, Read},
};
use omen_core::{
    Inventory,
    cooling::FirmwareIrTemperature,
    fan::{self, FanCurve, Smoother},
    profiles::FanPoint,
};
use serde::Serialize;
use std::time::{Duration, Instant};
use windows_sys::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
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
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut ok = Process32FirstW(snapshot, &mut entry) != 0;
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

/// Runs until the process ends. `max` is the dashboard quick switch (both
/// fans at maximum); otherwise `load` returns the active shared curve (None:
/// leave the BIOS in charge). `ccd` returns the saved CCD selection (None:
/// every CCD). `report` persists each status.
pub fn run(
    inventory: &Inventory,
    mut max: impl FnMut() -> bool,
    mut load: impl FnMut() -> Result<Option<Vec<FanPoint>>, String>,
    mut ccd: impl FnMut() -> Result<Option<u8>, String>,
    mut report: impl FnMut(&FanServiceStatus),
) -> Result<(), String> {
    if inventory.manufacturer != "HP" || inventory.board != "8E35" || inventory.bios != "F.13" {
        return Err("Fan control is qualified only for HP8E35/F.13".into());
    }
    let bios = HpBios::new(inventory).map_err(|e| e.to_string())?;
    let mut cpu = CpuHardwareReader::new(inventory).ok();
    let mut gpu = Nvidia::new();
    let mut smoother = (Smoother::default(), Smoother::default());
    let mut last_heartbeat: Option<Instant> = None;
    let mut protection = 0u8;
    let ccd_masks = super::ccd::ccd_masks();
    let mut pinning = Pinning {
        enforcer: omen_core::ccd::Enforcer::default(),
        processes: super::ccd::WindowsProcesses::default(),
    };
    loop {
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
        // Releasing control means sending nothing: the BIOS resumes its own
        // fan table once the heartbeat and level updates stop.
        if hub_running() {
            last_heartbeat = None;
            report(&idle(
                "waiting_for_hub",
                omen_core::message::msg("fan.status.hub_running", &[]),
            ));
            std::thread::sleep(Duration::from_secs(10));
            continue;
        }
        let max = max();
        let curve = match load().and_then(|p| p.map(|p| curve_of(&p)).transpose()) {
            Ok(curve) if max => curve,
            Ok(Some(curve)) => Some(curve),
            Ok(None) => {
                last_heartbeat = None;
                report(&idle(
                    "bios",
                    omen_core::message::msg("fan.status.no_curve", &[]),
                ));
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
            Err(_) if max => None,
            Err(e) => {
                last_heartbeat = None;
                report(&idle(
                    "bios",
                    omen_core::message::msg("fan.status.invalid_curve", &[&e]),
                ));
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };
        if last_heartbeat.is_none_or(|t| t.elapsed() >= HEARTBEAT) {
            protection = bios.heartbeat().map_err(|e| e.to_string())?;
            last_heartbeat = Some(Instant::now());
        }
        if cpu.is_none() {
            cpu = CpuHardwareReader::new(inventory).ok();
        }
        let cpu_c = cpu
            .as_mut()
            .and_then(|r| r.sample().ok())
            .and_then(|s| s.temperature_c);
        if gpu.needs_reconnect() {
            gpu = Nvidia::new();
        }
        let gpu_c = gpu.sample().and_then(|s| s.temperature_c);
        let ir_c = bios
            .get(Read::SurfaceSensor)
            .ok()
            .and_then(|(code, bytes)| FirmwareIrTemperature::decode(code, &bytes).ok())
            .map(|t| t.celsius);
        let full = fan::emergency(
            cpu_c.map(|t| t as f32),
            gpu_c.map(|t| t as f32),
            ir_c,
            protection,
        );
        let target = |t: Option<i32>| match &curve {
            Some(curve) if !full => fan::level(curve.target(t)),
            _ => fan::MAX_LEVEL,
        };
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
        std::thread::sleep(CYCLE.saturating_sub(started.elapsed()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
