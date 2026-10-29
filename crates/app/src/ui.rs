//! Small native Win32 shell. Hardware queries stay on one worker thread.
use crate::i18n::{t, tf};
use omen_core::{
    APP_NAME, CpuHardwareStatus, Snapshot,
    hp_mode::HpMode,
    power::Sampling,
    quick_switch::{FanMode, GraphicsMode},
};
use std::{
    cell::{Cell, RefCell},
    ptr,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::{BST_CHECKED, BST_UNCHECKED},
        Shell::*,
        WindowsAndMessaging::*,
    },
};

const UPDATE: u32 = WM_APP + 1;
const TRAY: u32 = crate::tray::CALLBACK;
const CPU_READER_PID: u32 = WM_APP + 61;
const POWER_STATE: u32 = WM_APP + 63;
const MODE_RESULT: u32 = WM_APP + 62;
const REFRESH: usize = 101;
const PROFILE: usize = 102;
const GPU_PROFILES: usize = 103;
const CURVES: usize = 104;
const KEYBOARD: usize = 105;
/// Fan quick switch on the Controls line: saved curve or both fans at maximum.
const FAN_SWITCH: usize = 106;
const MUX_SWITCH: usize = 107;
const MUX_NOTE: i32 = 153;
const APPLY: usize = 108;
const AUTOSTART: usize = 190;
const CLOSE_TO_TRAY: usize = 191;
/// Round "i" next to the performance-mode label.
const MODE_INFO: usize = 182;
const SHOW: u32 = 201;
const EXIT: u32 = 202;
/// Built-in HP modes (language keys) ahead of saved presets.
const DEFAULT_MODES: [(&str, HpMode); 4] = [
    ("mode.quiet", HpMode::Quiet),
    ("mode.balanced", HpMode::Balanced),
    ("mode.performance", HpMode::Performance),
    ("mode.unleashed", HpMode::Unleashed),
];

#[derive(Debug, Clone)]
enum Request {
    Refresh,
    Visibility(bool),
    Power,
    ApplyPreset(String),
    ApplyMode(HpMode),
    FanMode(FanMode),
    Graphics(GraphicsMode),
    Stop,
}

struct State {
    tx: mpsc::Sender<Request>,
    snapshot: Arc<Mutex<Option<Snapshot>>>,
    power: Arc<Mutex<Sampling>>,
    details: Cell<HWND>,
    profile: Cell<HWND>,
    preview: Cell<HWND>,
    tray: crate::tray::TrayIcon,
    taskbar_message: u32,
    theme: crate::theme::Theme,
    preview_index: Cell<usize>,
    custom_profiles: RefCell<Vec<(String, String)>>,
    selected_custom: RefCell<Option<String>>,
    mode_result: Arc<Mutex<Option<String>>>,
    applying_preset: Cell<bool>,
    mode_status: RefCell<String>,
    close_to_tray: Cell<bool>,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub(crate) fn cpu_error_message(detail: &str) -> &'static str {
    // Keep old saved diagnostics readable; new launches use a stable code.
    let approval_failed = detail.starts_with(crate::cpu_sensor_bridge::APPROVAL_FAILED)
        || detail.starts_with("CPU-Sensorfreigabe nicht erfolgt:");
    if detail.starts_with("This build has no staged signed CPU module") {
        t("cpu_error.no_module")
    } else if detail.contains("qualified only")
        || detail.contains("family/model/stepping/package")
        || detail.contains("requires Windows x64")
    {
        t("cpu_error.unsupported")
    } else if detail.starts_with("PawnIO must already be installed and running") {
        t("cpu_error.pawnio_missing")
    } else if detail.contains("hash differs")
        || detail.contains("version/response differs")
        || detail.contains("Embedded CPU module differs")
        || detail.contains("Signed CPU module load")
        || detail.contains("driver path")
    {
        t("cpu_error.driver_version")
    } else if approval_failed && detail.contains("1223") {
        t("cpu_error.cancelled")
    } else if approval_failed || detail.starts_with("CPU driver access requires") {
        t("cpu_error.denied")
    } else if detail.contains("pipe peer")
        || detail.contains("server PID")
        || detail.contains("stale or unrelated")
    {
        t("cpu_error.peer")
    } else if detail.contains("failed to connect")
        || detail.contains("disconnected")
        || detail.contains("timed out")
        || detail.contains("request pipe closed")
    {
        t("cpu_error.disconnected")
    } else {
        t("cpu_error.interrupted")
    }
}
fn cpu_failed(snapshot: &mut Snapshot, detail: String) {
    let message = cpu_error_message(&detail);
    if let Ok(store) = omen_platform::profile_store::ProfileStore::user_data() {
        let _=store.save_cpu_error(&serde_json::json!({"schema":1,
            "recorded_at_unix_seconds":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t|t.as_secs()).ok(),
            "user_message":message,"detail":detail}));
    }
    snapshot.cpu_hardware_status = CpuHardwareStatus::Error(detail);
}

fn worker(
    hwnd: usize,
    rx: mpsc::Receiver<Request>,
    shared: Arc<Mutex<Option<Snapshot>>>,
    initially_visible: bool,
    mode_result: Arc<Mutex<Option<String>>>,
    mut start_cpu_reader: bool,
    power: Arc<Mutex<Sampling>>,
) {
    let mut snapshot = Snapshot {
        inventory: crate::inventory::read(),
        power: omen_platform::power_status(),
        ..Default::default()
    };
    let mut visible = initially_visible;
    let mut nvidia = None;
    let mut cpu = None;
    let mut hardware: Option<crate::cpu_sensor_bridge::CpuSensorBridge> = None;
    let mut reopen_at = Instant::now();
    let publish = |snapshot: &Snapshot| {
        *shared.lock().unwrap_or_else(|e| e.into_inner()) = Some(snapshot.clone());
        // SAFETY: PostMessage does not dereference hwnd; a closed window rejects the message.
        unsafe {
            PostMessageW(hwnd as HWND, UPDATE, 0, 0);
        }
    };
    loop {
        let power_at_start = *power.lock().unwrap_or_else(|e| e.into_inner());
        let mut suspended = !power_at_start.active();
        if visible && !suspended {
            snapshot.cpu = Some(cpu.get_or_insert_with(omen_platform::Cpu::new).sample());
            if let Some(reader) = hardware.as_mut() {
                let result = reader.sample();
                if *power.lock().unwrap_or_else(|e| e.into_inner()) != power_at_start {
                    // A window notification is visible immediately, even when
                    // the worker was inside a read as Windows suspended it.
                    // Drain that exact outstanding response and reset both
                    // baselines; never publish an interval across the transition.
                    cpu = None;
                    snapshot.cpu = None;
                    match reader.pause() {
                        Ok(()) => snapshot.cpu_hardware_status = CpuHardwareStatus::Paused,
                        Err(error) => {
                            hardware = None;
                            cpu_failed(&mut snapshot, error);
                        }
                    }
                } else {
                    match result {
                        Ok(readout) => {
                            if let Some(cpu) = snapshot.cpu.as_mut() {
                                cpu.package_power_w = readout.package_power_w;
                                cpu.temperature_c = readout.temperature_c;
                                cpu.warnings.extend(readout.warnings.clone());
                                cpu.hardware = Some(readout);
                            }
                            snapshot.cpu_hardware_status = CpuHardwareStatus::Active;
                        }
                        Err(error) => {
                            hardware = None;
                            cpu_failed(&mut snapshot, error);
                        }
                    }
                }
            }
        } else {
            cpu = None;
            snapshot.cpu = None;
        }
        suspended = !power.lock().unwrap_or_else(|e| e.into_inner()).active();
        if visible && !suspended && Instant::now() >= reopen_at {
            let nvidia = nvidia.get_or_insert_with(crate::telemetry_worker::GpuTelemetry::new);
            snapshot.gpu = nvidia.sample();
            snapshot.warnings = nvidia.warnings().to_vec();
            if nvidia.needs_reconnect() {
                // Do not reuse lost handles. A 30-second cooldown avoids a retry loop.
                reopen_at = Instant::now() + Duration::from_secs(30);
                snapshot.gpu = None;
                snapshot.warnings.push(t("warning.nvidia_lost").into());
            }
        } else if visible && !suspended {
            snapshot.gpu = None;
        } else {
            // Release driver sessions and stop sensor polling while hidden/asleep.
            nvidia = None;
            snapshot.gpu = None;
        }
        if nvidia.as_ref().is_some_and(|api| api.needs_reconnect()) {
            nvidia = None;
        }
        let current_power = *power.lock().unwrap_or_else(|e| e.into_inner());
        suspended = !current_power.active();
        if current_power != power_at_start || suspended {
            snapshot.cpu = None;
            snapshot.gpu = None;
            cpu = None;
            nvidia = None;
        }
        snapshot.power = omen_platform::power_status();
        snapshot.cpu_reader_pid = hardware.as_ref().map(|r| r.process_id());
        publish(&snapshot);
        // CPU values need no separate switch: the first visible dashboard
        // requests the session helper once. A cancelled or failed request is
        // retried only through Aktualisieren, never in a prompt loop.
        if start_cpu_reader && hardware.is_none() && visible && !suspended {
            start_cpu_reader = false;
            snapshot.cpu_hardware_status = CpuHardwareStatus::Starting;
            publish(&snapshot);
            match crate::cpu_sensor_bridge::CpuSensorBridge::start() {
                Ok(reader) => {
                    hardware = Some(reader);
                    cpu = None;
                    continue;
                }
                Err(e) => {
                    cpu_failed(&mut snapshot, e);
                    publish(&snapshot);
                }
            }
        }
        let request = if visible && !suspended {
            match rx.recv_timeout(Duration::from_secs(3)) {
                Ok(request) => Some(request),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(request) => Some(request),
                Err(_) => break,
            }
        };
        let reset_hardware = matches!(&request, Some(Request::Refresh | Request::Power));
        match request {
            Some(Request::Stop) => break,
            Some(Request::ApplyPreset(id)) => {
                let result =
                    crate::elevation::run(Some(&id), crate::elevation::Action::Apply, || false);
                *mode_result.lock().unwrap_or_else(|e| e.into_inner()) = Some(if result.is_ok() {
                    t("preset.applied").into()
                } else {
                    t("mode.preset_failed").into()
                });
                unsafe {
                    PostMessageW(hwnd as HWND, MODE_RESULT, 0, 0);
                }
            }
            Some(Request::FanMode(mode)) => {
                // The switch is a user file the elevated fan service reads; only
                // installing or updating the service needs Windows approval. An
                // outdated service is updated even for Auto, because older builds
                // read every value except "max" as "follow the curve".
                let result =
                    omen_platform::profile_store::ProfileStore::user_data().and_then(|store| {
                        let previous = store.fan_mode();
                        store.save_fan_mode(mode)?;
                        if (mode.needs_service() || omen_platform::fan_service_present())
                            && !omen_platform::fan_service_installed()
                        {
                            if let Err(error) = crate::elevation::run(
                                None,
                                crate::elevation::Action::FanService,
                                || false,
                            ) {
                                store.save_fan_mode(previous)?;
                                return Err(error);
                            }
                        }
                        Ok(store.active_fan_curve().ok().flatten().is_some())
                    });
                *mode_result.lock().unwrap_or_else(|e| e.into_inner()) = Some(match result {
                    Ok(_) if mode == FanMode::Bios => t("fan.quick.bios_active").into(),
                    Ok(_) if mode == FanMode::Max => t("fan.quick.max_active").into(),
                    Ok(true) => t("fan.quick.curve_active").into(),
                    Ok(false) => t("fan.quick.curve_missing").into(),
                    Err(error) => tf("fan.quick.failed", &[&crate::i18n::error_message(&error)]),
                });
                unsafe {
                    PostMessageW(hwnd as HWND, MODE_RESULT, 0, 0);
                }
            }
            Some(Request::Graphics(mode)) => {
                let result =
                    crate::elevation::run(None, crate::elevation::Action::Graphics(mode), || false);
                let name = graphics_name(mode);
                *mode_result.lock().unwrap_or_else(|e| e.into_inner()) = Some(match result {
                    Ok(_) if restart_pending() => tf("mux.pending", &[&name]),
                    Ok(_) => tf("mux.active", &[&name]),
                    Err(error) => tf("mux.failed", &[&crate::i18n::error_message(&error)]),
                });
                unsafe {
                    PostMessageW(hwnd as HWND, MODE_RESULT, 0, 0);
                }
            }
            Some(Request::ApplyMode(mode)) => {
                let result =
                    crate::elevation::run(None, crate::elevation::Action::Mode(mode), || false);
                let label = DEFAULT_MODES
                    .iter()
                    .find(|(_, m)| *m == mode)
                    .map_or("", |(key, _)| t(key));
                *mode_result.lock().unwrap_or_else(|e| e.into_inner()) = Some(match result {
                    Ok(_) => tf("mode.active", &[&label]),
                    Err(error) => tf(
                        "mode.failed",
                        &[&label, &crate::i18n::error_message(&error)],
                    ),
                });
                unsafe {
                    PostMessageW(hwnd as HWND, MODE_RESULT, 0, 0);
                }
            }
            Some(Request::Visibility(value)) => {
                visible = value;
                if value {
                    reopen_at = Instant::now();
                }
            }
            Some(Request::Refresh) => {
                cpu = None;
                start_cpu_reader = hardware.is_none();
                snapshot.inventory = crate::inventory::read();
                nvidia = None;
                reopen_at = Instant::now();
            }
            Some(Request::Power) => {
                cpu = None;
                nvidia = None;
                reopen_at = Instant::now();
            }
            None => (),
        }
        if let Some(reader) = hardware.as_mut() {
            if !visible || suspended || reset_hardware {
                match reader.pause() {
                    Ok(()) => snapshot.cpu_hardware_status = CpuHardwareStatus::Paused,
                    Err(e) => {
                        hardware = None;
                        cpu_failed(&mut snapshot, e);
                    }
                }
            }
        }
    }
}

fn graphics_name(mode: GraphicsMode) -> &'static str {
    match mode {
        GraphicsMode::Hybrid => t("mux.hybrid"),
        GraphicsMode::Discrete => t("mux.discrete"),
    }
}
/// Whether the last graphics mode chosen in this boot still waits for a restart.
fn restart_pending() -> bool {
    omen_platform::profile_store::ProfileStore::user_data()
        .ok()
        .and_then(|store| store.graphics_mode())
        .is_some_and(|record| record.restart_pending(omen_platform::boot_unix_s()))
}
/// Shows the saved fan and graphics choices; without a record the graphics
/// switch shows the default Hybrid.
unsafe fn sync_switches(hwnd: HWND) {
    unsafe {
        let store = omen_platform::profile_store::ProfileStore::user_data().ok();
        let fan = store.as_ref().map(|s| s.fan_mode()).unwrap_or_default();
        let graphics = store.as_ref().and_then(|s| s.graphics_mode());
        crate::switch::select(GetDlgItem(hwnd, FAN_SWITCH as i32), fan.index());
        crate::switch::select(
            GetDlgItem(hwnd, MUX_SWITCH as i32),
            graphics.map(|r| r.mode).unwrap_or_default().index(),
        );
        let pending = graphics.is_some_and(|r| r.restart_pending(omen_platform::boot_unix_s()));
        for (id, checked) in [
            (AUTOSTART, crate::autostart::enabled()),
            (
                CLOSE_TO_TRAY,
                store.as_ref().is_none_or(|s| s.close_to_tray()),
            ),
        ] {
            SendMessageW(
                GetDlgItem(hwnd, id as i32),
                BM_SETCHECK,
                if checked { BST_CHECKED } else { BST_UNCHECKED } as usize,
                0,
            );
        }
        SetWindowTextW(
            GetDlgItem(hwnd, MUX_NOTE),
            wide(if pending { t("mux.restart_note") } else { "" }).as_ptr(),
        );
    }
}
unsafe fn reload_modes(hwnd: HWND, state: &State) {
    unsafe {
        if state.profile.get().is_null() {
            return;
        }
        let result = omen_platform::profile_store::ProfileStore::user_data().and_then(|s| s.list());
        if let Ok(profiles) = result {
            let entries = profiles
                .into_iter()
                .map(|p| (p.id, p.name))
                .collect::<Vec<_>>();
            SendMessageW(state.profile.get(), CB_RESETCONTENT, 0, 0);
            for (label, _) in DEFAULT_MODES {
                SendMessageW(
                    state.profile.get(),
                    CB_ADDSTRING,
                    0,
                    wide(&tf("mode.default_entry", &[&t(label)])).as_ptr() as isize,
                );
            }
            let mut selected = state.preview_index.get().min(DEFAULT_MODES.len() - 1);
            for (i, (id, name)) in entries.iter().enumerate() {
                SendMessageW(
                    state.profile.get(),
                    CB_ADDSTRING,
                    0,
                    wide(name).as_ptr() as isize,
                );
                if state.selected_custom.borrow().as_ref() == Some(id) {
                    selected = i + DEFAULT_MODES.len();
                }
            }
            if selected < DEFAULT_MODES.len() {
                *state.selected_custom.borrow_mut() = None;
            }
            *state.custom_profiles.borrow_mut() = entries;
            SendMessageW(state.profile.get(), CB_SETCURSEL, selected, 0);
            SetWindowTextW(
                state.preview.get(),
                wide(&state.mode_status.borrow()).as_ptr(),
            );
            for control in [state.profile.get(), GetDlgItem(hwnd, APPLY as i32)] {
                windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                    control,
                    (!state.applying_preset.get()) as i32,
                );
            }
        } else {
            SetWindowTextW(
                GetDlgItem(hwnd, 181),
                wide(t("mode.presets_unreadable")).as_ptr(),
            );
        }
    }
}

// SAFETY: Called only on the UI thread for the live, owned State bound to hwnd.
unsafe fn redraw(state: &State) {
    if state.details.get().is_null() {
        return;
    }
    let snapshot = state
        .snapshot
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    unsafe {
        if let Some(snapshot) = snapshot {
            let hwnd = GetParent(state.details.get());
            crate::dashboard::update(hwnd, &state.theme, &snapshot);
        }
    }
}

// Retain the tray/event window, but release the native dashboard controls while
// hidden. Hardware state and approval belong to the worker, not these views.
unsafe fn show_dashboard(hwnd: HWND, state: &State) {
    unsafe {
        if state.details.get().is_null() {
            crate::dashboard::create(hwnd, &state.theme);
            state.details.set(GetDlgItem(hwnd, 180));
            state.profile.set(GetDlgItem(hwnd, PROFILE as i32));
            state.preview.set(GetDlgItem(hwnd, 181));
            reload_modes(hwnd, state);
            sync_switches(hwnd);
            redraw(state);
        }
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
        let _ = state.tx.send(Request::Visibility(true));
    }
}
unsafe fn hide_dashboard(hwnd: HWND, state: &State) {
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
        let _ = state.tx.send(Request::Visibility(false));
        let mut child = GetWindow(hwnd, GW_CHILD);
        while !child.is_null() {
            let next = GetWindow(child, GW_HWNDNEXT);
            DestroyWindow(child);
            child = next;
        }
        state.details.set(ptr::null_mut());
        state.profile.set(ptr::null_mut());
        state.preview.set(ptr::null_mut());
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Win32 owns message dispatch on one thread. State is boxed in run()
    // and lives until after WM_DESTROY, the message loop and worker shutdown.
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if state_ptr.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        // Shared references plus Cell allow reentrant child-window notifications
        // without aliasing a Rust &mut State across a Windows call.
        let state = &*state_ptr;
        if state.taskbar_message != 0 && message == state.taskbar_message {
            // MODIFY/ADD is idempotent even if Explorer broadcasts twice.
            if !state.tray.ensure(hwnd) && !state.tray.retrying() && IsWindowVisible(hwnd) == 0 {
                show_dashboard(hwnd, state);
            }
            return 0;
        }
        if let Some(color) = state.theme.color(message, wparam, lparam) {
            return color;
        }
        match message {
            WM_CREATE => {
                crate::dashboard::create(hwnd, &state.theme);
                state.details.set(GetDlgItem(hwnd, 180));
                state.profile.set(GetDlgItem(hwnd, PROFILE as i32));
                state.preview.set(GetDlgItem(hwnd, 181));
                reload_modes(hwnd, state);
                sync_switches(hwnd);
                if [
                    state.details.get(),
                    state.profile.get(),
                    state.preview.get(),
                ]
                .iter()
                .any(|h| h.is_null())
                {
                    return -1;
                }
                state.theme.frame(hwnd);
                // Explorer must be able to notify this window even if the app
                // was launched elevated. Allow only this registered message.
                if state.taskbar_message != 0 {
                    ChangeWindowMessageFilterEx(
                        hwnd,
                        state.taskbar_message,
                        MSGFLT_ALLOW,
                        ptr::null_mut(),
                    );
                }
                state.tray.ensure(hwnd);
                0
            }
            WM_TIMER if wparam == crate::tray::RETRY_TIMER => {
                if !state.tray.retry(hwnd) && IsWindowVisible(hwnd) == 0 {
                    // Rebuild controls too: hidden dashboards released them.
                    show_dashboard(hwnd, state);
                }
                0
            }
            WM_ERASEBKGND => {
                crate::dashboard::paint(hwnd, &state.theme, wparam as HDC);
                1
            }
            WM_PRINTCLIENT => {
                crate::dashboard::paint(hwnd, &state.theme, wparam as HDC);
                0
            }
            WM_PAINT => {
                let mut paint: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(hwnd, &mut paint);
                crate::dashboard::paint(hwnd, &state.theme, dc);
                EndPaint(hwnd, &paint);
                0
            }
            WM_DRAWITEM => {
                let item = &*(lparam as *const windows_sys::Win32::UI::Controls::DRAWITEMSTRUCT);
                if item.CtlID == MODE_INFO as u32 {
                    state.theme.info(item);
                    1
                } else if item.CtlType == windows_sys::Win32::UI::Controls::ODT_BUTTON {
                    state.theme.button(
                        item,
                        state.theme.body,
                        item.CtlID == GPU_PROFILES as u32 || item.CtlID == APPLY as u32,
                    );
                    1
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(lparam as *mut MINMAXINFO);
                info.ptMinTrackSize = POINT { x: 960, y: 750 };
                0
            }
            WM_COMMAND => {
                let id = wparam & 0xffff;
                // Owner-drawn buttons also send non-click notifications.
                // Focus/paint/double-click notifications cannot trigger actions.
                if id != PROFILE && (wparam >> 16) as u32 != BN_CLICKED {
                    return 0;
                }
                if id == MODE_INFO {
                    MessageBoxW(
                        hwnd,
                        wide(t("info.mode")).as_ptr(),
                        wide(t("dash.mode")).as_ptr(),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
                if id == REFRESH {
                    reload_modes(hwnd, state);
                    let _ = state.tx.send(Request::Refresh);
                }
                if id == FAN_SWITCH {
                    let index = crate::switch::selection(lparam as HWND);
                    if let Some(mode) = FanMode::from_index(index) {
                        let _ = state.tx.send(Request::FanMode(mode));
                    }
                }
                if id == MUX_SWITCH {
                    let index = crate::switch::selection(lparam as HWND);
                    if let Some(mode) = GraphicsMode::from_index(index) {
                        let _ = state.tx.send(Request::Graphics(mode));
                    }
                }
                if id == GPU_PROFILES || id == CURVES || id == KEYBOARD {
                    match std::env::current_exe().and_then(|path| {
                        std::process::Command::new(path)
                            .args(["--lang", crate::i18n::language_code()])
                            .arg(match id {
                                CURVES => "fan-ui",
                                KEYBOARD => "keyboard-ui",
                                _ => "presets-ui",
                            })
                            .spawn()
                    }) {
                        Ok(_) => {}
                        Err(error) => {
                            SetWindowTextW(
                                state.preview.get(),
                                wide(&tf("main.open_failed", &[&error])).as_ptr(),
                            );
                        }
                    }
                }
                if id == PROFILE && (wparam >> 16) as u32 == CBN_SELENDOK {
                    // Choosing only selects; "Anwenden" applies.
                    SetWindowTextW(state.preview.get(), wide(t("mode.apply_hint")).as_ptr());
                }
                if id == APPLY && !state.applying_preset.get() {
                    let index = SendMessageW(state.profile.get(), CB_GETCURSEL, 0, 0);
                    let begin = |request: Request| {
                        state.applying_preset.set(true);
                        for control in [state.profile.get(), GetDlgItem(hwnd, APPLY as i32)] {
                            windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                                control, 0,
                            );
                        }
                        SetWindowTextW(state.preview.get(), wide(t("common.uac")).as_ptr());
                        *state.mode_status.borrow_mut() = t("common.uac").into();
                        let _ = state.tx.send(request);
                    };
                    if index >= DEFAULT_MODES.len() as isize {
                        let selected = state
                            .custom_profiles
                            .borrow()
                            .get(index as usize - DEFAULT_MODES.len())
                            .cloned();
                        if let Some((id, _)) = selected {
                            *state.selected_custom.borrow_mut() = Some(id.clone());
                            begin(Request::ApplyPreset(id));
                        }
                    } else if index >= 0 {
                        state.preview_index.set(index as usize);
                        *state.selected_custom.borrow_mut() = None;
                        begin(Request::ApplyMode(DEFAULT_MODES[index as usize].1));
                    }
                }
                if id == AUTOSTART {
                    let check = GetDlgItem(hwnd, AUTOSTART as i32);
                    let enable = SendMessageW(check, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
                    let message = match crate::autostart::set(enable) {
                        Ok(()) if enable => t("settings.autostart_on").to_owned(),
                        Ok(()) => t("settings.autostart_off").to_owned(),
                        Err(error) => {
                            SendMessageW(
                                check,
                                BM_SETCHECK,
                                if enable { BST_UNCHECKED } else { BST_CHECKED } as usize,
                                0,
                            );
                            tf("settings.failed", &[&error])
                        }
                    };
                    SetWindowTextW(state.preview.get(), wide(&message).as_ptr());
                }
                if id == CLOSE_TO_TRAY {
                    let check = GetDlgItem(hwnd, CLOSE_TO_TRAY as i32);
                    let enable = SendMessageW(check, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
                    match omen_platform::profile_store::ProfileStore::user_data()
                        .and_then(|s| s.save_close_to_tray(enable))
                    {
                        Ok(()) => state.close_to_tray.set(enable),
                        Err(error) => {
                            SendMessageW(
                                check,
                                BM_SETCHECK,
                                if enable { BST_UNCHECKED } else { BST_CHECKED } as usize,
                                0,
                            );
                            SetWindowTextW(
                                state.preview.get(),
                                wide(&tf("settings.failed", &[&error])).as_ptr(),
                            );
                        }
                    }
                }
                if id == SHOW as usize {
                    show_dashboard(hwnd, state);
                }
                if id == EXIT as usize {
                    DestroyWindow(hwnd);
                }
                0
            }
            UPDATE => {
                redraw(state);
                0
            }
            MODE_RESULT => {
                state.applying_preset.set(false);
                if !state.profile.get().is_null() {
                    for control in [state.profile.get(), GetDlgItem(hwnd, APPLY as i32)] {
                        windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(control, 1);
                    }
                }
                if let Some(message) = state
                    .mode_result
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take()
                {
                    sync_switches(hwnd);
                    *state.mode_status.borrow_mut() = message.clone();
                    if !state.preview.get().is_null() {
                        SetWindowTextW(
                            state.preview.get(),
                            wide(&crate::i18n::message(&message)).as_ptr(),
                        );
                    }
                }
                0
            }
            WM_ACTIVATE => {
                if wparam & 0xffff != WA_INACTIVE as usize {
                    reload_modes(hwnd, state);
                }
                0
            }
            CPU_READER_PID => state
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .and_then(|s| s.cpu_reader_pid)
                .unwrap_or(0) as isize,
            POWER_STATE => {
                let power = *state.power.lock().unwrap_or_else(|e| e.into_inner());
                ((power.generation << 2)
                    | u64::from(power.suspended)
                    | (u64::from(!power.display_on) << 1)) as isize
            }
            WM_POWERBROADCAST => {
                if let Some(event) = crate::power_events::decode(wparam as u32, lparam) {
                    let changed = state
                        .power
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .observe(event);
                    if changed {
                        let _ = state.tx.send(Request::Power);
                    }
                }
                1
            }
            TRAY => {
                // Version 4 packs the icon ID in the high word; the legacy
                // mouse callback has the same low-word event values.
                match lparam as u32 & 0xFFFF {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_SELECT | crate::tray::KEY_SELECT => {
                        show_dashboard(hwnd, state);
                    }
                    WM_RBUTTONUP | WM_CONTEXTMENU => {
                        let menu = CreatePopupMenu();
                        AppendMenuW(
                            menu,
                            MF_STRING,
                            SHOW as usize,
                            wide(t("tray.open")).as_ptr(),
                        );
                        AppendMenuW(
                            menu,
                            MF_STRING,
                            EXIT as usize,
                            wide(t("tray.exit")).as_ptr(),
                        );
                        let mut point = POINT { x: 0, y: 0 };
                        GetCursorPos(&mut point);
                        SetForegroundWindow(hwnd);
                        TrackPopupMenu(
                            menu,
                            TPM_RIGHTBUTTON,
                            point.x,
                            point.y,
                            0,
                            hwnd,
                            ptr::null(),
                        );
                        DestroyMenu(menu);
                        PostMessageW(hwnd, WM_NULL, 0, 0);
                    }
                    _ => (),
                }
                0
            }
            WM_SIZE if wparam == SIZE_MINIMIZED as usize && state.close_to_tray.get() => {
                // A cached success cannot prove the shell still has the icon.
                if state.tray.ensure(hwnd) {
                    hide_dashboard(hwnd, state);
                } else {
                    show_dashboard(hwnd, state);
                }
                0
            }
            WM_SIZE => {
                crate::dashboard::layout(hwnd);
                0
            }
            WM_CLOSE => {
                if state.close_to_tray.get() {
                    if state.tray.ensure(hwnd) {
                        hide_dashboard(hwnd, state);
                    } else {
                        show_dashboard(hwnd, state);
                    }
                } else {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_DESTROY => {
                let _ = state.tx.send(Request::Stop);
                state.tray.remove(hwnd);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

pub fn run(start_in_tray: bool) -> Result<(), String> {
    run_window(start_in_tray, None, [1140, 780])
}
pub fn preview(path: &std::path::Path) -> Result<(), String> {
    run_window(false, Some(path), [1140, 780])
}
pub fn preview_sized(path: &std::path::Path, size: [i32; 2]) -> Result<(), String> {
    if size[0] < 960 || size[1] < 750 || size[0] > 2000 || size[1] > 1400 {
        return Err(t("error.preview_size").into());
    }
    run_window(false, Some(path), size)
}
fn run_window(
    start_in_tray: bool,
    preview: Option<&std::path::Path>,
    size: [i32; 2],
) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(None));
    let power = Arc::new(Mutex::new(Sampling::default()));
    let mut state = Box::new(State {
        tx,
        snapshot: shared.clone(),
        power: power.clone(),
        details: Cell::new(ptr::null_mut()),
        profile: Cell::new(ptr::null_mut()),
        preview: Cell::new(ptr::null_mut()),
        tray: crate::tray::TrayIcon::default(),
        taskbar_message: 0,
        theme: crate::theme::Theme::new(),
        preview_index: Cell::new(1),
        custom_profiles: RefCell::new(vec![]),
        selected_custom: RefCell::new(None),
        mode_result: Arc::new(Mutex::new(None)),
        applying_preset: Cell::new(false),
        mode_status: RefCell::new(t("mode.bios_unreadable").into()),
        close_to_tray: Cell::new(
            omen_platform::profile_store::ProfileStore::user_data()
                .map_or(true, |s| s.close_to_tray()),
        ),
    });
    // SAFETY: All window/class strings are nul-terminated. Box owns the stable
    // state address; Windows objects and power registrations are cleaned up.
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class_name = wide("OmenHubPlusPlusWindow");
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hIcon: crate::native_ui::app_icon(SM_CXICON),
            hIconSm: crate::native_ui::app_icon(SM_CXSMICON),
            hbrBackground: state.theme.background,
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassExW(&class) == 0 {
            return Err(tf("error.window", &[&std::io::Error::last_os_error()]));
        }
        state.taskbar_message = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide(APP_NAME).as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            size[0],
            size[1],
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            (&mut *state as *mut State).cast(),
        );
        if hwnd.is_null() {
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err(tf("error.window", &[&std::io::Error::last_os_error()]));
        }
        let power_notifications = match crate::power_events::Notifications::register(hwnd) {
            Ok(notifications) => notifications,
            Err(error) => {
                DestroyWindow(hwnd);
                UnregisterClassW(class_name.as_ptr(), instance);
                return Err(tf("error.window", &[&error]));
            }
        };
        let initially_visible = !start_in_tray || !state.tray.registered();
        let worker_hwnd = hwnd as usize;
        let mode_result = state.mode_result.clone();
        // Previews never request Windows approval.
        let auto_cpu = preview.is_none();
        let worker = thread::spawn(move || {
            worker(
                worker_hwnd,
                rx,
                shared,
                initially_visible,
                mode_result,
                auto_cpu,
                power,
            )
        });
        if initially_visible && preview.is_none() {
            ShowWindow(hwnd, SW_SHOW);
        }
        let mut message: MSG = std::mem::zeroed();
        let result = loop {
            let status = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
            if status == 0 {
                break Ok(());
            }
            if status == -1 {
                let error = std::io::Error::last_os_error();
                DestroyWindow(hwnd);
                break Err(tf("error.window", &[&error]));
            }
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            if message.message == UPDATE {
                if let Some(path) = preview {
                    let render = crate::curve_widget::render_window(hwnd, path);
                    DestroyWindow(hwnd);
                    break render;
                }
            }
        };
        let _ = state.tx.send(Request::Stop);
        // Read-only driver calls can still block inside a vendor DLL. UI shutdown
        // must not wait forever; process exit ends any detached telemetry thread.
        for _ in 0..20 {
            if worker.is_finished() {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        if worker.is_finished() {
            let _ = worker.join();
        }
        drop(power_notifications);
        UnregisterClassW(class_name.as_ptr(), instance);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_failure_copy_separates_permanent_support_from_approval_and_read_recovery() {
        let missing = cpu_error_message(
            "This build has no staged signed CPU module; use scripts/prepare-cpu-module.ps1",
        );
        assert_eq!(missing, t("cpu_error.no_module"));
        assert!(!missing.contains("scripts/"));
        let unsupported =
            cpu_error_message("CPU sensors are qualified only for HP8E35/F.13/Ryzen9 8940HX");
        assert_eq!(unsupported, t("cpu_error.unsupported"));
        let driver =
            cpu_error_message("PawnIO driver hash differs from the measured 2.2.0 contract");
        assert_eq!(driver, t("cpu_error.driver_version"));
        assert_eq!(
            cpu_error_message(
                "CPU-Sensorfreigabe nicht erfolgt: operation cancelled (os error 1223)"
            ),
            t("cpu_error.cancelled")
        );
        assert_eq!(
            cpu_error_message("CPU reader disconnected"),
            t("cpu_error.disconnected")
        );
    }
    #[test]
    fn cpu_status_line_is_empty_while_measuring_and_explains_other_states() {
        let mut snapshot = Snapshot {
            cpu: Some(omen_core::CpuSample::default()),
            cpu_hardware_status: CpuHardwareStatus::Active,
            ..Default::default()
        };
        assert!(crate::dashboard::cpu_detail(&snapshot).is_empty());
        snapshot.cpu_hardware_status = CpuHardwareStatus::Paused;
        assert!(crate::dashboard::cpu_detail(&snapshot).is_empty());
        snapshot.cpu_hardware_status = CpuHardwareStatus::Off;
        assert_eq!(crate::dashboard::cpu_detail(&snapshot), t("dash.cpu_off"));
        snapshot.cpu_hardware_status = CpuHardwareStatus::Starting;
        assert_eq!(
            crate::dashboard::cpu_detail(&snapshot),
            t("dash.cpu_starting")
        );
    }

    #[test]
    fn measured_cpu_clocks_are_distinct_from_windows_mhz_and_hidden_after_pause() {
        use omen_core::{CpuHardwareSample, CpuHardwareSource, CpuTemperatureKind, cpu::*};
        let mut snapshot = Snapshot {
            cpu: Some(omen_core::CpuSample {
                busy_percent: Some(12.5),
                windows_clock_mhz: Some(2401),
                hardware: Some(CpuHardwareSample {
                    schema: 1,
                    source: CpuHardwareSource::PawnioAmdRapl,
                    temperature_kind: CpuTemperatureKind::AmdTctl,
                    package_power_w: Some(35.0),
                    temperature_c: Some(55.0),
                    power_interval_ms: Some(1100),
                    effective_clocks: None,
                    warnings: vec![],
                }),
                ..Default::default()
            }),
            cpu_hardware_status: CpuHardwareStatus::Active,
            ..Default::default()
        };
        assert!(!crate::dashboard::cpu_load_line(&snapshot).contains("2401"));
        assert_eq!(
            crate::dashboard::cpu_detail(&snapshot),
            t("dash.effective_clock_measuring")
        );
        let hardware = snapshot.cpu.as_mut().unwrap().hardware.as_mut().unwrap();
        hardware.effective_clocks = Some(CpuEffectiveClocks {
            source: CpuClockSource::AmdAperfReadOnly,
            logical_processor_count: 2,
            processors: vec![
                CpuLogicalClock {
                    logical_processor: 0,
                    effective_mhz: 135.0,
                    interval_ms: 1100,
                },
                CpuLogicalClock {
                    logical_processor: 1,
                    effective_mhz: 3705.0,
                    interval_ms: 1101,
                },
            ],
        });
        let line = crate::dashboard::cpu_load_line(&snapshot);
        assert!(line.contains("3705") && line.contains(&crate::i18n::number("12.5")));
        assert!(!line.contains("2401"));
        assert!(crate::dashboard::cpu_detail(&snapshot).contains(&crate::i18n::number("1.1")));
        snapshot.cpu_hardware_status = CpuHardwareStatus::Paused;
        assert!(!crate::dashboard::cpu_load_line(&snapshot).contains("3705"));
        assert!(crate::dashboard::cpu_detail(&snapshot).is_empty());
        snapshot.cpu_hardware_status = CpuHardwareStatus::Active;
        let hardware = snapshot.cpu.as_mut().unwrap().hardware.as_mut().unwrap();
        hardware.effective_clocks = None;
        hardware
            .warnings
            .push("CPU effective clock measurement unavailable: reset".into());
        assert_eq!(
            crate::dashboard::cpu_detail(&snapshot),
            t("dash.effective_clock_unavailable")
        );
        assert_eq!(
            snapshot
                .cpu
                .as_ref()
                .unwrap()
                .hardware
                .as_ref()
                .unwrap()
                .package_power_w,
            Some(35.0)
        );
    }
}
