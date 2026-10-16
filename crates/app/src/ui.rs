//! Small native Win32 shell. Hardware queries stay on one worker thread.
use omen_core::{APP_NAME, CpuHardwareStatus, Snapshot};
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
    System::{LibraryLoader::GetModuleHandleW, Power::*, SystemServices::GUID_ACDC_POWER_SOURCE},
    UI::{Shell::*, WindowsAndMessaging::*},
};

const UPDATE: u32 = WM_APP + 1;
const TRAY: u32 = WM_APP + 2;
const CPU_READER_PID: u32 = WM_APP + 61;
const MODE_RESULT: u32 = WM_APP + 62;
const REFRESH: usize = 101;
const PROFILE: usize = 102;
const GPU_PROFILES: usize = 103;
const CURVES: usize = 104;
const KEYBOARD: usize = 105;
const CPU_SENSORS: usize = 106;
const SHOW: u32 = 201;
const EXIT: u32 = 202;

#[derive(Debug, Clone)]
enum Request {
    Refresh,
    Visibility(bool),
    Suspend,
    Resume,
    Power,
    CpuSensors,
    ApplyPreset(String),
    Stop,
}

struct State {
    tx: mpsc::Sender<Request>,
    snapshot: Arc<Mutex<Option<Snapshot>>>,
    details: Cell<HWND>,
    profile: Cell<HWND>,
    preview: Cell<HWND>,
    cpu_sensor: Cell<HWND>,
    tray: Cell<bool>,
    taskbar_message: u32,
    theme: crate::theme::Theme,
    preview_index: Cell<usize>,
    custom_profiles: RefCell<Vec<(String, String)>>,
    selected_custom: RefCell<Option<String>>,
    mode_result: Arc<Mutex<Option<String>>>,
    applying_preset: Cell<bool>,
    mode_status: RefCell<String>,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub(crate) fn cpu_error_message(detail: &str) -> &'static str {
    if detail.starts_with("This build has no staged signed CPU module") {
        "Diese App-Version enthält keinen CPU-Sensoradapter. Vollständigen Build verwenden."
    } else if detail.contains("qualified only")
        || detail.contains("family/model/stepping/package")
        || detail.contains("requires Windows x64")
    {
        "CPU-Sensoren sind für dieses Gerät/BIOS nicht freigegeben. Geräteunterstützung prüfen."
    } else if detail.starts_with("PawnIO must already be installed and running") {
        "PawnIO fehlt oder läuft nicht. Sensor-Voraussetzungen in der Anleitung prüfen."
    } else if detail.contains("hash differs")
        || detail.contains("version/response differs")
        || detail.contains("Embedded CPU module differs")
        || detail.contains("Signed CPU module load")
        || detail.contains("driver path")
    {
        "Treiber-/Modulversion nicht freigegeben. Passende Version von Omen Hub++ verwenden."
    } else if detail.starts_with("CPU-Sensorfreigabe nicht erfolgt:") && detail.contains("1223") {
        "CPU-Sensorfreigabe abgebrochen. Zum Aktivieren erneut CPU-Sensoren … wählen."
    } else if detail.starts_with("CPU-Sensorfreigabe nicht erfolgt:")
        || detail.starts_with("CPU driver access requires")
    {
        "Windows hat den CPU-Zugriff verweigert. Berechtigungen und PawnIO-Installation prüfen."
    } else if detail.contains("pipe peer")
        || detail.contains("server PID")
        || detail.contains("stale or unrelated")
    {
        "CPU-Leseprozess konnte nicht sicher zugeordnet werden. App neu öffnen."
    } else if detail.contains("failed to connect")
        || detail.contains("disconnected")
        || detail.contains("timed out")
        || detail.contains("request pipe closed")
    {
        "CPU-Verbindung unterbrochen. Sensoren erneut aktivieren."
    } else {
        "CPU-Messung unterbrochen; alte Werte verworfen. Sensoren erneut aktivieren."
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
) {
    let mut snapshot = Snapshot {
        inventory: crate::inventory::read(),
        power: omen_platform::power_status(),
        ..Default::default()
    };
    let mut visible = initially_visible;
    let mut suspended = false;
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
        if visible && !suspended {
            snapshot.cpu = Some(cpu.get_or_insert_with(omen_platform::Cpu::new).sample());
            if let Some(reader) = hardware.as_mut() {
                match reader.sample() {
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
        } else {
            cpu = None;
            snapshot.cpu = None;
        }
        if visible && !suspended && Instant::now() >= reopen_at {
            let nvidia = nvidia.get_or_insert_with(crate::telemetry_worker::GpuTelemetry::new);
            snapshot.gpu = nvidia.sample();
            snapshot.warnings = nvidia.warnings().to_vec();
            if nvidia.needs_reconnect() {
                // Do not reuse lost handles. A 30-second cooldown avoids a retry loop.
                reopen_at = Instant::now() + Duration::from_secs(30);
                snapshot.gpu = None;
                snapshot
                    .warnings
                    .push("NVIDIA session lost; retrying after 30 seconds".into());
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
        snapshot.power = omen_platform::power_status();
        snapshot.cpu_reader_pid = hardware.as_ref().map(|r| r.process_id());
        publish(&snapshot);
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
        let reset_hardware = matches!(
            &request,
            Some(Request::Resume | Request::Refresh | Request::Power)
        );
        match request {
            Some(Request::Stop) => break,
            Some(Request::CpuSensors) => {
                if hardware.is_some() {
                    hardware = None;
                    snapshot.cpu_hardware_status = CpuHardwareStatus::Off;
                } else if visible && !suspended {
                    snapshot.cpu_hardware_status = CpuHardwareStatus::Starting;
                    publish(&snapshot);
                    match crate::cpu_sensor_bridge::CpuSensorBridge::start() {
                        Ok(reader) => hardware = Some(reader),
                        Err(e) => cpu_failed(&mut snapshot, e),
                    }
                }
                cpu = None;
            }
            Some(Request::ApplyPreset(id)) => {
                let result =
                    crate::elevation::run(Some(&id), crate::elevation::Action::Apply, || false);
                *mode_result.lock().unwrap_or_else(|e| e.into_inner()) = Some(if result.is_ok() {
                    "Preset angewendet.".into()
                } else {
                    "Preset konnte nicht angewendet werden. Einstellungen unter Leistungsprofile prüfen.".into()
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
            Some(Request::Suspend) => suspended = true,
            Some(Request::Resume) => {
                cpu = None;
                suspended = false;
                nvidia = None;
                reopen_at = Instant::now();
            }
            Some(Request::Refresh) => {
                cpu = None;
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
            for label in [
                "Quiet (Default)",
                "Balanced (Default)",
                "Performance (Default)",
            ] {
                SendMessageW(
                    state.profile.get(),
                    CB_ADDSTRING,
                    0,
                    wide(label).as_ptr() as isize,
                );
            }
            let mut selected = state.preview_index.get().min(2);
            for (i, (id, name)) in entries.iter().enumerate() {
                SendMessageW(
                    state.profile.get(),
                    CB_ADDSTRING,
                    0,
                    wide(name).as_ptr() as isize,
                );
                if state.selected_custom.borrow().as_ref() == Some(id) {
                    selected = i + 3;
                }
            }
            if selected < 3 {
                *state.selected_custom.borrow_mut() = None;
            }
            *state.custom_profiles.borrow_mut() = entries;
            SendMessageW(state.profile.get(), CB_SETCURSEL, selected, 0);
            if selected < 3 {
                *state.mode_status.borrow_mut() = format!(
                    "{} (Default): noch nicht verfügbar.",
                    ["Quiet", "Balanced", "Performance"][selected]
                );
            }
            SetWindowTextW(
                state.preview.get(),
                wide(&state.mode_status.borrow()).as_ptr(),
            );
            windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                state.profile.get(),
                (!state.applying_preset.get()) as i32,
            );
        } else {
            SetWindowTextW(
                GetDlgItem(hwnd, 181),
                wide("Gespeicherte Presets konnten nicht geladen werden.").as_ptr(),
            );
        }
    }
}

fn tray_data(hwnd: HWND) -> NOTIFYICONDATAW {
    // SAFETY: NOTIFYICONDATAW has no invalid zero bit patterns.
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = TRAY;
    // Shared embedded icon, never destroyed by this process.
    data.hIcon = crate::native_ui::app_icon(SM_CXSMICON);
    for (target, value) in data.szTip.iter_mut().zip(wide(APP_NAME)) {
        *target = value;
    }
    data
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
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
            EnableWindow(
                state.cpu_sensor.get(),
                (!matches!(snapshot.cpu_hardware_status, CpuHardwareStatus::Starting)) as i32,
            );
            SetWindowTextW(
                state.cpu_sensor.get(),
                wide(
                    if matches!(
                        snapshot.cpu_hardware_status,
                        CpuHardwareStatus::Active | CpuHardwareStatus::Paused
                    ) {
                        "CPU-Sensoren aus"
                    } else if matches!(snapshot.cpu_hardware_status, CpuHardwareStatus::Starting) {
                        "Windows-Freigabe …"
                    } else {
                        "CPU-Sensoren …"
                    },
                )
                .as_ptr(),
            );
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
            state.cpu_sensor.set(GetDlgItem(hwnd, CPU_SENSORS as i32));
            reload_modes(hwnd, state);
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
        state.cpu_sensor.set(ptr::null_mut());
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
        if message == state.taskbar_message {
            state
                .tray
                .set(Shell_NotifyIconW(NIM_ADD, &tray_data(hwnd)) != 0);
            if !state.tray.get() {
                ShowWindow(hwnd, SW_SHOW);
                let _ = state.tx.send(Request::Visibility(true));
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
                state.cpu_sensor.set(GetDlgItem(hwnd, CPU_SENSORS as i32));
                reload_modes(hwnd, state);
                if [
                    state.details.get(),
                    state.profile.get(),
                    state.preview.get(),
                    state.cpu_sensor.get(),
                ]
                .iter()
                .any(|h| h.is_null())
                {
                    return -1;
                }
                state.theme.frame(hwnd);
                state
                    .tray
                    .set(Shell_NotifyIconW(NIM_ADD, &tray_data(hwnd)) != 0);
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
                if item.CtlType == windows_sys::Win32::UI::Controls::ODT_BUTTON {
                    state
                        .theme
                        .button(item, state.theme.body, item.CtlID == GPU_PROFILES as u32);
                    1
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(lparam as *mut MINMAXINFO);
                info.ptMinTrackSize = POINT { x: 960, y: 790 };
                0
            }
            WM_COMMAND => {
                let id = wparam & 0xffff;
                // Owner-drawn buttons also send non-click notifications.
                // Focus/paint/double-click notifications cannot trigger actions.
                if id != PROFILE && (wparam >> 16) as u32 != BN_CLICKED {
                    return 0;
                }
                if id == REFRESH {
                    reload_modes(hwnd, state);
                    let _ = state.tx.send(Request::Refresh);
                }
                if id == CPU_SENSORS {
                    let _ = state.tx.send(Request::CpuSensors);
                }
                if id == GPU_PROFILES || id == CURVES || id == KEYBOARD {
                    match std::env::current_exe().and_then(|path| {
                        std::process::Command::new(path)
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
                                wide(&format!("Steuerung konnte nicht geöffnet werden: {error}"))
                                    .as_ptr(),
                            );
                        }
                    }
                }
                if id == PROFILE
                    && (wparam >> 16) as u32 == CBN_SELENDOK
                    && !state.applying_preset.get()
                {
                    let index = SendMessageW(state.profile.get(), CB_GETCURSEL, 0, 0);
                    if index >= 3 {
                        let selected = state
                            .custom_profiles
                            .borrow()
                            .get(index as usize - 3)
                            .cloned();
                        if let Some((id, _)) = selected {
                            state.applying_preset.set(true);
                            windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                                state.profile.get(),
                                0,
                            );
                            *state.selected_custom.borrow_mut() = Some(id.clone());
                            SetWindowTextW(
                                state.preview.get(),
                                wide("Windows-Freigabe …").as_ptr(),
                            );
                            *state.mode_status.borrow_mut() = "Windows-Freigabe …".into();
                            let _ = state.tx.send(Request::ApplyPreset(id));
                        }
                    } else if index >= 0 {
                        state.preview_index.set(index as usize);
                        *state.selected_custom.borrow_mut() = None;
                        let label = ["Quiet", "Balanced", "Performance"][index as usize];
                        *state.mode_status.borrow_mut() =
                            format!("{label} (Default): noch nicht verfügbar.");
                        SetWindowTextW(
                            state.preview.get(),
                            wide(&format!("{label} (Default): noch nicht verfügbar.")).as_ptr(),
                        );
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
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                        state.profile.get(),
                        1,
                    );
                }
                if let Some(message) = state
                    .mode_result
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take()
                {
                    *state.mode_status.borrow_mut() = message.clone();
                    if !state.preview.get().is_null() {
                        SetWindowTextW(state.preview.get(), wide(&message).as_ptr());
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
            WM_POWERBROADCAST => {
                match wparam as u32 {
                    PBT_APMSUSPEND => {
                        let _ = state.tx.send(Request::Suspend);
                    }
                    PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                        let _ = state.tx.send(Request::Resume);
                    }
                    PBT_APMPOWERSTATUSCHANGE | PBT_POWERSETTINGCHANGE => {
                        let _ = state.tx.send(Request::Power);
                    }
                    _ => (),
                }
                1
            }
            TRAY => {
                match lparam as u32 {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                        show_dashboard(hwnd, state);
                    }
                    WM_RBUTTONUP | WM_CONTEXTMENU => {
                        let menu = CreatePopupMenu();
                        AppendMenuW(
                            menu,
                            MF_STRING,
                            SHOW as usize,
                            wide("Omen Hub++ öffnen").as_ptr(),
                        );
                        AppendMenuW(menu, MF_STRING, EXIT as usize, wide("Beenden").as_ptr());
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
            WM_SIZE if wparam == SIZE_MINIMIZED as usize && state.tray.get() => {
                hide_dashboard(hwnd, state);
                0
            }
            WM_SIZE => {
                crate::dashboard::layout(hwnd);
                0
            }
            WM_CLOSE => {
                if state.tray.get() {
                    hide_dashboard(hwnd, state);
                } else {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_DESTROY => {
                let _ = state.tx.send(Request::Stop);
                if state.tray.get() {
                    Shell_NotifyIconW(NIM_DELETE, &tray_data(hwnd));
                }
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

pub fn run(start_in_tray: bool) -> Result<(), String> {
    run_window(start_in_tray, None, [1140, 820])
}
pub fn preview(path: &std::path::Path) -> Result<(), String> {
    run_window(false, Some(path), [1140, 820])
}
pub fn preview_sized(path: &std::path::Path, size: [i32; 2]) -> Result<(), String> {
    if size[0] < 960 || size[1] < 790 || size[0] > 2000 || size[1] > 1400 {
        return Err("Preview size outside dashboard bounds".into());
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
    let mut state = Box::new(State {
        tx,
        snapshot: shared.clone(),
        details: Cell::new(ptr::null_mut()),
        profile: Cell::new(ptr::null_mut()),
        preview: Cell::new(ptr::null_mut()),
        cpu_sensor: Cell::new(ptr::null_mut()),
        tray: Cell::new(false),
        taskbar_message: 0,
        theme: crate::theme::Theme::new(),
        preview_index: Cell::new(1),
        custom_profiles: RefCell::new(vec![]),
        selected_custom: RefCell::new(None),
        mode_result: Arc::new(Mutex::new(None)),
        applying_preset: Cell::new(false),
        mode_status: RefCell::new("Balanced (Default): noch nicht verfügbar.".into()),
    });
    // SAFETY: All window/class strings are nul-terminated. Box owns the stable
    // state address; Windows objects and power registrations are cleaned up.
    unsafe {
        SetProcessDPIAware();
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
            return Err(format!("Window class: {}", std::io::Error::last_os_error()));
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
            return Err(format!(
                "Window creation: {}",
                std::io::Error::last_os_error()
            ));
        }
        let initially_visible = !start_in_tray || !state.tray.get();
        let worker_hwnd = hwnd as usize;
        let mode_result = state.mode_result.clone();
        let worker =
            thread::spawn(move || worker(worker_hwnd, rx, shared, initially_visible, mode_result));
        let power_registration = RegisterPowerSettingNotification(
            hwnd,
            &GUID_ACDC_POWER_SOURCE,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        );
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
                break Err(format!("Window messages: {error}"));
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
        if power_registration != 0 {
            UnregisterPowerSettingNotification(power_registration);
        }
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
        assert!(missing.contains("Vollständigen Build"));
        assert!(!missing.contains("scripts/"));
        let unsupported =
            cpu_error_message("CPU sensors are qualified only for HP8E35/F.13/Ryzen9 8940HX");
        assert!(unsupported.contains("Gerät/BIOS"));
        assert!(!unsupported.contains("erneut"));
        let driver =
            cpu_error_message("PawnIO driver hash differs from the measured 2.2.0 contract");
        assert!(driver.contains("Treiber-/Modulversion"));
        assert!(!driver.contains("erneut"));
        assert!(
            cpu_error_message(
                "CPU-Sensorfreigabe nicht erfolgt: operation cancelled (os error 1223)"
            )
            .contains("abgebrochen")
        );
        assert!(cpu_error_message("CPU reader disconnected").contains("erneut aktivieren"));
    }
    #[test]
    fn cpu_status_distinguishes_energy_baseline_and_mean_with_tctl_provenance() {
        let hardware = omen_core::CpuHardwareSample {
            schema: 1,
            source: omen_core::CpuHardwareSource::PawnioAmdRapl,
            temperature_kind: omen_core::CpuTemperatureKind::AmdTctl,
            package_power_w: Some(55.0),
            temperature_c: Some(82.0),
            power_interval_ms: Some(1100),
            warnings: vec![],
        };
        let mut snapshot = Snapshot {
            cpu: Some(omen_core::CpuSample {
                windows_clock_mhz: Some(2400),
                package_power_w: hardware.package_power_w,
                temperature_c: hardware.temperature_c,
                hardware: Some(hardware),
                ..Default::default()
            }),
            cpu_hardware_status: CpuHardwareStatus::Active,
            ..Default::default()
        };
        let report = crate::dashboard::cpu_detail(&snapshot);
        assert!(report.contains("Mittelwert über 1.10 s"));
        assert!(report.contains("Temperatur: Tctl"));
        assert!(report.contains("Quelle: CPU-Energiezähler und Tctl über PawnIO"));
        snapshot
            .cpu
            .as_mut()
            .unwrap()
            .hardware
            .as_mut()
            .unwrap()
            .power_interval_ms = None;
        let baseline = crate::dashboard::cpu_detail(&snapshot);
        assert!(baseline.contains("Energie-Baseline"));
        assert!(!baseline.contains("Mittelwert über"));
        snapshot.cpu = Some(omen_core::CpuSample::default());
        snapshot.cpu_hardware_status = CpuHardwareStatus::Off;
        assert!(crate::dashboard::cpu_detail(&snapshot).contains("nicht aktiviert"));
    }
}
