//! Four-zone native editor. Saving is metadata; reads/apply/restore use fixed UAC children.
use crate::{
    elevation::{self, Action},
    gpu_ui::{control, input, text, token, wide},
};
use omen_core::{
    features::Feature,
    keyboard::KeyboardState,
    profiles::{Choice, DeviceBinding, PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
    transaction::Backend,
};
use omen_platform::{Controller, RestoreState, profile_store::ProfileStore};
use std::{
    cell::{Cell, RefCell},
    ptr,
    sync::mpsc,
    thread,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::{
            Dialogs::{CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW},
            EM_SETLIMITTEXT,
        },
        Input::KeyboardAndMouse::{EnableWindow, GetKeyState},
        WindowsAndMessaging::*,
    },
};

const UPDATE: u32 = WM_APP + 51;
const SELECT: usize = 701;
const NAME: usize = 702;
const NEW: usize = 703;
const SAVE: usize = 704;
const READ: usize = 705;
const APPLY: usize = 706;
const RESTORE: usize = 707;
const STATUS: usize = 708;
const HEX: usize = 710;
const PICK: usize = 714;
const MODE: usize = 718;
const CURRENT: usize = 722;
const COPY_CURRENT: usize = 726;
#[derive(Clone)]
struct View {
    profiles: Vec<PerformanceProfile>,
    selected: Option<PerformanceProfile>,
    binding: DeviceBinding,
    pending: bool,
    restore_ready: bool,
}
fn view(id: Option<&str>) -> Result<View, String> {
    let store = ProfileStore::user_data()?;
    let selected = id.map(|id| store.load(id)).transpose()?;
    let mut c = Controller::new(false, None)?;
    let restore_ready = store.last_transaction::<RestoreState>().is_ok_and(|t| {
        selected.as_ref().is_some_and(|p| p.id == t.profile_id)
            && t.requested
                .iter()
                .any(|s| matches!(s.feature, Feature::KeyboardColors | Feature::KeyboardEffect))
    });
    Ok(View {
        profiles: store.list()?,
        selected,
        binding: c.context()?.binding,
        pending: store.pending()?,
        restore_ready,
    })
}
enum Request {
    Load(Option<String>),
    Read(Option<String>),
    Save {
        baseline: Box<Option<PerformanceProfile>>,
        binding: Box<DeviceBinding>,
        name: String,
        colors: [[u8; 3]; 4],
        mode: Option<Choice>,
    },
    Apply(String),
    Restore(String),
    Stop,
}
enum Update {
    View(Box<View>, String),
    Read(KeyboardState, bool, String),
    Error(String),
}
fn worker(hwnd: usize, rx: mpsc::Receiver<Request>, tx: mpsc::Sender<Update>) {
    while let Ok(request) = rx.recv() {
        if matches!(request, Request::Stop) {
            break;
        }
        let result = (|| -> Result<Update, String> {
            match request {
                Request::Load(id) => {
                    Ok(Update::View(Box::new(view(id.as_deref())?), String::new()))
                }
                Request::Read(id) => {
                    let record = elevation::run(id.as_deref(), Action::KeyboardRead, || {
                        matches!(
                            rx.try_recv(),
                            Ok(Request::Stop) | Err(mpsc::TryRecvError::Disconnected)
                        )
                    })?;
                    let result = &record["result"]["Ok"];
                    let state: KeyboardState = serde_json::from_value(result["state"].clone())
                        .map_err(|e| e.to_string())?;
                    state.validate()?;
                    let applicable = result["plan"]["applicable"].as_bool() == Some(true);
                    let status = if id.is_none() {
                        "Firmwarezustand gelesen. Farben übernehmen oder einen eigenen Entwurf speichern.".into()
                    } else if applicable {
                        "Firmware und gespeichertes Profil geprüft. Anwenden überträgt das gesamte Profil nach Windows-Freigabe.".into()
                    } else {
                        format!(
                            "Profil noch nicht anwendbar: {}",
                            result["plan"]["blockers"]
                        )
                    };
                    Ok(Update::Read(state, applicable, status))
                }
                Request::Save {
                    baseline,
                    binding,
                    name,
                    colors,
                    mode,
                } => {
                    let store = ProfileStore::user_data()?;
                    let mut profile = if let Some(old) = baseline.as_ref() {
                        let fresh = store.load(&old.id)?;
                        let managed = |p: &PerformanceProfile| {
                            p.settings
                                .iter()
                                .filter(|s| {
                                    matches!(
                                        s.feature,
                                        Feature::KeyboardColors | Feature::KeyboardEffect
                                    )
                                })
                                .cloned()
                                .collect::<Vec<_>>()
                        };
                        if managed(&fresh) != managed(old)
                            || (name != old.name && fresh.name != old.name)
                        {
                            return Err("Beleuchtungsprofil wurde in einem anderen Fenster geändert. Neu laden, bevor es überschrieben wird.".into());
                        }
                        fresh
                    } else {
                        PerformanceProfile {
                            schema: PROFILE_SCHEMA,
                            id: format!("lighting-{}", token()),
                            name: name.clone(),
                            binding: (*binding).clone(),
                            settings: vec![],
                        }
                    };
                    if baseline.as_ref().as_ref().is_none_or(|p| name != p.name) {
                        profile.name = name;
                    }
                    profile.settings.retain(|s| {
                        !matches!(s.feature, Feature::KeyboardColors | Feature::KeyboardEffect)
                    });
                    profile.settings.push(Setting {
                        feature: Feature::KeyboardColors,
                        value: Value::Colors(colors.to_vec()),
                    });
                    if let Some(mode) = mode {
                        profile.settings.push(Setting {
                            feature: Feature::KeyboardEffect,
                            value: Value::Choice(mode),
                        });
                    }
                    profile.binding = profile.binding.for_settings(&profile.settings);
                    store.save(&profile)?;
                    Ok(Update::View(Box::new(view(Some(&profile.id))?),"Profil gespeichert. Keine Hardware verändert. Firmware prüfen, bevor das Profil angewendet wird.".into()))
                }
                Request::Apply(id) => {
                    elevation::run(Some(&id), Action::Apply, || {
                        matches!(
                            rx.try_recv(),
                            Ok(Request::Stop) | Err(mpsc::TryRecvError::Disconnected)
                        )
                    })?;
                    Ok(Update::View(Box::new(view(Some(&id))?),"Gesamtes Profil angewendet; Farben, Modus und unveränderte Steuerbytes wurden zurückgelesen.".into()))
                }
                Request::Restore(id) => {
                    elevation::run(Some(&id), Action::Restore, || {
                        matches!(
                            rx.try_recv(),
                            Ok(Request::Stop) | Err(mpsc::TryRecvError::Disconnected)
                        )
                    })?;
                    Ok(Update::View(Box::new(view(Some(&id))?),"Aufgezeichneter Originalzustand vollständig wiederhergestellt. Anmeldungsauswahl ausgeschaltet.".into()))
                }
                Request::Stop => unreachable!(),
            }
        })();
        if tx.send(result.unwrap_or_else(Update::Error)).is_err() {
            break;
        }
        unsafe {
            PostMessageW(hwnd as HWND, UPDATE, 0, 0);
        }
    }
}
#[derive(Clone, Copy)]
struct Controls {
    profiles: HWND,
    name: HWND,
    new: HWND,
    save: HWND,
    read: HWND,
    apply: HWND,
    restore: HWND,
    hex: [HWND; 4],
    pick: [HWND; 4],
    swatch: [HWND; 4],
    current: [HWND; 4],
    mode: HWND,
    source: HWND,
    status: HWND,
    copy: HWND,
}
struct State {
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Update>,
    view: RefCell<Option<View>>,
    live: RefCell<Option<KeyboardState>>,
    controls: Cell<Option<Controls>>,
    font: Cell<HFONT>,
    busy: Cell<bool>,
    rendering: Cell<bool>,
    dirty: Cell<bool>,
    checked: Cell<bool>,
}
fn id(s: &State) -> Option<String> {
    s.view
        .borrow()
        .as_ref()?
        .selected
        .as_ref()
        .map(|p| p.id.clone())
}
fn colors(s: &State) -> Result<[[u8; 3]; 4], String> {
    let c = s.controls.get().ok_or("Fenster noch nicht bereit")?;
    let mut values = [[0; 3]; 4];
    for (i, field) in c.hex.iter().enumerate() {
        let value = input(*field)?;
        let value = value.trim().trim_start_matches('#');
        if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!(
                "Zone {}: Farbe mit sechs Hex-Zeichen eingeben, z. B. 3399FF.",
                i + 1
            ));
        }
        let n = u32::from_str_radix(value, 16).map_err(|e| e.to_string())?;
        values[i] = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
    }
    Ok(values)
}
fn hex(color: [u8; 3]) -> String {
    format!("{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}
fn enabled(s: &State) {
    let Some(c) = s.controls.get() else { return };
    let ready = s.view.borrow().is_some() && !s.busy.get();
    unsafe {
        for h in [c.profiles, c.name, c.new, c.save, c.read, c.mode] {
            EnableWindow(h, ready as i32);
        }
        for h in c.hex.into_iter().chain(c.pick) {
            EnableWindow(h, ready as i32);
        }
        let v = s.view.borrow();
        let blocked = v.as_ref().is_none_or(|v| v.pending);
        EnableWindow(
            c.apply,
            (ready && !blocked && !s.dirty.get() && s.checked.get() && id(s).is_some()) as i32,
        );
        EnableWindow(
            c.restore,
            (ready && v.as_ref().is_some_and(|v| v.restore_ready)) as i32,
        );
        EnableWindow(c.copy, (ready && s.live.borrow().is_some()) as i32);
    }
}
fn dirty(s: &State) {
    if s.rendering.get() {
        return;
    }
    s.dirty.set(true);
    s.checked.set(false);
    if let Some(c) = s.controls.get() {
        text(
            c.status,
            "Ungespeicherter Entwurf. Speichern verändert keine Hardware.",
        );
        unsafe {
            for h in c.swatch {
                InvalidateRect(h, ptr::null(), 1);
            }
        }
    }
    enabled(s);
}
fn invalidate_live(s: &State) {
    *s.live.borrow_mut() = None;
    s.checked.set(false);
    if let Some(c) = s.controls.get() {
        for field in c.current {
            text(field, "Firmware: nicht gelesen");
        }
        text(
            c.source,
            "Firmwarezustand erneut prüfen, um aktuelle Farben zu lesen.",
        );
    }
}
fn dispatch(s: &State, request: Request, status: &str) {
    if s.busy.replace(true) {
        return;
    }
    // A child may write before returning an error. Drop the old GET snapshot
    // before dispatch, including when UAC is cancelled or the child times out.
    if matches!(
        &request,
        Request::Read(_) | Request::Apply(_) | Request::Restore(_)
    ) {
        invalidate_live(s);
    }
    if let Some(c) = s.controls.get() {
        text(c.status, status);
    }
    enabled(s);
    if s.tx.send(request).is_err() {
        s.busy.set(false);
        if let Some(c) = s.controls.get() {
            text(c.status, "Steuerung nicht erreichbar. Fenster neu öffnen.");
        }
        enabled(s);
    }
}
fn show(s: &State, v: View, status: &str) {
    let Some(c) = s.controls.get() else { return };
    invalidate_live(s);
    s.rendering.set(true);
    unsafe {
        SendMessageW(c.profiles, CB_RESETCONTENT, 0, 0);
        SendMessageW(
            c.profiles,
            CB_ADDSTRING,
            0,
            wide("Neues Beleuchtungsprofil …").as_ptr() as isize,
        );
        for p in &v.profiles {
            SendMessageW(c.profiles, CB_ADDSTRING, 0, wide(&p.name).as_ptr() as isize);
        }
        let selected = v
            .selected
            .as_ref()
            .and_then(|p| v.profiles.iter().position(|q| q.id == p.id))
            .map_or(0, |i| i + 1);
        SendMessageW(c.profiles, CB_SETCURSEL, selected, 0);
    }
    text(
        c.name,
        v.selected
            .as_ref()
            .map_or("Meine Tastaturfarben", |p| &p.name),
    );
    let color_value = v.selected.as_ref().and_then(|p| {
        p.settings
            .iter()
            .find(|s| s.feature == Feature::KeyboardColors)
    });
    let values = match color_value {
        Some(Setting {
            value: Value::Colors(colors),
            ..
        }) if colors.len() == 4 => colors.clone(),
        _ => vec![[255; 3]; 4],
    };
    for (field, value) in c.hex.iter().zip(values) {
        text(*field, &hex(value));
    }
    let mode = v.selected.as_ref().and_then(|p| {
        p.settings
            .iter()
            .find(|s| s.feature == Feature::KeyboardEffect)
    });
    let selected = match mode {
        Some(Setting {
            value: Value::Choice(Choice::FirmwareDefault),
            ..
        }) => 1,
        None if v.selected.is_some() => 2,
        _ => 0,
    };
    unsafe {
        SendMessageW(c.mode, CB_SETCURSEL, selected, 0);
    }
    text(
        c.source,
        "Vier Zonen · Farben sind Entwürfe oder Profilwerte. Firmware prüfen liest den tatsächlichen Zustand mit Administratorfreigabe.",
    );
    *s.view.borrow_mut() = Some(v);
    s.busy.set(false);
    s.dirty.set(false);
    s.checked.set(false);
    s.rendering.set(false);
    enabled(s);
    text(
        c.status,
        if status.is_empty() {
            "Farben speichern, Firmware prüfen, dann das gesamte Profil anwenden. Animationen und automatische Beleuchtungs-Wiederanwendung sind noch nicht freigegeben."
        } else {
            status
        },
    );
}
fn live(s: &State, state: KeyboardState, checked: bool, status: &str) {
    let Some(c) = s.controls.get() else { return };
    if let Ok(colors) = state.colors() {
        for (h, v) in c.current.iter().zip(colors) {
            text(*h, &format!("Firmware: #{}", hex(v)));
        }
    }
    text(
        c.source,
        if state.custom_colors_enabled().unwrap_or(false) {
            "Firmware: Eigene Farben aktiv. Der Modusschalter erhält die bekannten Steuerzustände; eine Helligkeitsskala wird hier nicht angeboten."
        } else {
            "Firmware: Voreinstellung aktiv. Eigene Farben im Profil auswählen, damit die gespeicherten Zonenfarben verwendet werden."
        },
    );
    *s.live.borrow_mut() = Some(state);
    s.checked.set(checked);
    s.busy.set(false);
    text(c.status, status);
    enabled(s);
}
fn save(s: &State) {
    let Some(c) = s.controls.get() else { return };
    let request = (|| -> Result<Request, String> {
        let colors = colors(s)?;
        let name = input(c.name)?.trim().to_owned();
        if name.is_empty() {
            return Err("Profilnamen eingeben.".into());
        }
        let selected = unsafe { SendMessageW(c.mode, CB_GETCURSEL, 0, 0) };
        let mode = match selected {
            0 => Some(Choice::Static),
            1 => Some(Choice::FirmwareDefault),
            2 => None,
            _ => return Err("Beleuchtungsmodus auswählen.".into()),
        };
        let v = s.view.borrow();
        let v = v.as_ref().ok_or("Profilansicht fehlt")?;
        Ok(Request::Save {
            baseline: Box::new(v.selected.clone()),
            binding: Box::new(v.binding.clone()),
            name,
            colors,
            mode,
        })
    })();
    match request {
        Ok(r) => dispatch(s, r, "Profil wird gespeichert; keine Hardwareänderung …"),
        Err(e) => text(c.status, &e),
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
        if pointer.is_null() {
            return DefWindowProcW(hwnd, message, w, l);
        }
        let s = &*pointer;
        match message {
            WM_CREATE => {
                let mut f: LOGFONTW = std::mem::zeroed();
                f.lfHeight = -18;
                for (a, b) in f.lfFaceName.iter_mut().zip(wide("Segoe UI")) {
                    *a = b;
                }
                s.font.set(CreateFontIndirectW(&f));
                let child = |id: usize, class: &str, label: &str, style: u32, rect: [i32; 4]| {
                    control(hwnd, s.font.get(), id, class, label, style, rect)
                };
                child(
                    0,
                    "STATIC",
                    "Tastaturbeleuchtung · vier Zonen",
                    0,
                    [20, 18, 720, 28],
                );
                child(
                    0,
                    "STATIC",
                    "Speichern hält einen Entwurf fest. Anwenden verändert die Beleuchtungsfirmware.",
                    0,
                    [20, 53, 720, 28],
                );
                child(0, "STATIC", "Profil", 0, [20, 82, 285, 20]);
                child(0, "STATIC", "Profilname", 0, [321, 82, 315, 20]);
                let profiles = child(
                    SELECT,
                    "COMBOBOX",
                    "",
                    WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                    [20, 106, 285, 240],
                );
                let name = child(
                    NAME,
                    "EDIT",
                    "",
                    WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                    [321, 106, 315, 32],
                );
                SendMessageW(name, EM_SETLIMITTEXT, 96, 0);
                let new = child(NEW, "BUTTON", "&Neu", WS_TABSTOP, [652, 106, 88, 32]);
                let source = child(
                    0,
                    "STATIC",
                    "Profile werden geladen …",
                    0,
                    [20, 148, 720, 52],
                );
                let hex = std::array::from_fn(|i| {
                    child(
                        0,
                        "STATIC",
                        &format!("Zone {}", i + 1),
                        0,
                        [20, 211 + i as i32 * 58, 86, 26],
                    );
                    let h = child(
                        HEX + i,
                        "EDIT",
                        "FFFFFF",
                        WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                        [125, 206 + i as i32 * 58, 120, 32],
                    );
                    SendMessageW(h, EM_SETLIMITTEXT, 7, 0);
                    h
                });
                let swatch = std::array::from_fn(|i| {
                    child(
                        740 + i,
                        "BUTTON",
                        "",
                        BS_OWNERDRAW as u32,
                        [270, 206 + i as i32 * 58, 78, 32],
                    )
                });
                let pick = std::array::from_fn(|i| {
                    child(
                        PICK + i,
                        "BUTTON",
                        "Farbe wählen …",
                        WS_TABSTOP,
                        [372, 206 + i as i32 * 58, 164, 32],
                    )
                });
                let current = std::array::from_fn(|i| {
                    child(
                        CURRENT + i,
                        "STATIC",
                        "Firmware: nicht gelesen",
                        0,
                        [554, 211 + i as i32 * 58, 186, 26],
                    )
                });
                child(0, "STATIC", "Anzeige", 0, [20, 452, 90, 26]);
                let mode = child(
                    MODE,
                    "COMBOBOX",
                    "",
                    WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                    [125, 448, 615, 180],
                );
                for value in [
                    "Eigene Farben aktivieren",
                    "Firmware-Voreinstellung verwenden",
                    "Aktivierung unverändert lassen",
                ] {
                    SendMessageW(mode, CB_ADDSTRING, 0, wide(value).as_ptr() as isize);
                }
                let read = child(
                    READ,
                    "BUTTON",
                    "Firmware prüfen …",
                    WS_TABSTOP,
                    [20, 499, 215, 34],
                );
                let copy = child(
                    COPY_CURRENT,
                    "BUTTON",
                    "Firmwarefarben übernehmen",
                    WS_TABSTOP,
                    [250, 499, 280, 34],
                );
                let save = child(
                    SAVE,
                    "BUTTON",
                    "&Speichern",
                    WS_TABSTOP,
                    [552, 499, 188, 34],
                );
                let apply = child(
                    APPLY,
                    "BUTTON",
                    "Gesamtes Profil &anwenden …",
                    WS_TABSTOP,
                    [20, 552, 348, 38],
                );
                let restore = child(
                    RESTORE,
                    "BUTTON",
                    "Original &wiederherstellen …",
                    WS_TABSTOP,
                    [389, 552, 351, 38],
                );
                let status = child(
                    STATUS,
                    "EDIT",
                    "Wird geladen …",
                    WS_BORDER
                        | ES_MULTILINE as u32
                        | ES_READONLY as u32
                        | ES_AUTOVSCROLL as u32
                        | WS_VSCROLL,
                    [20, 610, 720, 104],
                );
                let c = Controls {
                    profiles,
                    name,
                    new,
                    save,
                    read,
                    apply,
                    restore,
                    hex,
                    pick,
                    swatch,
                    current,
                    mode,
                    source,
                    status,
                    copy,
                };
                if [
                    profiles, name, source, mode, read, status, save, apply, restore,
                ]
                .iter()
                .chain(hex.iter())
                .any(|h| h.is_null())
                {
                    return -1;
                }
                s.controls.set(Some(c));
                enabled(s);
                0
            }
            WM_DRAWITEM => {
                let item = &*(l as *const windows_sys::Win32::UI::Controls::DRAWITEMSTRUCT);
                if (740..744).contains(&item.CtlID) {
                    let i = (item.CtlID - 740) as usize;
                    let value = colors(s).map_or([128; 3], |v| v[i]);
                    let color = u32::from(value[0])
                        | (u32::from(value[1]) << 8)
                        | (u32::from(value[2]) << 16);
                    let fill = CreateSolidBrush(color);
                    FillRect(item.hDC, &item.rcItem, fill);
                    DeleteObject(fill);
                    FrameRect(item.hDC, &item.rcItem, GetSysColorBrush(COLOR_WINDOWTEXT));
                    1
                } else {
                    0
                }
            }
            UPDATE => {
                while let Ok(update) = s.rx.try_recv() {
                    match update {
                        Update::View(v, status) => show(s, *v, &status),
                        Update::Read(state, checked, status) => live(s, state, checked, &status),
                        Update::Error(e) => {
                            s.busy.set(false);
                            s.checked.set(false);
                            if let Some(c) = s.controls.get() {
                                text(c.status, &e);
                            }
                            enabled(s);
                        }
                    }
                }
                0
            }
            WM_COMMAND => {
                if s.rendering.get() || s.busy.get() {
                    return 0;
                }
                let Some(c) = s.controls.get() else { return 0 };
                let code = (w >> 16) as u32;
                let action = w & 0xffff;
                if (action == NAME || (HEX..HEX + 4).contains(&action)) && code == EN_CHANGE {
                    dirty(s);
                    return 0;
                }
                if action == MODE && code == CBN_SELCHANGE {
                    dirty(s);
                    return 0;
                }
                match action {
                    SELECT if code == CBN_SELCHANGE => {
                        let i = SendMessageW(c.profiles, CB_GETCURSEL, 0, 0);
                        let selected = s
                            .view
                            .borrow()
                            .as_ref()
                            .and_then(|v| {
                                usize::try_from(i - 1).ok().and_then(|i| v.profiles.get(i))
                            })
                            .map(|p| p.id.clone());
                        dispatch(s, Request::Load(selected), "Profil wird geladen …");
                    }
                    NEW => dispatch(s, Request::Load(None), "Neuer Beleuchtungsentwurf …"),
                    SAVE => save(s),
                    READ => dispatch(
                        s,
                        Request::Read(id(s)),
                        "Windows-Freigabe für das Lesen und die Profilprüfung …",
                    ),
                    APPLY if !s.dirty.get() && s.checked.get() => {
                        if let Some(id) = id(s) {
                            dispatch(
                                s,
                                Request::Apply(id),
                                "Windows-Freigabe für das gesamte Profil …",
                            );
                        }
                    }
                    RESTORE if s.view.borrow().as_ref().is_some_and(|v| v.restore_ready) => {
                        if let Some(id) = id(s) {
                            dispatch(
                                s,
                                Request::Restore(id),
                                "Windows-Freigabe für die aufgezeichneten Originalwerte …",
                            );
                        }
                    }
                    COPY_CURRENT => {
                        if let Some(state) = s.live.borrow().as_ref() {
                            if let Ok(values) = state.colors() {
                                for (h, v) in c.hex.iter().zip(values) {
                                    text(*h, &hex(v));
                                }
                            }
                        }
                        dirty(s);
                    }
                    _ if (PICK..PICK + 4).contains(&action) => {
                        let i = action - PICK;
                        let color = colors(s).map_or([255; 3], |v| v[i]);
                        let mut custom = [0u32; 16];
                        let mut chooser: CHOOSECOLORW = std::mem::zeroed();
                        chooser.lStructSize = size_of::<CHOOSECOLORW>() as u32;
                        chooser.hwndOwner = hwnd;
                        chooser.rgbResult = u32::from(color[0])
                            | (u32::from(color[1]) << 8)
                            | (u32::from(color[2]) << 16);
                        chooser.lpCustColors = custom.as_mut_ptr();
                        chooser.Flags = CC_RGBINIT | CC_FULLOPEN;
                        if ChooseColorW(&mut chooser) != 0 {
                            let v = chooser.rgbResult;
                            text(c.hex[i], &hex([v as u8, (v >> 8) as u8, (v >> 16) as u8]));
                            dirty(s);
                        }
                    }
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
            _ => DefWindowProcW(hwnd, message, w, l),
        }
    }
}
pub fn run(selected: Option<String>) -> Result<(), String> {
    run_window(selected, None)
}
pub fn preview(selected: String, path: &std::path::Path) -> Result<(), String> {
    run_window(Some(selected), Some(path.into()))
}
fn run_window(selected: Option<String>, preview: Option<std::path::PathBuf>) -> Result<(), String> {
    if let Some(id) = &selected {
        omen_core::profiles::validate_id(id)?;
    }
    let (tx, requests) = mpsc::channel();
    let (updates, rx) = mpsc::channel();
    let s = Box::new(State {
        tx,
        rx,
        view: RefCell::new(None),
        live: RefCell::new(None),
        controls: Cell::new(None),
        font: Cell::new(ptr::null_mut()),
        busy: Cell::new(false),
        rendering: Cell::new(false),
        dirty: Cell::new(false),
        checked: Cell::new(false),
    });
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class = wide("OmenHubPPKeyboardEditor");
        let mut reg: WNDCLASSW = std::mem::zeroed();
        reg.lpfnWndProc = Some(procedure);
        reg.hInstance = instance;
        reg.lpszClassName = class.as_ptr();
        reg.hCursor = LoadCursorW(ptr::null_mut(), IDC_ARROW);
        reg.hbrBackground = (COLOR_BTNFACE + 1) as HBRUSH;
        if RegisterClassW(&reg) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide("Omen Hub++ – Tastaturbeleuchtung").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            780,
            780,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            (&*s as *const State).cast(),
        );
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let handle = hwnd as usize;
        let thread = thread::spawn(move || worker(handle, requests, updates));
        ShowWindow(hwnd, if preview.is_some() { SW_HIDE } else { SW_SHOW });
        dispatch(&s, Request::Load(selected), "Profile werden geladen …");
        let mut message: MSG = std::mem::zeroed();
        let mut preview_error = None;
        let result = loop {
            let r = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
            if r <= 0 {
                break r;
            }
            if message.message == WM_KEYDOWN && message.wParam == 0x53 && GetKeyState(0x11) < 0 {
                save(&s);
                continue;
            }
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            if let Some(path) = &preview {
                if message.message == UPDATE && !s.busy.get() {
                    if s.view.borrow().is_some() {
                        if let Err(e) = crate::curve_widget::render_window(hwnd, path) {
                            preview_error = Some(e);
                        }
                    } else {
                        preview_error =
                            Some("Beleuchtungsansicht konnte nicht geladen werden.".into());
                    }
                    DestroyWindow(hwnd);
                    break 1;
                }
            }
        };
        let _ = s.tx.send(Request::Stop);
        if thread.is_finished() {
            let _ = thread.join();
        }
        if !s.font.get().is_null() {
            DeleteObject(s.font.get());
        }
        if let Some(e) = preview_error {
            Err(e)
        } else if result < 0 {
            Err(std::io::Error::last_os_error().to_string())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omen_core::keyboard::{decode_keyboard_capability, decode_legacy_lighting};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;

    #[test]
    fn native_firmware_snapshot_is_discarded_after_refresh_or_possible_write() {
        // A hidden window with real controls, synthetic firmware and no worker:
        // exercise completion/error transitions without UAC or hardware IO.
        let (tx, requests) = mpsc::channel();
        let (updates, rx) = mpsc::channel();
        let s = Box::new(State {
            tx,
            rx,
            view: RefCell::new(None),
            live: RefCell::new(None),
            controls: Cell::new(None),
            font: Cell::new(ptr::null_mut()),
            busy: Cell::new(false),
            rendering: Cell::new(false),
            dirty: Cell::new(false),
            checked: Cell::new(false),
        });
        let binding = DeviceBinding {
            manufacturer: "Test".into(),
            board: "test".into(),
            bios: "test".into(),
            cpu_names: vec![],
            nvidia_uuid: None,
            nvidia_driver: None,
            nvidia_vbios: None,
        };
        let v = View {
            profiles: vec![],
            selected: None,
            binding,
            pending: false,
            restore_ready: false,
        };
        let capability = decode_keyboard_capability(0, &[1, 0, 0, 0]).unwrap();
        let state = KeyboardState {
            board: "test".into(),
            bios: "test".into(),
            legacy: Some(
                decode_legacy_lighting(capability.kind, 0, &[0; 128], 0, &[0; 128]).unwrap(),
            ),
            capability,
        };
        unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let class = wide("OmenHubPPKeyboardSnapshotTest");
            let mut reg: WNDCLASSW = std::mem::zeroed();
            reg.lpfnWndProc = Some(procedure);
            reg.hInstance = instance;
            reg.lpszClassName = class.as_ptr();
            reg.hbrBackground = (COLOR_BTNFACE + 1) as HBRUSH;
            assert_ne!(RegisterClassW(&reg), 0);
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                wide("Snapshot test").as_ptr(),
                WS_OVERLAPPED,
                0,
                0,
                780,
                780,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                (&*s as *const State).cast(),
            );
            assert!(!hwnd.is_null());
            let c = s.controls.get().unwrap();
            let clear = || {
                assert!(s.live.borrow().is_none());
                assert!(!s.checked.get());
                assert_eq!(IsWindowEnabled(c.copy), 0);
                for h in c.current {
                    assert_eq!(input(h).unwrap(), "Firmware: nicht gelesen");
                }
            };
            show(&s, v.clone(), "Initial load");
            live(&s, state.clone(), true, "Verified GET");
            assert_ne!(IsWindowEnabled(c.copy), 0);
            show(&s, v, "Successful apply or restore refreshed the view");
            clear();
            for request in [
                Request::Read(None),
                Request::Apply("test".into()),
                Request::Restore("test".into()),
            ] {
                live(&s, state.clone(), true, "Verified GET");
                dispatch(&s, request, "Child starting");
                clear();
                assert!(requests.try_recv().is_ok());
                updates
                    .send(Update::Error("Child failed after possible write".into()))
                    .unwrap();
                SendMessageW(hwnd, UPDATE, 0, 0);
                clear();
                assert!(!s.busy.get());
            }
            DestroyWindow(hwnd);
            DeleteObject(s.font.get());
            UnregisterClassW(class.as_ptr(), instance);
        }
    }
}
