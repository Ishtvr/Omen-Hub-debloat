//! Native GPU profile editor. Save is metadata; apply/restore use a fixed UAC child.
use omen_core::{
    features::Feature,
    profiles::{PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
    transaction::{self, Access, Backend},
    vf::{self, VfState},
};
use omen_platform::{Controller, GpuControlsReport, profile_store::ProfileStore};
use std::{
    cell::{Cell, RefCell},
    ptr,
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::UI::{
    Controls::{BST_CHECKED, BST_UNCHECKED, EM_SETLIMITTEXT},
    Input::KeyboardAndMouse::EnableWindow,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::GetModuleHandleW, Threading::*},
    UI::{Shell::*, WindowsAndMessaging::*},
};

const UPDATE: u32 = WM_APP + 11;
const SELECT: usize = 301;
const NEW: usize = 302;
const SAVE: usize = 303;
const APPLY: usize = 304;
const RESTORE: usize = 305;
const STARTUP: usize = 306;
const DISABLE: usize = 307;
const REFRESH: usize = 308;
const NAME: usize = 309;
const CORE: usize = 310;
const MEMORY: usize = 311;
const CURVE: usize = 312;
const VOLTAGE: usize = 313;
const FREQUENCY: usize = 314;
const CURVE_EDITOR: usize = 315;
pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
pub(crate) fn token() -> String {
    format!(
        "{:x}{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id()
    )
}

struct View {
    profiles: Vec<PerformanceProfile>,
    selected: Option<PerformanceProfile>,
    gpu: GpuControlsReport,
    ranges: [(i64, i64); 2],
    ready: bool,
    core_ready: bool,
    curve_ready: bool,
    curve: Result<VfState, String>,
    applicable: bool,
    startup: String,
    blockers: String,
}
fn load(id: Option<&str>) -> Result<View, String> {
    let store = ProfileStore::user_data()?;
    let profiles = store.list()?;
    let selected = id.map(|id| store.load(id)).transpose()?;
    let mut controller = Controller::new(true, None)?;
    let gpu = controller.gpu_report()?;
    let caps = [
        controller.capability(Feature::GpuCoreOffsetMhz),
        controller.capability(Feature::GpuMemoryOffsetMhz),
    ];
    let core_ready = caps[0].access == Access::ReadWrite;
    let curve_cap = controller.capability(Feature::GpuVfCurve);
    let curve_ready = curve_cap.access == Access::ReadWrite;
    let curve = controller.gpu_curve();
    let ready = caps[1].access == Access::ReadWrite && (core_ready || curve_ready);
    let ranges = [
        caps[0].integer_range.unwrap_or((0, 0)),
        caps[1].integer_range.unwrap_or((0, 0)),
    ];
    let plan = selected
        .as_ref()
        .map(|p| transaction::plan(p, &mut controller, false));
    let applicable = plan.as_ref().is_some_and(|p| p.applicable);
    let blockers = plan.map_or(String::new(), |p| {
        p.blockers
            .iter()
            .map(|b| b.reason.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    });
    let startup = match store.startup_profile() {
        Ok(Some(p)) => match omen_platform::verify_startup_profile(&p) {
            Ok(()) => format!("Für die nächste Windows-Anmeldung vorgemerkt: {}", p.name),
            Err(e) => format!("Nach Anmeldung gesperrt: {e}"),
        },
        Ok(None) => "Nach Anmeldung: kein Profil ausgewählt.".into(),
        Err(error) => format!("Nach Anmeldung gesperrt: {error}"),
    };
    Ok(View {
        profiles,
        selected,
        gpu,
        ranges,
        ready,
        core_ready,
        curve_ready,
        curve,
        applicable,
        startup,
        blockers,
    })
}
enum Request {
    Load(Option<String>),
    Save {
        id: Option<String>,
        name: String,
        graphics: Setting,
        memory: i64,
    },
    Action {
        id: Option<String>,
        restore: bool,
    },
    Startup(String),
    Disable,
    Stop,
}
enum Update {
    View(Box<View>, String),
    Status(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Apply,
    Restore,
    Startup,
}
impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::Restore => "restore",
            Self::Startup => "startup",
        }
    }
}

fn elevated(
    id: Option<&str>,
    action: Action,
    receiver: &mpsc::Receiver<Request>,
) -> Result<String, String> {
    let nonce = token();
    let arguments = if action == Action::Restore {
        format!("profile-ui-restore {nonce}")
    } else {
        let id = id.ok_or("Zuerst ein Profil speichern.")?;
        omen_core::profiles::validate_id(id)?;
        match action {
            Action::Apply => format!("profile-ui-apply {id} {nonce}"),
            Action::Startup => format!("profile-ui-startup-enable {id} {nonce}"),
            Action::Restore => unreachable!(),
        }
    };
    let file = wide(
        &std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy(),
    );
    let parameters = wide(&arguments);
    let verb = wide("runas");
    let mut execute: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    execute.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    execute.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    execute.lpVerb = verb.as_ptr();
    execute.lpFile = file.as_ptr();
    execute.lpParameters = parameters.as_ptr();
    execute.nShow = SW_HIDE;
    // SAFETY: Only our executable and fixed typed commands; strings live through launch.
    if unsafe { ShellExecuteExW(&mut execute) } == 0 {
        return Err(format!(
            "Windows-Freigabe nicht erfolgt: {}",
            std::io::Error::last_os_error()
        ));
    }
    if execute.hProcess.is_null() {
        return Err("Windows gab keinen Prozess zur Ergebnisprüfung zurück.".into());
    }
    let result = (|| {
        let deadline = std::time::Instant::now() + Duration::from_secs(45);
        loop {
            // SAFETY: Handle was returned with SEE_MASK_NOCLOSEPROCESS and is live.
            match unsafe { WaitForSingleObject(execute.hProcess, 50) } {
                WAIT_OBJECT_0 => break,
                WAIT_TIMEOUT => {}
                _ => return Err(std::io::Error::last_os_error().to_string()),
            }
            if matches!(
                receiver.try_recv(),
                Ok(Request::Stop) | Err(mpsc::TryRecvError::Disconnected)
            ) {
                return Err("Fenster geschlossen; die laufende Transaktion schließt ihre Prüfung und Wiederherstellung selbst ab.".into());
            }
            if std::time::Instant::now() >= deadline {
                return Err("Der Steuerprozess läuft noch. Sein Ergebnis prüfen, bevor eine weitere Aktion begonnen wird.".into());
            }
        }
        let mut code = 1;
        // SAFETY: Live process handle and initialized DWORD output.
        if unsafe { GetExitCodeProcess(execute.hProcess, &mut code) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let record = ProfileStore::user_data()?.control_result()?;
        if record["schema"] != 1
            || record["nonce"].as_str() != Some(&nonce)
            || record["operation"].as_str() != Some(action.name())
        {
            return Err("Kein passendes Ergebnisprotokoll vorhanden. Steuerung ist nicht als erfolgreich bestätigt.".into());
        }
        if code != 0 || record["result"].get("Err").is_some() {
            return Err(format!(
                "Anwendung fehlgeschlagen: {}",
                record["result"]["Err"]
            ));
        }
        if record["result"].get("Ok").is_none() {
            return Err("Ergebnis enthält keinen verifizierten Abschluss.".into());
        }
        Ok(match action { Action::Restore => "Die aufgezeichneten Originalwerte wurden vollständig zurückgelesen und wiederhergestellt. Autostart-Auswahl ist ausgeschaltet.", Action::Apply => "Das gespeicherte Profil wurde angewendet und seine vollständigen Steuerwerte zurückgelesen.", Action::Startup => "Die geprüfte Profilversion wurde mit Administratorfreigabe für die nächste Windows-Anmeldung ausgewählt." }.into())
    })();
    // SAFETY: Closing the handle never kills a still-running hardware transaction.
    unsafe {
        CloseHandle(execute.hProcess);
    }
    result
}

fn worker(hwnd: usize, receiver: mpsc::Receiver<Request>, updates: mpsc::Sender<Update>) {
    let mut selected: Option<String> = None;
    while let Ok(request) = receiver.recv() {
        if matches!(request, Request::Stop) {
            break;
        }
        let result = (|| -> Result<(View, String), String> {
            let mut status = String::new();
            match request {
                Request::Load(id) => selected = id,
                Request::Save {
                    id,
                    name,
                    graphics,
                    memory,
                } => {
                    let store = ProfileStore::user_data()?;
                    let mut profile = if let Some(id) = id {
                        store.load(&id)?
                    } else {
                        let mut controller = Controller::new(true, None)?;
                        PerformanceProfile {
                            schema: PROFILE_SCHEMA,
                            id: format!("gpu-{}", token()),
                            name: name.clone(),
                            binding: controller.context()?.binding,
                            settings: vec![],
                        }
                    };
                    profile.name = name;
                    // Graphics curve and global core offset address the same domain.
                    // Saving a mode never resets the currently active hardware mode.
                    profile.settings.retain(|s| {
                        !matches!(
                            s.feature,
                            Feature::GpuCoreOffsetMhz
                                | Feature::GpuVfCurve
                                | Feature::GpuMemoryOffsetMhz
                        )
                    });
                    profile.settings.push(graphics);
                    profile.settings.push(Setting {
                        feature: Feature::GpuMemoryOffsetMhz,
                        value: Value::Integer(memory),
                    });
                    store.save(&profile)?;
                    selected = Some(profile.id);
                    status =
                        "Profil gespeichert. Die Hardware wurde dadurch nicht verändert.".into();
                }
                Request::Action { id, restore } => {
                    status = elevated(
                        id.as_deref(),
                        if restore {
                            Action::Restore
                        } else {
                            Action::Apply
                        },
                        &receiver,
                    )?
                }
                Request::Startup(id) => {
                    status = elevated(Some(&id), Action::Startup, &receiver)?;
                }
                Request::Disable => {
                    crate::profile_commands::run(&["profile-startup-disable".into()])?;
                    status = "Für die Windows-Anmeldung ist kein Profil mehr ausgewählt.".into();
                }
                Request::Stop => unreachable!(),
            }
            Ok((load(selected.as_deref())?, status))
        })();
        let update = match result {
            Ok((view, status)) => Update::View(Box::new(view), status),
            Err(e) => Update::Status(e),
        };
        if updates.send(update).is_err() {
            break;
        }
        // SAFETY: Opaque HWND is used only to notify our window, never dereferenced.
        unsafe {
            PostMessageW(hwnd as HWND, UPDATE, 0, 0);
        }
    }
}

#[derive(Clone, Copy)]
struct Controls {
    profiles: HWND,
    name: HWND,
    core: HWND,
    memory: HWND,
    curve: HWND,
    voltage: HWND,
    frequency: HWND,
    curve_hint: HWND,
    current: HWND,
    limits: HWND,
    status: HWND,
    save: HWND,
    apply: HWND,
    restore: HWND,
    startup: HWND,
    disable: HWND,
    new: HWND,
    refresh: HWND,
}
struct State {
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Update>,
    view: RefCell<Option<View>>,
    controls: Cell<Option<Controls>>,
    font: Cell<HFONT>,
    busy: Cell<bool>,
    rendering: Cell<bool>,
    dirty: Cell<bool>,
    curve_dirty: Cell<bool>,
}
pub(crate) fn text(handle: HWND, value: &str) {
    // SAFETY: Owned native control, terminated text lives through the call.
    unsafe {
        SetWindowTextW(handle, wide(value).as_ptr());
    }
}
pub(crate) fn input(handle: HWND) -> Result<String, String> {
    let mut buffer = [0_u16; 257];
    // SAFETY: Bounded writable UTF-16 storage; each EDIT also has a matching limit.
    let count = unsafe { GetWindowTextW(handle, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16(&buffer[..count as usize]).map_err(|e| e.to_string())
}
fn selected(state: &State) -> Option<String> {
    state
        .view
        .borrow()
        .as_ref()?
        .selected
        .as_ref()
        .map(|p| p.id.clone())
}
fn curve_mode(c: Controls) -> bool {
    // SAFETY: An owned, living native checkbox; no pointer parameters.
    unsafe { SendMessageW(c.curve, BM_GETCHECK, 0, 0) == BST_CHECKED as isize }
}
fn enabled(state: &State) {
    let Some(c) = state.controls.get() else {
        return;
    };
    let view = state.view.borrow();
    let ready = view.as_ref().is_some_and(|v| v.ready);
    let saved = view.as_ref().is_some_and(|v| v.selected.is_some());
    let applicable = view.as_ref().is_some_and(|v| v.applicable);
    // SAFETY: All handles belong to the living editor, UI thread only.
    unsafe {
        for handle in [c.profiles, c.new, c.refresh, c.restore, c.disable] {
            EnableWindow(handle, (!state.busy.get()) as i32);
        }
        for handle in [c.name, c.memory, c.save] {
            EnableWindow(handle, (ready && !state.busy.get()) as i32);
        }
        let curve = curve_mode(c);
        let core_ready = view.as_ref().is_some_and(|v| v.core_ready);
        let curve_ready = view.as_ref().is_some_and(|v| v.curve_ready);
        EnableWindow(c.core, (core_ready && !curve && !state.busy.get()) as i32);
        EnableWindow(c.curve, (curve_ready && !state.busy.get()) as i32);
        for handle in [c.voltage, c.frequency] {
            EnableWindow(handle, (curve_ready && curve && !state.busy.get()) as i32);
        }
        EnableWindow(
            c.apply,
            (applicable && saved && !state.dirty.get() && !state.busy.get()) as i32,
        );
        EnableWindow(
            c.startup,
            (applicable && saved && !state.dirty.get() && !state.busy.get()) as i32,
        );
    }
}
fn dispatch(state: &State, request: Request, status: &str) {
    if state.busy.replace(true) {
        return;
    }
    if let Some(c) = state.controls.get() {
        text(c.status, status);
    }
    enabled(state);
    if state.tx.send(request).is_err() {
        state.busy.set(false);
        if let Some(c) = state.controls.get() {
            text(
                c.status,
                "Der Steuerprozess ist nicht verfügbar. Fenster neu öffnen.",
            );
        }
        enabled(state);
    }
}
fn show(state: &State, view: View, status: &str) {
    let Some(c) = state.controls.get() else {
        return;
    };
    state.rendering.set(true);
    // SAFETY: Native combo, owned strings are copied by Windows.
    unsafe {
        SendMessageW(c.profiles, CB_RESETCONTENT, 0, 0);
        SendMessageW(
            c.profiles,
            CB_ADDSTRING,
            0,
            wide("Neues Profil …").as_ptr() as isize,
        );
        for p in &view.profiles {
            SendMessageW(c.profiles, CB_ADDSTRING, 0, wide(&p.name).as_ptr() as isize);
        }
        let index = view
            .selected
            .as_ref()
            .and_then(|p| {
                view.profiles
                    .iter()
                    .position(|candidate| candidate.id == p.id)
            })
            .map_or(0, |i| i + 1);
        SendMessageW(c.profiles, CB_SETCURSEL, index, 0);
    }
    let actual = [
        view.gpu.core.as_ref().ok().map(|s| s.current_mhz),
        view.gpu.memory.as_ref().ok().map(|s| s.current_mhz),
    ];
    text(
        c.current,
        &format!(
            "{} · Treiber {}\r\nAktuell: GPU {} MHz / VRAM {} MHz Offset · Temperatur {}",
            view.gpu.identity.name,
            view.gpu.identity.driver,
            actual[0].map_or("nicht verfügbar".into(), |v| v.to_string()),
            actual[1].map_or("nicht verfügbar".into(), |v| v.to_string()),
            view.gpu
                .temperature_c
                .map_or("nicht verfügbar".into(), |t| format!("{t} °C"))
        ),
    );
    text(
        c.name,
        view.selected
            .as_ref()
            .map_or("Mein GPU-Profil", |p| p.name.as_str()),
    );
    for (index, feature, handle) in [
        (0, Feature::GpuCoreOffsetMhz, c.core),
        (1, Feature::GpuMemoryOffsetMhz, c.memory),
    ] {
        let value = view
            .selected
            .as_ref()
            .and_then(|p| p.settings.iter().find(|s| s.feature == feature))
            .and_then(|s| {
                if let Value::Integer(v) = s.value {
                    Some(v)
                } else {
                    None
                }
            })
            .or(actual[index].map(i64::from));
        text(handle, &value.map_or(String::new(), |v| v.to_string()));
    }
    let stored_curve = view.selected.as_ref().and_then(|p| {
        p.settings.iter().find_map(|s| {
            if s.feature == Feature::GpuVfCurve {
                Some(&s.value)
            } else {
                None
            }
        })
    });
    let use_curve =
        stored_curve.is_some() || (view.selected.is_none() && !view.core_ready && view.curve_ready);
    // SAFETY: Our native controls copy strings/data and store no Rust references.
    unsafe {
        SendMessageW(
            c.curve,
            BM_SETCHECK,
            if use_curve {
                BST_CHECKED
            } else {
                BST_UNCHECKED
            } as usize,
            0,
        );
        SendMessageW(c.voltage, CB_RESETCONTENT, 0, 0);
    }
    let displayed_curve = stored_curve
        .cloned()
        .or_else(|| view.curve.as_ref().ok()?.value().ok());
    if let (Ok(current), Some(Value::VfCurve(points))) = (&view.curve, displayed_curve) {
        let last = points.last().expect("validated nonempty curve");
        let anchor = points
            .iter()
            .rposition(|p| p.frequency_mhz != last.frequency_mhz)
            .map_or(0, |i| i + 1);
        let anchor_mv = points[anchor].voltage_mv;
        text(c.frequency, &last.frequency_mhz.to_string());
        for node in &current.nodes {
            let mv = node.voltage_uv / 1000;
            unsafe {
                let index = SendMessageW(
                    c.voltage,
                    CB_ADDSTRING,
                    0,
                    wide(&format!("{mv} mV")).as_ptr() as isize,
                );
                SendMessageW(c.voltage, CB_SETITEMDATA, index as usize, mv as isize);
                if mv == anchor_mv {
                    SendMessageW(c.voltage, CB_SETCURSEL, index as usize, 0);
                }
            }
        }
        text(
            c.curve_hint,
            &format!(
                "{} vollständige Kurvenpunkte; ein neuer Anker erzeugt ein Plateau.\r\nFrequenzziel ab Anker, kein hartes Spannungslimit. Laststabilität ungeprüft.",
                points.len()
            ),
        );
    } else {
        text(c.frequency, "");
        text(
            c.curve_hint,
            view.curve
                .as_ref()
                .err()
                .map_or("V/F-Steuerung nicht verfügbar.", String::as_str),
        );
    }
    let ranges = if view.ready {
        format!(
            "Treiberspannen: GPU {} bis {} / VRAM {} bis {} MHz.\r\nDiese Grenzen bestätigen keine Stabilität. VRAM-Werte sind NVML-MHz.",
            view.ranges[0].0, view.ranges[0].1, view.ranges[1].0, view.ranges[1].1
        )
    } else {
        "Taktsteuerung für diese Kombination nicht freigegeben.\r\nKeine verifizierten Schreibgrenzen verfügbar.".into()
    };
    text(c.limits, &format!("{ranges}\r\n{}", view.startup));
    text(
        c.status,
        if !status.is_empty() {
            status
        } else if !view.blockers.is_empty() {
            &view.blockers
        } else if view.ready {
            "Speichern legt ein Profil an. Anwenden ändert die GPU nach Windows-Freigabe."
        } else {
            "GPU-Taktsteuerung ist für diese Hardware-/Treiberkombination nicht validiert."
        },
    );
    *state.view.borrow_mut() = Some(view);
    state.dirty.set(false);
    state.curve_dirty.set(false);
    state.rendering.set(false);
    state.busy.set(false);
    enabled(state);
}
fn save_request(state: &State) -> Result<Request, String> {
    let c = state.controls.get().ok_or("Fenster wird noch geladen.")?;
    let name = input(c.name)?.trim().to_owned();
    if name.is_empty() {
        return Err("Bitte einen Profilnamen eingeben.".into());
    }
    let memory = input(c.memory)?
        .trim()
        .parse::<i64>()
        .map_err(|_| "VRAM-Offset als ganze MHz-Zahl eingeben, z. B. -24.")?;
    let view = state.view.borrow();
    let view = view.as_ref().ok_or("GPU noch nicht gelesen.")?;
    if !(view.ranges[1].0..=view.ranges[1].1).contains(&memory) {
        return Err("Der VRAM-Offset liegt außerhalb der gelesenen Treiberspanne.".into());
    }
    let graphics = if curve_mode(c) {
        if !view.curve_ready {
            return Err("V/F-Steuerung ist für diese Kombination nicht freigegeben.".into());
        }
        let current = view.curve.as_ref().map_err(Clone::clone)?;
        let preserved = if !state.curve_dirty.get() {
            view.selected
                .as_ref()
                .and_then(|p| p.settings.iter().find(|s| s.feature == Feature::GpuVfCurve))
                .map(|s| s.value.clone())
                .or_else(|| {
                    if view.selected.is_none() {
                        current.value().ok()
                    } else {
                        None
                    }
                })
        } else {
            None
        };
        let value = if let Some(value) = preserved {
            value
        } else {
            // SAFETY: Integer item data belongs to the voltage combo; no pointers.
            let index = unsafe { SendMessageW(c.voltage, CB_GETCURSEL, 0, 0) };
            if index < 0 {
                return Err("Einen vorhandenen Spannungsanker auswählen.".into());
            }
            let voltage = unsafe { SendMessageW(c.voltage, CB_GETITEMDATA, index as usize, 0) };
            let voltage = u32::try_from(voltage).map_err(|_| "Ungültiger Spannungsanker.")?;
            let frequency = input(c.frequency)?
                .trim()
                .parse::<u32>()
                .map_err(|_| "Frequenzziel als ganze MHz-Zahl eingeben.")?;
            vf::plateau(current, voltage, frequency)
                .map_err(|e| format!("Kurvenziel nicht gültig: {e}"))?
        };
        let Value::VfCurve(points) = &value else {
            return Err("Gespeicherte Kurve ist ungültig.".into());
        };
        current.target_deltas(points)?;
        Setting {
            feature: Feature::GpuVfCurve,
            value,
        }
    } else {
        if !view.core_ready {
            return Err("Eine individuelle V/F-Kurve ist aktiv. Original wiederherstellen oder ein vollständiges Kurvenprofil verwenden.".into());
        }
        let core = input(c.core)?
            .trim()
            .parse::<i64>()
            .map_err(|_| "GPU-Offset als ganze MHz-Zahl eingeben, z. B. -15.")?;
        if !(view.ranges[0].0..=view.ranges[0].1).contains(&core) {
            return Err("Der GPU-Offset liegt außerhalb der gelesenen Treiberspanne.".into());
        }
        Setting {
            feature: Feature::GpuCoreOffsetMhz,
            value: Value::Integer(core),
        }
    };
    Ok(Request::Save {
        id: selected(state),
        name,
        graphics,
        memory,
    })
}

pub(crate) fn control(
    hwnd: HWND,
    font: HFONT,
    id: usize,
    class: &str,
    label: &str,
    style: u32,
    rect: [i32; 4],
) -> HWND {
    // SAFETY: Children and text belong to the editor; Windows copies creation text.
    unsafe {
        let handle = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(label).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            rect[0],
            rect[1],
            rect[2],
            rect[3],
            hwnd,
            id as HMENU,
            GetModuleHandleW(ptr::null()),
            ptr::null(),
        );
        SendMessageW(handle, WM_SETFONT, font as usize, 1);
        handle
    }
}
unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Box<State> lives through the message loop; no pointer crosses threads.
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
        if pointer.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let state = &*pointer;
        match message {
            WM_CREATE => {
                let mut font: LOGFONTW = std::mem::zeroed();
                font.lfHeight = -18;
                for (target, value) in font.lfFaceName.iter_mut().zip(wide("Segoe UI")) {
                    *target = value;
                }
                state.font.set(CreateFontIndirectW(&font));
                let f = state.font.get();
                control(
                    hwnd,
                    f,
                    0,
                    "STATIC",
                    "GPU & eigene Profile",
                    0,
                    [20, 18, 720, 28],
                );
                let current = control(
                    hwnd,
                    f,
                    0,
                    "STATIC",
                    "GPU und Treiber werden gelesen …",
                    0,
                    [20, 55, 720, 54],
                );
                control(
                    hwnd,
                    f,
                    0,
                    "STATIC",
                    "Gespeichertes &Profil",
                    0,
                    [20, 122, 200, 28],
                );
                let profiles = control(
                    hwnd,
                    f,
                    SELECT,
                    "COMBOBOX",
                    "",
                    WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                    [225, 119, 350, 240],
                );
                let new = control(
                    hwnd,
                    f,
                    NEW,
                    "BUTTON",
                    "&Neu",
                    WS_TABSTOP,
                    [590, 119, 140, 32],
                );
                control(hwnd, f, 0, "STATIC", "Profil&name", 0, [20, 165, 200, 28]);
                let name = control(
                    hwnd,
                    f,
                    NAME,
                    "EDIT",
                    "",
                    WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                    [225, 162, 505, 30],
                );
                SendMessageW(name, EM_SETLIMITTEXT, 100, 0);
                control(
                    hwnd,
                    f,
                    0,
                    "STATIC",
                    "&GPU-Offset (MHz)",
                    0,
                    [20, 212, 200, 28],
                );
                let core = control(
                    hwnd,
                    f,
                    CORE,
                    "EDIT",
                    "",
                    WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                    [225, 209, 150, 30],
                );
                control(
                    hwnd,
                    f,
                    0,
                    "STATIC",
                    "&VRAM-Offset (MHz)",
                    0,
                    [405, 212, 175, 28],
                );
                let memory = control(
                    hwnd,
                    f,
                    MEMORY,
                    "EDIT",
                    "",
                    WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                    [590, 209, 140, 30],
                );
                SendMessageW(core, EM_SETLIMITTEXT, 12, 0);
                SendMessageW(memory, EM_SETLIMITTEXT, 12, 0);
                let limits = control(hwnd, f, 0, "STATIC", "", 0, [20, 257, 710, 82]);
                let curve = control(
                    hwnd,
                    f,
                    CURVE,
                    "BUTTON",
                    "V/F-&Kurve verwenden (Undervolting)",
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                    [20, 345, 710, 28],
                );
                control(
                    hwnd,
                    f,
                    0,
                    "STATIC",
                    "Spannungsanker (mV)",
                    0,
                    [20, 383, 200, 28],
                );
                let voltage = control(
                    hwnd,
                    f,
                    VOLTAGE,
                    "COMBOBOX",
                    "",
                    WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                    [225, 380, 150, 230],
                );
                control(
                    hwnd,
                    f,
                    0,
                    "STATIC",
                    "Frequenzziel (MHz)",
                    0,
                    [405, 383, 175, 28],
                );
                let frequency = control(
                    hwnd,
                    f,
                    FREQUENCY,
                    "EDIT",
                    "",
                    WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                    [590, 380, 140, 30],
                );
                SendMessageW(frequency, EM_SETLIMITTEXT, 12, 0);
                let curve_hint = control(
                    hwnd,
                    f,
                    0,
                    "STATIC",
                    "Kurve wird gelesen …",
                    0,
                    [20, 418, 710, 44],
                );
                control(
                    hwnd,
                    f,
                    0,
                    "STATIC",
                    "Unleashed: vollständiger Firmware-Readback und Restore fehlen.\r\nVRAM-Spannung: kein verifizierter Steuervertrag auf dieser Hardware.",
                    0,
                    [20, 466, 710, 44],
                );
                let save = control(
                    hwnd,
                    f,
                    SAVE,
                    "BUTTON",
                    "&Speichern",
                    WS_TABSTOP,
                    [20, 514, 160, 36],
                );
                let apply = control(
                    hwnd,
                    f,
                    APPLY,
                    "BUTTON",
                    "&Anwenden …",
                    WS_TABSTOP,
                    [196, 514, 160, 36],
                );
                let restore = control(
                    hwnd,
                    f,
                    RESTORE,
                    "BUTTON",
                    "&Original wiederherstellen …",
                    WS_TABSTOP,
                    [372, 514, 358, 36],
                );
                let startup = control(
                    hwnd,
                    f,
                    STARTUP,
                    "BUTTON",
                    "Nach Anmeldung aktivieren",
                    WS_TABSTOP,
                    [20, 565, 310, 36],
                );
                let disable = control(
                    hwnd,
                    f,
                    DISABLE,
                    "BUTTON",
                    "Auswahl ausschalten",
                    WS_TABSTOP,
                    [346, 565, 236, 36],
                );
                let refresh = control(
                    hwnd,
                    f,
                    REFRESH,
                    "BUTTON",
                    "Aktualisieren",
                    WS_TABSTOP,
                    [598, 565, 132, 36],
                );
                let status = control(
                    hwnd,
                    f,
                    0,
                    "EDIT",
                    "Wird gelesen …",
                    WS_VSCROLL
                        | WS_BORDER
                        | ES_MULTILINE as u32
                        | ES_READONLY as u32
                        | ES_AUTOVSCROLL as u32,
                    [20, 622, 710, 78],
                );
                control(
                    hwnd,
                    f,
                    CURVE_EDITOR,
                    "BUTTON",
                    "GPU-V/F- und Lüfterkurven bearbeiten …",
                    WS_TABSTOP,
                    [20, 712, 710, 36],
                );
                if [
                    profiles, name, core, memory, curve, voltage, frequency, curve_hint, current,
                    limits, status, save, apply, restore, startup, disable, new, refresh,
                ]
                .iter()
                .any(|h| h.is_null())
                {
                    return -1;
                }
                state.controls.set(Some(Controls {
                    profiles,
                    name,
                    core,
                    memory,
                    curve,
                    voltage,
                    frequency,
                    curve_hint,
                    current,
                    limits,
                    status,
                    save,
                    apply,
                    restore,
                    startup,
                    disable,
                    new,
                    refresh,
                }));
                enabled(state);
                0
            }
            UPDATE => {
                while let Ok(update) = state.rx.try_recv() {
                    match update {
                        Update::View(view, status) => show(state, *view, &status),
                        Update::Status(error) => {
                            state.busy.set(false);
                            if let Some(c) = state.controls.get() {
                                text(c.status, &error);
                            }
                            enabled(state);
                        }
                    }
                }
                0
            }
            WM_COMMAND => {
                let id = wparam & 0xffff;
                let notification = (wparam >> 16) as u32;
                if state.rendering.get() {
                    return 0;
                }
                if (matches!(id, NAME | CORE | MEMORY | FREQUENCY) && notification == EN_CHANGE)
                    || (id == VOLTAGE && notification == CBN_SELCHANGE)
                    || (id == CURVE && notification == BN_CLICKED)
                {
                    state.dirty.set(true);
                    if matches!(id, CURVE | VOLTAGE | FREQUENCY) {
                        state.curve_dirty.set(true);
                    }
                    enabled(state);
                    if let Some(c) = state.controls.get() {
                        text(
                            c.status,
                            "Ungespeicherte Änderungen. Zum Anwenden zuerst speichern.",
                        );
                    }
                    return 0;
                }
                if matches!(id, CORE | MEMORY | NAME | FREQUENCY)
                    && notification == EN_KILLFOCUS
                    && state.dirty.get()
                {
                    if let Err(error) = save_request(state) {
                        if let Some(c) = state.controls.get() {
                            text(c.status, &error);
                        }
                    }
                    return 0;
                }
                if state.busy.get() {
                    return 0;
                }
                if matches!(id, APPLY | STARTUP)
                    && (state.dirty.get()
                        || !state.view.borrow().as_ref().is_some_and(|v| v.applicable))
                {
                    if let Some(c) = state.controls.get() {
                        text(
                            c.status,
                            "Zuerst ein vollständiges, unterstütztes Profil speichern und prüfen.",
                        );
                    }
                    return 0;
                }
                match id {
                    CURVE_EDITOR => {
                        let result = std::env::current_exe().and_then(|exe| {
                            let mut command = std::process::Command::new(exe);
                            command.arg("curve-ui");
                            if let Some(id) = selected(state) {
                                command.arg(id);
                            }
                            command.spawn()
                        });
                        if let Err(e) = result {
                            if let Some(c) = state.controls.get() {
                                text(
                                    c.status,
                                    &format!("Kurveneditor konnte nicht geöffnet werden: {e}"),
                                );
                            }
                        }
                    }
                    SELECT if notification == CBN_SELCHANGE => {
                        let c = state.controls.get().unwrap();
                        let index = SendMessageW(c.profiles, CB_GETCURSEL, 0, 0);
                        let id = state
                            .view
                            .borrow()
                            .as_ref()
                            .and_then(|v| {
                                usize::try_from(index - 1)
                                    .ok()
                                    .and_then(|i| v.profiles.get(i))
                            })
                            .map(|p| p.id.clone());
                        dispatch(state, Request::Load(id), "Profil wird gelesen …");
                    }
                    NEW => dispatch(
                        state,
                        Request::Load(None),
                        "Neues Profil wird vorbereitet …",
                    ),
                    REFRESH => dispatch(
                        state,
                        Request::Load(selected(state)),
                        "GPU und Profil werden erneut gelesen …",
                    ),
                    SAVE => match save_request(state) {
                        Ok(request) => dispatch(state, request, "Profil wird gespeichert …"),
                        Err(error) => {
                            if let Some(c) = state.controls.get() {
                                text(c.status, &error);
                            }
                        }
                    },
                    APPLY => dispatch(
                        state,
                        Request::Action {
                            id: selected(state),
                            restore: false,
                        },
                        "Windows-Freigabe, Geräteprüfung und Rücklesen …",
                    ),
                    RESTORE => dispatch(
                        state,
                        Request::Action {
                            id: None,
                            restore: true,
                        },
                        "Windows-Freigabe und Prüfung des Originalprotokolls …",
                    ),
                    STARTUP => {
                        if let Some(id) = selected(state) {
                            dispatch(
                                state,
                                Request::Startup(id),
                                "Verifizierte Profilversion und Anmeldungseintrag werden geprüft …",
                            );
                        }
                    }
                    DISABLE => dispatch(state, Request::Disable, "Auswahl wird ausgeschaltet …"),
                    _ => {}
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

pub fn run_selected(id: Option<String>) -> Result<(), String> {
    if let Some(id) = &id {
        omen_core::profiles::validate_id(id)?;
    }
    let (tx, requests) = mpsc::channel();
    let (updates, rx) = mpsc::channel();
    let state = Box::new(State {
        tx,
        rx,
        view: RefCell::new(None),
        controls: Cell::new(None),
        font: Cell::new(ptr::null_mut()),
        busy: Cell::new(false),
        rendering: Cell::new(false),
        dirty: Cell::new(false),
        curve_dirty: Cell::new(false),
    });
    // SAFETY: Class, state and font stay alive through every dispatched message.
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class = wide("OmenHubPPGpuProfiles");
        let mut registration: WNDCLASSW = std::mem::zeroed();
        registration.lpfnWndProc = Some(procedure);
        registration.hInstance = instance;
        registration.lpszClassName = class.as_ptr();
        registration.hCursor = LoadCursorW(ptr::null_mut(), IDC_ARROW);
        registration.hbrBackground = (COLOR_BTNFACE + 1) as HBRUSH;
        if RegisterClassW(&registration) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide("Omen Hub++ – GPU & eigene Profile").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            770,
            810,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            (&*state as *const State).cast(),
        );
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let handle = hwnd as usize;
        let worker = thread::spawn(move || worker(handle, requests, updates));
        ShowWindow(hwnd, SW_SHOW);
        dispatch(&state, Request::Load(id), "GPU wird gelesen …");
        let mut message: MSG = std::mem::zeroed();
        let result = loop {
            let result = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
            if result <= 0 {
                break result;
            }
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        };
        let _ = state.tx.send(Request::Stop);
        if worker.is_finished() {
            let _ = worker.join();
        }
        if !state.font.get().is_null() {
            DeleteObject(state.font.get());
        }
        if result < 0 {
            Err(std::io::Error::last_os_error().to_string())
        } else {
            Ok(())
        }
    }
}
