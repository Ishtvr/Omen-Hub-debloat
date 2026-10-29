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
mod power_events;
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
mod single_instance;
#[cfg(windows)]
mod startup_setup;
#[cfg(windows)]
mod switch;
#[cfg(windows)]
mod system_validation;
#[cfg(windows)]
mod telemetry_worker;
#[cfg(windows)]
mod theme;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod ui;
#[cfg(test)]
#[path = "../build_support/win_resource.rs"]
mod win_resource;

fn main() {
    if let Err(error) = run() {
        #[cfg(windows)]
        let show_error_dialog = {
            let args: Vec<_> = std::env::args().skip(1).collect();
            let args = if args.first().is_some_and(|arg| arg == "--lang")
                && args.get(1).is_some_and(|code| code == "de" || code == "en")
            {
                &args[2..]
            } else {
                &args[..]
            };
            args.is_empty() || args == ["--tray"]
        };
        #[cfg(windows)]
        if show_error_dialog {
            let body: Vec<u16> = i18n::error_message(&error)
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
        #[cfg(windows)]
        eprintln!("{}", i18n::message(&error));
        #[cfg(not(windows))]
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
    #[cfg(windows)]
    let args = {
        let mut args = args;
        i18n::configure(&mut args)?;
        args
    };
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
        let tray = !args.is_empty();
        // A second launch only brings the running dashboard forward; a
        // second sign-in start stays silent.
        let existing = || {
            if !tray {
                single_instance::activate(ui::CLASS, true);
            }
            Ok(())
        };
        if !single_instance::elevated() {
            if single_instance::running(ui::KIND) {
                return existing();
            }
            // Hardware actions need administrator rights: restart with them.
            // Declining the prompt keeps the dashboard with prompts per action.
            let arguments = format!("--lang {} {}", i18n::language_code(), args.join(" "));
            if single_instance::relaunch_elevated(arguments.trim()) {
                return Ok(());
            }
        }
        let Some(_instance) = single_instance::claim(ui::KIND) else {
            return existing();
        };
        return ui::run(tray);
    }
    #[cfg(windows)]
    if args
        .first()
        .is_some_and(|a| matches!(a.as_str(), "gpu-ui" | "presets-ui"))
        && args.len() <= 2
    {
        let Some(_instance) = single_instance::claim("Presets") else {
            single_instance::activate(preset_ui::CLASS, false);
            return Ok(());
        };
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
        let Some(_instance) = single_instance::claim("Keyboard") else {
            single_instance::activate(keyboard_ui::CLASS, false);
            return Ok(());
        };
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
    {
        use windows_sys::Win32::{Storage::FileSystem::*, System::Console::*};
        // AttachConsole replaces redirected standard handles. Keep an existing
        // pipe/file intact so diagnostics and stop acknowledgements terminate
        // correctly when launched by PowerShell or our own bounded readers.
        let output = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        let kind = unsafe { GetFileType(output) };
        if kind != FILE_TYPE_PIPE && kind != FILE_TYPE_DISK {
            // SAFETY: No console is created; only the caller's console is used.
            unsafe {
                AttachConsole(u32::MAX);
            }
        }
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
        Some("hp-control-guardian") if args.len() == 1 => {
            omen_platform::hp_control::run_guardian_process()?;
        }
        #[cfg(windows)]
        Some("hp-control-status") if args.len() == 1 => {
            println!(
                "{}",
                serde_json::to_string_pretty(&omen_platform::hp_control::inspect(
                    &inventory::read()
                ))
                .map_err(|e| e.to_string())?
            );
        }
        #[cfg(windows)]
        Some("hp-control-qualify") if args == ["hp-control-qualify", "--confirm-service-test"] => {
            let report = omen_platform::hp_control::qualify(&inventory::read())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            report.observation?;
            report.recovery?;
        }
        #[cfg(windows)]
        Some("hp-control-recover") if args.len() == 1 => {
            println!(
                "{}",
                serde_json::to_string_pretty(&omen_platform::hp_control::recover_controller()?)
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
        Some("--help" | "-h") => {
            #[cfg(windows)]
            println!("{}", i18n::tf("cli.help", &[&env!("CARGO_PKG_VERSION")]));
            #[cfg(not(windows))]
            println!(
                "Omen Hub++ {}: native UI and hardware controls require Windows x64.",
                env!("CARGO_PKG_VERSION")
            );
        }
        _ => {
            #[cfg(windows)]
            return Err(omen_core::message::msg("cli.unknown", &[]));
            #[cfg(not(windows))]
            return Err("Use --help for supported commands".into());
        }
    }
    Ok(())
}
