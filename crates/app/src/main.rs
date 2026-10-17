#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod autostart;
#[cfg(windows)]
mod cpu_sensor_bridge;
#[cfg(windows)]
mod curve_ui;
#[cfg(windows)]
mod curve_widget;
#[cfg(windows)]
mod dashboard;
#[cfg(windows)]
mod elevation;
#[cfg(windows)]
mod i18n;
#[cfg(windows)]
mod inventory;
#[cfg(windows)]
mod keyboard_ui;
#[cfg(windows)]
mod native_ui;
#[cfg(windows)]
mod preset_fields;
#[cfg(windows)]
mod preset_ui;
#[cfg(windows)]
mod profile_commands;
#[cfg(windows)]
mod read_process;
#[cfg_attr(not(windows), allow(dead_code))]
mod segments;
#[cfg(windows)]
mod startup_setup;
#[cfg(windows)]
mod switch;
#[cfg(windows)]
mod telemetry_worker;
#[cfg(windows)]
mod theme;
#[cfg(windows)]
mod ui;
#[cfg(test)]
#[path = "../build_support/win_resource.rs"]
mod win_resource;

fn main() {
    if let Err(error) = run() {
        #[cfg(windows)]
        if std::env::args().count() == 1 || std::env::args().skip(1).eq(["--tray"]) {
            let body: Vec<u16> = i18n::message(&error)
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let title: Vec<u16> = omen_core::APP_NAME.encode_utf16().chain(Some(0)).collect();
            // SAFETY: Strings are terminated and remain alive for the modal call.
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    std::ptr::null_mut(),
                    body.as_ptr(),
                    title.as_ptr(),
                    windows_sys::Win32::UI::WindowsAndMessaging::MB_OK
                        | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
                );
            }
        }
        eprintln!("{error}");
        std::process::exit(1);
    }
}
#[cfg(windows)]
fn preview_size(args: &[String], offset: usize) -> Result<Option<[i32; 2]>, String> {
    if args.len() == offset + 2 {
        Ok(Some([
            args[offset].parse::<i32>().map_err(|e| e.to_string())?,
            args[offset + 1].parse::<i32>().map_err(|e| e.to_string())?,
        ]))
    } else {
        Ok(None)
    }
}
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // GDI scaling renders text and shapes at the monitor resolution while the
    // fixed-pixel layouts stay in 96-DPI units. Without it, Windows stretches
    // the bitmap and text looks blurred at 125 % and above. Windowless control
    // commands are DPI aware so display APIs report physical modes.
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::HiDpi::*;
        let windowless = args
            .first()
            .is_some_and(|a| profile_commands::handles(a) || a.starts_with("mode-"));
        // SAFETY: Called once before any window exists; failure keeps the default.
        unsafe {
            SetProcessDpiAwarenessContext(if windowless {
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
            } else {
                DPI_AWARENESS_CONTEXT_UNAWARE_GDISCALED
            });
        }
    }
    #[cfg(windows)]
    if args.is_empty() || args == ["--tray"] {
        return ui::run(!args.is_empty());
    }
    #[cfg(windows)]
    if args
        .first()
        .is_some_and(|a| matches!(a.as_str(), "gpu-ui" | "presets-ui"))
        && args.len() <= 2
    {
        return preset_ui::run(args.get(1).cloned());
    }
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "curve-ui") && args.len() <= 2 {
        return curve_ui::run(args.get(1).cloned());
    }
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "fan-ui") && args.len() <= 2 {
        return curve_ui::run_fans(args.get(1).cloned());
    }
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "gpu-curve-ui") && args.len() <= 2 {
        return curve_ui::run_gpu(args.get(1).cloned());
    }
    #[cfg(windows)]
    if args == ["keyboard-ui"] {
        return keyboard_ui::run(None);
    }
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "gpu-preview") && args.len() == 2 {
        return preset_ui::preview(std::path::Path::new(&args[1]), None, None);
    }
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "preset-preview") && [2, 3, 5].contains(&args.len()) {
        return preset_ui::preview(
            std::path::Path::new(&args[1]),
            args.get(2)
                .map(|v| v.parse::<usize>().map_err(|e| e.to_string()))
                .transpose()?,
            preview_size(&args, 3)?,
        );
    }
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "dashboard-preview") && args.len() == 2 {
        return ui::preview(std::path::Path::new(&args[1]));
    }
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "dashboard-preview") && args.len() == 4 {
        return ui::preview_sized(
            std::path::Path::new(&args[1]),
            [
                args[2]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?,
                args[3]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?,
            ],
        );
    }
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "keyboard-preview") && [3, 5].contains(&args.len()) {
        return keyboard_ui::preview(
            args[1].clone(),
            std::path::Path::new(&args[2]),
            preview_size(&args, 3)?,
        );
    }
    #[cfg(windows)]
    if args.first().is_some_and(|a| a == "curve-preview") && args.len() == 3 {
        return curve_ui::preview(&args[1], std::path::Path::new(&args[2]));
    }
    #[cfg(windows)]
    // Attach to an existing terminal for explicit CLI use; GUI launches create no console.
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(u32::MAX);
    }
    #[cfg(windows)]
    if args
        .first()
        .is_some_and(|command| profile_commands::handles(command))
    {
        return profile_commands::run(&args);
    }
    match args.first().map(String::as_str) {
        #[cfg(windows)]
        Some("startup-setup") if args.len() == 1 => startup_setup::ensure_current_installation()?,
        #[cfg(windows)]
        Some("cpu-sensor-status") if args.len() == 1 => {
            let record = omen_platform::profile_store::ProfileStore::user_data()?.cpu_error()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({"last_cpu_error":record}))
                    .map_err(|e| e.to_string())?
            );
        }
        #[cfg(windows)]
        Some("cpu-sensor-launch") if args.len() == 3 => {
            let parent = args[2].parse::<u32>().map_err(|e| e.to_string())?;
            return cpu_sensor_bridge::launch(&args[1], parent);
        }
        #[cfg(windows)]
        Some("cpu-sensor-child") if args.len() == 3 => {
            let parent = args[2].parse::<u32>().map_err(|e| e.to_string())?;
            return cpu_sensor_bridge::child(&args[1], parent);
        }
        Some("diagnose") if args.len() == 1 => {
            #[cfg(windows)]
            let mut cpu = omen_platform::Cpu::new();
            let inventory = omen_platform::inventory();
            #[cfg(windows)]
            let mut nvidia = omen_platform::Nvidia::new();
            let mut snapshot = omen_core::Snapshot {
                inventory,
                power: omen_platform::power_status(),
                ..Default::default()
            };
            #[cfg(windows)]
            {
                // A meaningful CPU busy-time reading requires two samples.
                // Inventory/NVIDIA startup may already have supplied this interval.
                std::thread::sleep(std::time::Duration::from_millis(250));
                snapshot.cpu = Some(cpu.sample());
                snapshot.gpu = nvidia.sample();
                snapshot.warnings = nvidia.warnings().to_vec();
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?
            );
        }
        Some("inventory") if args.len() == 1 => {
            println!(
                "{}",
                serde_json::to_string(&omen_platform::inventory()).map_err(|e| e.to_string())?
            );
        }
        #[cfg(windows)]
        Some("gpu-telemetry-read") if args.len() == 1 => {
            println!(
                "{}",
                serde_json::to_string(&telemetry_worker::read_in_child())
                    .map_err(|e| e.to_string())?
            );
        }
        #[cfg(windows)]
        Some("hp-read") if args.len() == 1 => {
            let inventory = omen_platform::inventory();
            let status = omen_platform::read_hp_gpu_status(&inventory)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&status).map_err(|e| e.to_string())?
            );
        }
        #[cfg(windows)]
        Some("fan-read") if args.len() == 1 => {
            let report = omen_platform::read_hp_fans(&omen_platform::inventory())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        #[cfg(windows)]
        Some("cooling-read") if args.len() == 1 => {
            let report = omen_platform::read_cooling_snapshot(&omen_platform::inventory())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            if !report.temperature_batch_valid {
                return Err("Cooling diagnostic contains missing, stale or inconsistent sensor data; see sensor_faults".into());
            }
        }
        #[cfg(windows)]
        Some("hp-performance-read") if args.len() == 1 => {
            let inventory = omen_platform::inventory();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "gpu_status": omen_platform::read_hp_gpu_status(&inventory),
                    "system_design": omen_platform::read_hp_system_design(&inventory),
                    "hardware_applied": false
                }))
                .map_err(|e| e.to_string())?
            );
        }
        #[cfg(windows)]
        Some("cpu-hardware-read") if args.len() == 1 => {
            let inventory = omen_platform::inventory();
            let mut reader = omen_platform::CpuHardwareReader::new(&inventory)?;
            let baseline = reader.sample()?;
            std::thread::sleep(std::time::Duration::from_millis(1100));
            let measured = reader.sample()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "board":inventory.board,"bios":inventory.bios,
                    "driver_sha256":omen_platform::CPU_DRIVER_SHA256,
                    "module_sha256":omen_platform::CPU_MODULE_SHA256,
                    "baseline":baseline,"measured":measured,
                }))
                .map_err(|e| e.to_string())?
            );
        }
        #[cfg(windows)]
        Some("keyboard-read") if args.len() == 1 => {
            let inventory = omen_platform::inventory();
            let state = omen_platform::read_hp_keyboard_state(&inventory)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?
            );
        }
        Some("plan") if args.len() == 2 => {
            let profile = omen_core::Profile::parse(&args[1])?;
            let plan = omen_core::preview_profile(&omen_platform::inventory(), profile);
            println!(
                "{}",
                serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?
            );
        }
        Some("--help" | "-h") => println!(
            "Omen Hub++ 0.1.0\n\n  omen-hub-pp.exe           Native dashboard\n  omen-hub-pp.exe --tray    Start hidden in tray\n  omen-hub-pp.exe presets-ui [id] Native custom preset editor (gpu-ui alias)\n  omen-hub-pp.exe fan-ui [id] Native CPU/GPU fan curve editor\n  omen-hub-pp.exe gpu-curve-ui [id] Native GPU V/F curve editor\n  omen-hub-pp.exe curve-ui [id] Combined curve workbench (legacy entry)\n  omen-hub-pp.exe keyboard-ui Native direct four-zone lighting and on/off (UAC)\n  omen-hub-pp.exe keyboard-preview <id> <bmp-path> Render our native lighting controls\n  omen-hub-pp.exe curve-preview gpu|cpu-fan|gpu-fan <bmp-path> Render only our widget\n  omen-hub-pp.exe diagnose  Read-only JSON diagnostics\n  omen-hub-pp.exe inventory Read-only system inventory\n  omen-hub-pp.exe cpu-hardware-read  Fixed measured CPU energy/Tctl GETs; administrator required\n  omen-hub-pp.exe cpu-sensor-status  Last CPU error with technical detail\n  omen-hub-pp.exe dashboard-preview <bmp-path> Render our own native dashboard client\n  omen-hub-pp.exe cooling-read  One-shot CPU/GPU/IR/fan diagnostics; administrator required\n  omen-hub-pp.exe fan-read  Fixed HP fan/Max-Fan/table GETs; administrator required\n  omen-hub-pp.exe fan-validate-max  Fixed maximum cooling ON/hold/original restore trial\n  omen-hub-pp.exe hp-read   HP GPU-status GET (8E35/F.13 only)\n  omen-hub-pp.exe hp-performance-read  HP GPU status and system design GETs\n  omen-hub-pp.exe keyboard-read  Read-only keyboard state (8E35/F.13 only)\n  omen-hub-pp.exe keyboard-validate-colors Fixed one-byte RGB trial with complete restore\n  omen-hub-pp.exe keyboard-validate-lighting Fixed RGB/static-preset trial with complete restore\n  omen-hub-pp.exe gpu-capabilities [uuid] Read GPU offset limits and feature support\n  omen-hub-pp.exe gpu-curve-read  Read the complete Graphics V/F domain\n  omen-hub-pp.exe gpu-validate-memory-curve Fixed memory-after-curve preservation trial; full restore\n  omen-hub-pp.exe gpu-validate-curve Fixed -15 MHz Graphics frequency trial; full restore\n  omen-hub-pp.exe gpu-validate Two fixed +/-15 MHz GPU and +/-24 MHz VRAM trials; restore after each\n  omen-hub-pp.exe profile-catalog|profile-list\n  omen-hub-pp.exe profile-create <id> <name>\n  omen-hub-pp.exe profile-show|profile-plan|profile-apply <id>\n  omen-hub-pp.exe profile-restore  Recover original state from the latest journal\n  omen-hub-pp.exe profile-set <id> <feature> <value>\n  omen-hub-pp.exe profile-set-vf <id> <anchor-mV> <target-MHz> Save a full plateau curve; no write\n  omen-hub-pp.exe profile-storage|profile-migrate-legacy\n  omen-hub-pp.exe profile-import <json-file>\n  omen-hub-pp.exe startup-setup Install/update the fixed protected logon task (UAC)\n  omen-hub-pp.exe profile-startup-enable <id>\n  omen-hub-pp.exe profile-startup-disable|profile-startup-apply|profile-startup-status\n  omen-hub-pp.exe plan quiet|balanced|performance|unleashed\n\nOnly validated adapters may write hardware. Built-in HP profiles remain previews."
        ),
        _ => return Err("Use --help for supported commands".into()),
    }
    Ok(())
}
