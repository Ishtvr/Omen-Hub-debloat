//! Small native Win32 shell. Hardware queries stay on one worker thread.
use omen_core::{APP_NAME, Profile, Snapshot};
use std::{
    cell::Cell,
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
const REFRESH: usize = 101;
const PROFILE: usize = 102;
const GPU_PROFILES: usize = 103;
const CURVES: usize = 104;
const SHOW: u32 = 201;
const EXIT: u32 = 202;

#[derive(Debug)]
enum Request {
    Refresh,
    Visibility(bool),
    Suspend,
    Resume,
    Power,
    Stop,
}

struct State {
    tx: mpsc::Sender<Request>,
    snapshot: Arc<Mutex<Option<Snapshot>>>,
    details: Cell<HWND>,
    profile: Cell<HWND>,
    preview: Cell<HWND>,
    font: Cell<HFONT>,
    tray: Cell<bool>,
    taskbar_message: u32,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn snapshot_text(snapshot: &Snapshot) -> String {
    let i = &snapshot.inventory;
    let power = match snapshot.power.ac_connected {
        Some(true) => "Netzbetrieb",
        Some(false) => "Akkubetrieb",
        None => "Unbekannt",
    };
    let battery = snapshot
        .power
        .battery_percent
        .map(|p| format!("{p} %"))
        .unwrap_or_else(|| "Nicht verfügbar".into());
    // Both processors lead the report; inventory must not push CPU readings
    // below the first viewport in the current diagnostic prototype.
    let mut text = String::new();
    if let Some(gpu) = &snapshot.gpu {
        let temp = gpu
            .temperature_c
            .map(|v| format!("{v} °C"))
            .unwrap_or_else(|| "Nicht verfügbar".into());
        let clock = gpu
            .clock_mhz
            .map(|v| format!("{v} MHz"))
            .unwrap_or_else(|| "Nicht verfügbar".into());
        let watts = |v: Option<f64>| {
            v.map(|v| format!("{v:.1} W"))
                .unwrap_or_else(|| "Nicht verfügbar".into())
        };
        text.push_str(&format!(
            "NVIDIA SENSOREN\r\n{}\r\nTemperatur: {temp}    Takt: {clock}\r\nLeistungsaufnahme: {}\r\nStandard-Power-Limit: {}    Obergrenze laut Treiber: {}\r\nDie Obergrenze ist keine Messung dauerhaft erreichbarer Leistung.\r\n\r\n",
            gpu.name, watts(gpu.power_w), watts(gpu.default_limit_w), watts(gpu.maximum_limit_w)
        ));
    } else {
        text.push_str("NVIDIA SENSOREN\r\nDerzeit nicht verfügbar.\r\n\r\n");
    }
    if let Some(cpu) = &snapshot.cpu {
        let usage = cpu
            .busy_percent
            .map(|v| format!("{v:.1} %"))
            .unwrap_or_else(|| "Wird gemessen / nicht verfügbar".into());
        let clock = cpu
            .windows_clock_mhz
            .map(|v| format!("{v} MHz"))
            .unwrap_or_else(|| "Nicht verfügbar".into());
        let watts = cpu
            .package_power_w
            .map(|v| format!("{v:.1} W"))
            .unwrap_or_else(|| "Nicht verfügbar – Hardwareanbindung ausstehend".into());
        let temperature = cpu
            .temperature_c
            .map(|v| format!("{v:.1} °C"))
            .unwrap_or_else(|| "Nicht verfügbar – Hardwareanbindung ausstehend".into());
        text.push_str(&format!("CPU LEISTUNG\r\nAuslastung: {usage}    Takt (Windows): {clock}\r\nCPU-Paketleistung: {watts}\r\nCPU-Temperatur: {temperature}\r\nDer Windows-Takt ist kein gemessener Boost- oder Effektivtakt.\r\n\r\n"));
    } else {
        text.push_str("CPU LEISTUNG\r\nDerzeit nicht verfügbar.\r\n\r\n");
    }
    text.push_str(&format!(
        "SYSTEM\r\n{} {}\r\nBoard: {}    BIOS: {}\r\nCPU: {}\r\nGrafik: {}\r\n\r\nVERSORGUNG\r\n{power}    Akku: {battery}\r\n\r\n",
        i.manufacturer,
        i.model,
        i.board,
        i.bios,
        i.cpus.join(", "),
        i.gpus.join(", ")
    ));
    text.push_str(if i.hp_bios_interface_present {
        "HP-Schnittstelle erkannt. Modell-Unterstützung wird geprüft.\r\n"
    } else {
        "HP-Schnittstelle derzeit nicht verfügbar.\r\n"
    });
    text.push_str("Lüfter, MUX und RGB: noch nicht angebunden.\r\n");
    let count = i.warnings.len()
        + snapshot.warnings.len()
        + snapshot.gpu.as_ref().map_or(0, |g| g.warnings.len());
    let count = count + snapshot.cpu.as_ref().map_or(0, |cpu| cpu.warnings.len());
    if count > 0 {
        text.push_str(
            "Einige Abfragen sind nicht verfügbar; der Diagnosebericht enthält Details.\r\n",
        );
    }
    text
}

fn worker(
    hwnd: usize,
    rx: mpsc::Receiver<Request>,
    shared: Arc<Mutex<Option<Snapshot>>>,
    initially_visible: bool,
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
        } else {
            cpu = None;
            snapshot.cpu = None;
        }
        if visible && !suspended && Instant::now() >= reopen_at {
            let nvidia = nvidia.get_or_insert_with(omen_platform::Nvidia::new);
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
        match request {
            Some(Request::Stop) => break,
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
    // SAFETY: Shared system icon, never destroyed by this process.
    data.hIcon = unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) };
    for (target, value) in data.szTip.iter_mut().zip(wide(APP_NAME)) {
        *target = value;
    }
    data
}

// SAFETY: Called only on the UI thread for the live, owned State bound to hwnd.
unsafe fn redraw(state: &State) {
    let text = state
        .snapshot
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .map(snapshot_text);
    unsafe {
        if let Some(text) = text {
            SetWindowTextW(state.details.get(), wide(&text).as_ptr());
        }
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
        match message {
            WM_CREATE => {
                let instance = GetModuleHandleW(ptr::null());
                let mut font_data: LOGFONTW = std::mem::zeroed();
                font_data.lfHeight = -18;
                for (target, value) in font_data.lfFaceName.iter_mut().zip(wide("Segoe UI")) {
                    *target = value;
                }
                state.font.set(CreateFontIndirectW(&font_data));
                let control =
                    |class: &str, title: &str, style: u32, x, y, width, height, id: usize| {
                        let handle = CreateWindowExW(
                            0,
                            wide(class).as_ptr(),
                            wide(title).as_ptr(),
                            WS_CHILD | WS_VISIBLE | style,
                            x,
                            y,
                            width,
                            height,
                            hwnd,
                            id as HMENU,
                            instance,
                            ptr::null(),
                        );
                        SendMessageW(handle, WM_SETFONT, state.font.get() as usize, 1);
                        handle
                    };
                control(
                    "STATIC",
                    "Omen Hub++  |  Systemübersicht",
                    0,
                    20,
                    18,
                    740,
                    28,
                    0,
                );
                control(
                    "STATIC",
                    "Hardwareübersicht · GPU-Taktprofile verfügbar · HP-Modi noch in Validierung",
                    0,
                    20,
                    49,
                    740,
                    25,
                    0,
                );
                state.details.set(control(
                    "EDIT",
                    "Hardware wird erkannt …",
                    WS_BORDER
                        | WS_VSCROLL
                        | ES_MULTILINE as u32
                        | ES_READONLY as u32
                        | ES_AUTOVSCROLL as u32,
                    20,
                    85,
                    750,
                    390,
                    0,
                ));
                control(
                    "BUTTON",
                    "Aktualisieren",
                    WS_TABSTOP,
                    20,
                    493,
                    160,
                    32,
                    REFRESH,
                );
                control("STATIC", "Profil (Vorschau)", 0, 205, 497, 160, 25, 0);
                state.profile.set(control(
                    "COMBOBOX",
                    "",
                    WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                    370,
                    493,
                    210,
                    200,
                    PROFILE,
                ));
                for label in ["Quiet", "Balanced", "Performance", "Unleashed"] {
                    SendMessageW(
                        state.profile.get(),
                        CB_ADDSTRING,
                        0,
                        wide(label).as_ptr() as isize,
                    );
                }
                SendMessageW(state.profile.get(), CB_SETCURSEL, 1, 0);
                state.preview.set(control(
                    "STATIC",
                    "Balanced: Vorschau. Noch keine Hardware-Einstellungen angewendet.",
                    0,
                    20,
                    544,
                    550,
                    53,
                    0,
                ));
                control(
                    "BUTTON",
                    "GPU-Profile …",
                    WS_TABSTOP,
                    596,
                    493,
                    174,
                    32,
                    GPU_PROFILES,
                );
                control(
                    "BUTTON",
                    "Lüfter & V/F …",
                    WS_TABSTOP,
                    596,
                    544,
                    174,
                    34,
                    CURVES,
                );
                control(
                    "STATIC",
                    "Beim Schließen bleibt die App im Tray. Rechtsklick auf das Tray-Symbol: Beenden.",
                    0,
                    20,
                    601,
                    750,
                    28,
                    0,
                );
                if state.details.get().is_null()
                    || state.profile.get().is_null()
                    || state.preview.get().is_null()
                {
                    return -1;
                }
                state
                    .tray
                    .set(Shell_NotifyIconW(NIM_ADD, &tray_data(hwnd)) != 0);
                0
            }
            WM_COMMAND => {
                let id = wparam & 0xffff;
                if id == REFRESH {
                    let _ = state.tx.send(Request::Refresh);
                }
                if id == GPU_PROFILES || id == CURVES {
                    match std::env::current_exe().and_then(|path| {
                        std::process::Command::new(path)
                            .arg(if id == CURVES { "curve-ui" } else { "gpu-ui" })
                            .spawn()
                    }) {
                        Ok(_) => {}
                        Err(error) => {
                            SetWindowTextW(
                                state.preview.get(),
                                wide(&format!(
                                    "GPU-Profile konnten nicht geöffnet werden: {error}"
                                ))
                                .as_ptr(),
                            );
                        }
                    }
                }
                if id == PROFILE && (wparam >> 16) as u32 == CBN_SELCHANGE {
                    let index = SendMessageW(state.profile.get(), CB_GETCURSEL, 0, 0);
                    let profile = match index {
                        0 => Profile::Quiet,
                        2 => Profile::Performance,
                        3 => Profile::Unleashed,
                        _ => Profile::Balanced,
                    };
                    let label = format!(
                        "{profile:?}: Vorschau. Noch keine Hardware-Einstellungen angewendet."
                    );
                    SetWindowTextW(state.preview.get(), wide(&label).as_ptr());
                }
                if id == SHOW as usize {
                    ShowWindow(hwnd, SW_RESTORE);
                    SetForegroundWindow(hwnd);
                    let _ = state.tx.send(Request::Visibility(true));
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
                        ShowWindow(hwnd, SW_RESTORE);
                        SetForegroundWindow(hwnd);
                        let _ = state.tx.send(Request::Visibility(true));
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
                ShowWindow(hwnd, SW_HIDE);
                let _ = state.tx.send(Request::Visibility(false));
                0
            }
            WM_CLOSE => {
                if state.tray.get() {
                    ShowWindow(hwnd, SW_HIDE);
                    let _ = state.tx.send(Request::Visibility(false));
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
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(None));
    let mut state = Box::new(State {
        tx,
        snapshot: shared.clone(),
        details: Cell::new(ptr::null_mut()),
        profile: Cell::new(ptr::null_mut()),
        preview: Cell::new(ptr::null_mut()),
        font: Cell::new(ptr::null_mut()),
        tray: Cell::new(false),
        taskbar_message: 0,
    });
    // SAFETY: All window/class strings are nul-terminated. Box owns the stable
    // state address; Windows objects and power registrations are cleaned up.
    unsafe {
        SetProcessDPIAware();
        let instance = GetModuleHandleW(ptr::null());
        let class_name = wide("OmenHubPlusPlusWindow");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hIcon: LoadIconW(ptr::null_mut(), IDI_APPLICATION),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        if RegisterClassW(&class) == 0 {
            return Err(format!("Window class: {}", std::io::Error::last_os_error()));
        }
        state.taskbar_message = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide(APP_NAME).as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            808,
            680,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            (&mut *state as *mut State).cast(),
        );
        if hwnd.is_null() {
            if !state.font.get().is_null() {
                DeleteObject(state.font.get());
            }
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err(format!(
                "Window creation: {}",
                std::io::Error::last_os_error()
            ));
        }
        let initially_visible = !start_in_tray || !state.tray.get();
        let worker_hwnd = hwnd as usize;
        let worker = thread::spawn(move || worker(worker_hwnd, rx, shared, initially_visible));
        let power_registration = RegisterPowerSettingNotification(
            hwnd,
            &GUID_ACDC_POWER_SOURCE,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        );
        if initially_visible {
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
        if !state.font.get().is_null() {
            DeleteObject(state.font.get());
        }
        UnregisterClassW(class_name.as_ptr(), instance);
        result
    }
}
