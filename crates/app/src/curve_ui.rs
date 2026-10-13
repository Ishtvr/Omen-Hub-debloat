//! Native, metadata-only workbench for V/F and independent CPU/GPU fan curves.
use crate::{
    curve_widget as plot,
    gpu_ui::{control, input, text, token, wide},
};
use omen_core::{
    curve_editor::{Curve, Kind, Point},
    features::Feature,
    profiles::{DeviceBinding, PROFILE_SCHEMA, PerformanceProfile, Setting},
    transaction::Backend,
};
use omen_platform::{Controller, profile_store::ProfileStore};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    ptr,
    sync::mpsc,
    thread,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::{BST_CHECKED, BST_UNCHECKED, EM_SETLIMITTEXT, EM_SETREADONLY},
        Input::KeyboardAndMouse::{EnableWindow, GetKeyState},
        WindowsAndMessaging::*,
    },
};

const UPDATE: u32 = WM_APP + 32;
const SELECT: usize = 501;
const NAME: usize = 502;
const SAVE: usize = 503;
const NEW: usize = 504;
const GPU: usize = 505;
const CPU_FAN: usize = 506;
const GPU_FAN: usize = 507;
const INCLUDE: usize = 508;
const POINT: usize = 509;
const X: usize = 510;
const Y: usize = 511;
const SET_POINT: usize = 512;
const ADD: usize = 513;
const REMOVE: usize = 514;
const PLATEAU: usize = 515;
const RESET: usize = 516;
const OPEN: usize = 517;
const RELOAD: usize = 518;
const CANVAS: usize = 519;
const STATUS: usize = 520;
const KINDS: [Kind; 3] = [Kind::Gpu, Kind::CpuFan, Kind::GpuFan];
#[derive(Clone)]
struct Row {
    curve: Option<Curve>,
    enabled: bool,
    source: String,
}
struct View {
    profiles: Vec<PerformanceProfile>,
    selected: Option<PerformanceProfile>,
    binding: DeviceBinding,
    rows: [Row; 3],
}
fn load(id: Option<&str>) -> Result<View, String> {
    let store = ProfileStore::user_data()?;
    let profiles = store.list()?;
    let selected = id.map(|id| store.load(id)).transpose()?;
    let mut basic = Controller::new(false, None)?;
    let mut binding = basic.context()?.binding;
    let gpu = (|| {
        let mut driver = Controller::new(
            true,
            selected
                .as_ref()
                .and_then(|p| p.binding.nvidia_uuid.as_deref()),
        )?;
        let state = driver.gpu_curve()?;
        binding = driver.context()?.binding;
        let value = selected
            .as_ref()
            .and_then(|p| p.settings.iter().find(|s| s.feature == Feature::GpuVfCurve))
            .map(|s| &s.value);
        Curve::gpu(state, value)
    })();
    let rows = std::array::from_fn(|i| {
        let kind = KINDS[i];
        let setting = selected
            .as_ref()
            .and_then(|p| p.settings.iter().find(|s| s.feature == kind.feature()));
        let result = if i == 0 {
            gpu.clone()
        } else {
            Curve::fan(kind, setting.map(|s| &s.value))
        };
        match result {
            Ok(curve) => Row {
                curve: Some(curve),
                enabled: setting.is_some(),
                source: if i == 0 {
                    "GPU-Spannungsanker sind fest. Bearbeiten und Speichern ändern keine Hardware.\r\nFrequenzziele sind kein hartes Spannungslimit; Stabilität unter Last ist ungeprüft.".into()
                } else {
                    format!(
                        "{} · {}. Bearbeiten und Speichern verändern keine Lüfter.\r\nProfile mit Lüfterkurven sind bis zur geprüften HP-Anbindung noch nicht anwendbar.",
                        kind.title(),
                        if setting.is_some() {
                            "Gespeicherte Kurve"
                        } else {
                            "Vorlage, kein ausgelesener Hardwarezustand"
                        }
                    )
                },
            },
            Err(error) => Row {
                curve: None,
                enabled: setting.is_some(),
                source: error,
            },
        }
    });
    Ok(View {
        profiles,
        selected,
        binding,
        rows,
    })
}
enum Request {
    Load(Option<String>),
    Save {
        baseline: Box<Option<PerformanceProfile>>,
        binding: Box<DeviceBinding>,
        name: String,
        rows: Box<[Row; 3]>,
    },
    Stop,
}
enum Update {
    View(Box<View>, String),
    Error(String),
}
fn worker(hwnd: usize, rx: mpsc::Receiver<Request>, tx: mpsc::Sender<Update>) {
    while let Ok(request) = rx.recv() {
        if matches!(request, Request::Stop) {
            break;
        }
        let result = (|| -> Result<(View, String), String> {
            match request {
                Request::Load(id) => Ok((load(id.as_deref())?, String::new())),
                Request::Save {
                    baseline,
                    binding,
                    name,
                    rows,
                } => {
                    let store = ProfileStore::user_data()?;
                    let mut profile = if let Some(old) = baseline.as_ref() {
                        let fresh = store.load(&old.id)?;
                        let managed = |p: &PerformanceProfile| {
                            p.settings
                                .iter()
                                .filter(|s| KINDS.iter().any(|k| k.feature() == s.feature))
                                .cloned()
                                .collect::<Vec<_>>()
                        };
                        if managed(&fresh) != managed(old)
                            || (name != old.name && fresh.name != old.name)
                        {
                            return Err("Die Kurven wurden in einem anderen Fenster geändert. Neu laden, bevor sie überschrieben werden.".into());
                        }
                        fresh
                    } else {
                        PerformanceProfile {
                            schema: PROFILE_SCHEMA,
                            id: format!("curves-{}", token()),
                            name: name.clone(),
                            binding: (*binding).clone(),
                            settings: vec![],
                        }
                    };
                    if baseline
                        .as_ref()
                        .as_ref()
                        .is_none_or(|old| name != old.name)
                    {
                        profile.name = name;
                    }
                    for row in rows.iter() {
                        let kind = row.curve.as_ref().map(|c| c.kind);
                        if let Some(kind) = kind {
                            profile.settings.retain(|s| s.feature != kind.feature());
                            if row.enabled {
                                if kind == Kind::Gpu {
                                    profile
                                        .settings
                                        .retain(|s| s.feature != Feature::GpuCoreOffsetMhz);
                                    if profile.binding.nvidia_uuid.is_none() {
                                        if !profile
                                            .binding
                                            .for_settings(&[])
                                            .matches(&binding.for_settings(&[]))
                                        {
                                            return Err("Profil und aktuelles Board/BIOS unterscheiden sich.".into());
                                        }
                                        profile.binding = (*binding).clone();
                                    }
                                }
                                profile.settings.push(Setting {
                                    feature: kind.feature(),
                                    value: row.curve.as_ref().unwrap().value()?,
                                });
                            }
                        } else if row.enabled {
                            return Err("Eine ausgewählte Kurve ist nicht verfügbar.".into());
                        }
                    }
                    if profile.settings.is_empty() {
                        return Err("Mindestens eine Kurve im Profil auswählen.".into());
                    }
                    profile.binding = profile.binding.for_settings(&profile.settings);
                    store.save(&profile)?;
                    let fans = rows.iter().skip(1).any(|r| r.enabled);
                    Ok((load(Some(&profile.id))?,if fans{"Profil gespeichert. Keine Hardware verändert. Lüfterkurven bleiben Entwürfe; Anwenden ist noch gesperrt."}else{"Profil gespeichert. Keine Hardware verändert. Anwenden und Startoptionen befinden sich im GPU-Profilfenster."}.into()))
                }
                Request::Stop => unreachable!(),
            }
        })();
        let update = match result {
            Ok((view, status)) => Update::View(Box::new(view), status),
            Err(e) => Update::Error(e),
        };
        if tx.send(update).is_err() {
            break;
        }
        // SAFETY: Only an opaque HWND notification; no cross-thread Rust pointer.
        unsafe {
            PostMessageW(hwnd as HWND, UPDATE, 0, 0);
        }
    }
}
#[derive(Clone, Copy)]
struct Controls {
    profiles: HWND,
    name: HWND,
    save: HWND,
    new: HWND,
    modes: [HWND; 3],
    include: HWND,
    source: HWND,
    canvas: HWND,
    points: HWND,
    xlabel: HWND,
    x: HWND,
    ylabel: HWND,
    y: HWND,
    set: HWND,
    add: HWND,
    remove: HWND,
    plateau: HWND,
    reset: HWND,
    status: HWND,
    open: HWND,
    reload: HWND,
}
struct State {
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Update>,
    view: RefCell<Option<View>>,
    controls: Cell<Option<Controls>>,
    font: Cell<HFONT>,
    mode: Cell<usize>,
    busy: Cell<bool>,
    rendering: Cell<bool>,
    dirty: Cell<bool>,
}
fn selected(s: &State) -> Option<String> {
    s.view
        .borrow()
        .as_ref()?
        .selected
        .as_ref()
        .map(|p| p.id.clone())
}
fn sync(s: &State) {
    let Some(c) = s.controls.get() else {
        return;
    };
    let model = plot::curve(c.canvas);
    if let Some(view) = s.view.borrow_mut().as_mut() {
        view.rows[s.mode.get()].curve = model.clone();
    }
    s.rendering.set(true);
    // SAFETY: Owned controls copy strings; item data is a bounded integer index.
    unsafe {
        SendMessageW(c.points, CB_RESETCONTENT, 0, 0);
    }
    if let Some(model) = model {
        let p = model.points[model.selected];
        let (x, y) = model.kind.units();
        text(
            c.xlabel,
            if model.kind == Kind::Gpu {
                "Spannungsanker (mV)"
            } else {
                "Temperatur (°C)"
            },
        );
        text(
            c.ylabel,
            if model.kind == Kind::Gpu {
                "Frequenz (MHz)"
            } else {
                "Lüfterwert (%)"
            },
        );
        text(c.x, &p.x.to_string());
        text(c.y, &p.y.to_string());
        unsafe {
            for p in &model.points {
                SendMessageW(
                    c.points,
                    CB_ADDSTRING,
                    0,
                    wide(&format!("{} {} · {} {}", p.x, x, p.y, y)).as_ptr() as isize,
                );
            }
            SendMessageW(c.points, CB_SETCURSEL, model.selected, 0);
            SendMessageW(c.x, EM_SETREADONLY, usize::from(model.kind == Kind::Gpu), 0);
        }
    } else {
        text(c.x, "");
        text(c.y, "");
    }
    s.rendering.set(false);
    enabled(s);
}
fn enabled(s: &State) {
    let Some(c) = s.controls.get() else {
        return;
    };
    let view = s.view.borrow();
    let row = view.as_ref().map(|v| &v.rows[s.mode.get()]);
    let ready = row.is_some_and(|r| r.curve.is_some()) && !s.busy.get();
    unsafe {
        for h in [c.profiles, c.new, c.reload, c.name] {
            EnableWindow(h, (!s.busy.get()) as i32);
        }
        for h in c.modes {
            EnableWindow(h, (!s.busy.get()) as i32);
        }
        for h in [c.include, c.canvas, c.points, c.x, c.y, c.set, c.reset] {
            EnableWindow(h, ready as i32);
        }
        for h in [c.add, c.remove] {
            EnableWindow(h, (ready && s.mode.get() != 0) as i32);
        }
        EnableWindow(c.plateau, (ready && s.mode.get() == 0) as i32);
        EnableWindow(c.save, (view.is_some() && !s.busy.get()) as i32);
        EnableWindow(
            c.open,
            (selected(s).is_some() && !s.dirty.get() && !s.busy.get()) as i32,
        );
    }
}
fn mode(s: &State, index: usize, capture: bool) {
    let Some(c) = s.controls.get() else {
        return;
    };
    if capture {
        if let Some(view) = s.view.borrow_mut().as_mut() {
            view.rows[s.mode.get()].curve = plot::curve(c.canvas);
        }
    }
    s.mode.set(index);
    let row = {
        let view = s.view.borrow();
        let Some(view) = view.as_ref() else {
            return;
        };
        view.rows[index].clone()
    };
    unsafe {
        for (i, h) in c.modes.iter().enumerate() {
            SendMessageW(
                *h,
                BM_SETCHECK,
                if i == index {
                    BST_CHECKED
                } else {
                    BST_UNCHECKED
                } as usize,
                0,
            );
        }
        SendMessageW(
            c.include,
            BM_SETCHECK,
            if row.enabled {
                BST_CHECKED
            } else {
                BST_UNCHECKED
            } as usize,
            0,
        );
    }
    text(c.source, &row.source);
    plot::set(c.canvas, row.curve.clone());
    sync(s);
}
fn dirty(s: &State) {
    s.dirty.set(true);
    if let Some(c) = s.controls.get() {
        text(
            c.status,
            "Ungespeicherter Entwurf. Ziehen und Tastatur ändern nur die Kurve; Speichern ändert keine Hardware.",
        );
    }
    enabled(s);
}
fn edit(s: &State, action: impl FnOnce(&mut Curve) -> Result<bool, String>) {
    let Some(c) = s.controls.get() else {
        return;
    };
    match plot::mutate(c.canvas, action) {
        Ok(changed) => {
            if changed {
                if let Some(v) = s.view.borrow_mut().as_mut() {
                    v.rows[s.mode.get()].enabled = true;
                }
                unsafe {
                    SendMessageW(c.include, BM_SETCHECK, BST_CHECKED as usize, 0);
                }
                dirty(s);
            }
            sync(s);
        }
        Err(e) => text(c.status, &e),
    }
}
fn dispatch(s: &State, request: Request, status: &str) {
    if s.busy.replace(true) {
        return;
    }
    if let Some(c) = s.controls.get() {
        text(c.status, status);
    }
    enabled(s);
    if s.tx.send(request).is_err() {
        s.busy.set(false);
        if let Some(c) = s.controls.get() {
            text(c.status, "Ladevorgang nicht verfügbar. Fenster neu öffnen.");
        }
        enabled(s);
    }
}
fn show(s: &State, view: View, status: &str) {
    let Some(c) = s.controls.get() else {
        return;
    };
    s.rendering.set(true);
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
        let i = view
            .selected
            .as_ref()
            .and_then(|p| view.profiles.iter().position(|q| q.id == p.id))
            .map_or(0, |i| i + 1);
        SendMessageW(c.profiles, CB_SETCURSEL, i, 0);
    }
    text(
        c.name,
        view.selected
            .as_ref()
            .map_or("Mein Kurvenprofil", |p| &p.name),
    );
    *s.view.borrow_mut() = Some(view);
    s.busy.set(false);
    s.dirty.set(false);
    s.rendering.set(false);
    mode(s, s.mode.get(), false);
    text(
        c.status,
        if status.is_empty() {
            "Punkte ziehen oder links/rechts wählen, oben/unten ändern; Umschalt = 10 Schritte.\r\nLüfter: Strg + links/rechts ändert die Temperatur. Einfg/Entf fügt hinzu oder entfernt."
        } else {
            status
        },
    );
}
fn layout(hwnd: HWND, s: &State) {
    let Some(c) = s.controls.get() else {
        return;
    };
    let mut r: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetClientRect(hwnd, &mut r);
    }
    let w = r.right.max(780);
    let h = r.bottom.max(620);
    let plot_height = (h - 405).max(200);
    let bottom = 190 + plot_height;
    let mut boxes = vec![
        (c.profiles, [20, 54, 290, 240]),
        (c.name, [326, 54, w - 510, 32]),
        (c.save, [w - 170, 54, 150, 34]),
        (c.new, [20, 14, 100, 30]),
        (c.reload, [132, 14, 140, 30]),
        (c.include, [580, 104, w - 600, 28]),
        (c.source, [20, 142, w - 40, 42]),
        (c.canvas, [20, 190, w - 40, plot_height]),
        (c.points, [20, bottom + 12, w - 40, 240]),
        (c.xlabel, [20, bottom + 55, 176, 28]),
        (c.x, [200, bottom + 52, 100, 32]),
        (c.ylabel, [324, bottom + 55, 150, 28]),
        (c.y, [480, bottom + 52, 100, 32]),
        (c.set, [600, bottom + 52, w - 620, 34]),
        (c.add, [20, bottom + 96, 155, 34]),
        (c.remove, [190, bottom + 96, 155, 34]),
        (c.plateau, [360, bottom + 96, 195, 34]),
        (c.reset, [570, bottom + 96, w - 590, 34]),
        (c.status, [20, h - 68, w - 255, 58]),
        (c.open, [w - 220, h - 68, 200, 58]),
    ];
    for (i, handle) in c.modes.iter().enumerate() {
        boxes.push((*handle, [20 + i as i32 * 185, 104, 175, 28]));
    }
    unsafe {
        for (handle, b) in boxes {
            MoveWindow(handle, b[0], b[1], b[2], b[3], 1);
        }
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: Main-thread Box<State> stays alive through the message loop.
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
        if p.is_null() {
            return DefWindowProcW(hwnd, message, w, l);
        }
        let s = &*p;
        match message {
            WM_CREATE => {
                let mut f: LOGFONTW = std::mem::zeroed();
                f.lfHeight = -18;
                for (a, b) in f.lfFaceName.iter_mut().zip(wide("Segoe UI")) {
                    *a = b;
                }
                s.font.set(CreateFontIndirectW(&f));
                let f = s.font.get();
                let child = |id, class, label, style| {
                    control(hwnd, f, id, class, label, style, [0, 0, 100, 30])
                };
                let profiles = child(SELECT, "COMBOBOX", "", WS_TABSTOP | CBS_DROPDOWNLIST as u32);
                let name = child(
                    NAME,
                    "EDIT",
                    "",
                    WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                );
                SendMessageW(name, EM_SETLIMITTEXT, 96, 0);
                let modes = std::array::from_fn(|i| {
                    child(
                        GPU + i,
                        "BUTTON",
                        KINDS[i].title(),
                        WS_TABSTOP | BS_AUTORADIOBUTTON as u32,
                    )
                });
                let canvas = match plot::create(hwnd, CANVAS, f) {
                    Ok(h) => h,
                    Err(_) => return -1,
                };
                let x = child(
                    X,
                    "EDIT",
                    "",
                    WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                );
                let y = child(
                    Y,
                    "EDIT",
                    "",
                    WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                );
                SendMessageW(x, EM_SETLIMITTEXT, 8, 0);
                SendMessageW(y, EM_SETLIMITTEXT, 8, 0);
                let c = Controls {
                    profiles,
                    name,
                    save: child(SAVE, "BUTTON", "&Speichern", WS_TABSTOP),
                    new: child(NEW, "BUTTON", "&Neu", WS_TABSTOP),
                    modes,
                    include: child(
                        INCLUDE,
                        "BUTTON",
                        "Kurve im &Profil",
                        WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                    ),
                    source: child(0, "STATIC", "Kurven werden gelesen …", 0),
                    canvas,
                    points: child(POINT, "COMBOBOX", "", WS_TABSTOP | CBS_DROPDOWNLIST as u32),
                    xlabel: child(0, "STATIC", "Spannungsanker (mV)", 0),
                    x,
                    ylabel: child(0, "STATIC", "Frequenz (MHz)", 0),
                    y,
                    set: child(SET_POINT, "BUTTON", "&Wert übernehmen", WS_TABSTOP),
                    add: child(ADD, "BUTTON", "Punkt &hinzufügen", WS_TABSTOP),
                    remove: child(REMOVE, "BUTTON", "Punkt &entfernen", WS_TABSTOP),
                    plateau: child(PLATEAU, "BUTTON", "Ab hier P&lateau", WS_TABSTOP),
                    reset: child(RESET, "BUTTON", "Kurve &zurücksetzen", WS_TABSTOP),
                    status: child(
                        STATUS,
                        "EDIT",
                        "Wird gelesen …",
                        WS_BORDER
                            | WS_VSCROLL
                            | ES_MULTILINE as u32
                            | ES_READONLY as u32
                            | ES_AUTOVSCROLL as u32,
                    ),
                    open: child(OPEN, "BUTTON", "Anwenden & Startoptionen …", WS_TABSTOP),
                    reload: child(RELOAD, "BUTTON", "Neu &laden", WS_TABSTOP),
                };
                if [
                    c.profiles, c.name, c.save, c.source, c.canvas, c.points, c.x, c.y, c.set,
                    c.status, c.open,
                ]
                .iter()
                .any(|h| h.is_null())
                {
                    return -1;
                }
                s.controls.set(Some(c));
                layout(hwnd, s);
                enabled(s);
                0
            }
            WM_SIZE => {
                layout(hwnd, s);
                0
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(l as *mut MINMAXINFO);
                info.ptMinTrackSize = POINT { x: 820, y: 710 };
                0
            }
            UPDATE => {
                while let Ok(update) = s.rx.try_recv() {
                    match update {
                        Update::View(v, status) => show(s, *v, &status),
                        Update::Error(e) => {
                            s.busy.set(false);
                            if let Some(c) = s.controls.get() {
                                text(c.status, &e);
                            }
                            enabled(s);
                        }
                    }
                }
                0
            }
            plot::CHANGED => {
                if let Some(c) = s.controls.get() {
                    if l == c.canvas as isize {
                        if w == 2 {
                            text(c.status, &plot::error(c.canvas));
                        } else {
                            if w == 1 {
                                if let Some(v) = s.view.borrow_mut().as_mut() {
                                    v.rows[s.mode.get()].enabled = true;
                                }
                                SendMessageW(c.include, BM_SETCHECK, BST_CHECKED as usize, 0);
                                dirty(s);
                            }
                            sync(s);
                        }
                    }
                }
                0
            }
            WM_COMMAND => {
                let id = w & 0xffff;
                let code = (w >> 16) as u32;
                if s.rendering.get() || s.busy.get() {
                    return 0;
                }
                let Some(c) = s.controls.get() else {
                    return 0;
                };
                if id == NAME && code == EN_CHANGE {
                    dirty(s);
                    return 0;
                }
                match id {
                    SELECT if code == CBN_SELCHANGE => {
                        let i = SendMessageW(c.profiles, CB_GETCURSEL, 0, 0);
                        let id = s
                            .view
                            .borrow()
                            .as_ref()
                            .and_then(|v| {
                                usize::try_from(i - 1).ok().and_then(|i| v.profiles.get(i))
                            })
                            .map(|p| p.id.clone());
                        dispatch(s, Request::Load(id), "Profil wird geladen …");
                    }
                    NEW => dispatch(
                        s,
                        Request::Load(None),
                        "Neue Kurvenvorlagen werden geladen …",
                    ),
                    RELOAD => dispatch(
                        s,
                        Request::Load(selected(s)),
                        "Gespeicherte Kurven werden erneut geladen …",
                    ),
                    GPU | CPU_FAN | GPU_FAN => mode(s, id - GPU, true),
                    INCLUDE => {
                        if let Some(v) = s.view.borrow_mut().as_mut() {
                            v.rows[s.mode.get()].enabled =
                                SendMessageW(c.include, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
                        }
                        dirty(s);
                    }
                    POINT if code == CBN_SELCHANGE => {
                        let i = SendMessageW(c.points, CB_GETCURSEL, 0, 0);
                        let _ = plot::mutate(c.canvas, |curve| {
                            if let Ok(i) = usize::try_from(i) {
                                curve.selected = i.min(curve.points.len() - 1);
                            }
                            Ok(false)
                        });
                        sync(s);
                    }
                    SET_POINT => {
                        match input(c.x)
                            .and_then(|x| {
                                x.trim()
                                    .parse::<i32>()
                                    .map_err(|_| "X-Wert als ganze Zahl eingeben.".into())
                            })
                            .and_then(|x| {
                                input(c.y).and_then(|y| {
                                    y.trim()
                                        .parse::<i32>()
                                        .map(|y| Point { x, y })
                                        .map_err(|_| "Y-Wert als ganze Zahl eingeben.".into())
                                })
                            }) {
                            Ok(p) => edit(s, |curve| curve.set(curve.selected, p, false)),
                            Err(e) => text(c.status, &e),
                        }
                    }
                    PLATEAU => {
                        match input(c.y).and_then(|y| {
                            y.trim()
                                .parse::<i32>()
                                .map_err(|_| "Frequenz als ganze MHz-Zahl eingeben.".into())
                        }) {
                            Ok(y) => edit(s, |curve| curve.plateau(y)),
                            Err(e) => text(c.status, &e),
                        }
                    }
                    ADD => edit(s, |curve| {
                        curve.insert()?;
                        Ok(true)
                    }),
                    REMOVE => edit(s, |curve| {
                        curve.remove()?;
                        Ok(true)
                    }),
                    RESET => edit(s, |curve| {
                        curve.reset();
                        Ok(true)
                    }),
                    SAVE => {
                        sync(s);
                        let view = s.view.borrow();
                        if let Some(v) = view.as_ref() {
                            match input(c.name) {
                                Ok(name) => {
                                    let request = Request::Save {
                                        baseline: Box::new(v.selected.clone()),
                                        binding: Box::new(v.binding.clone()),
                                        name: name.trim().into(),
                                        rows: Box::new(v.rows.clone()),
                                    };
                                    drop(view);
                                    dispatch(
                                        s,
                                        request,
                                        "Kurvenprofil wird gespeichert; keine Hardwareänderung …",
                                    );
                                }
                                Err(e) => text(c.status, &e),
                            }
                        }
                    }
                    OPEN if !s.dirty.get() => {
                        if let Some(id) = selected(s) {
                            if let Err(e) = std::env::current_exe().and_then(|exe| {
                                std::process::Command::new(exe)
                                    .args(["gpu-ui", &id])
                                    .spawn()
                            }) {
                                text(
                                    c.status,
                                    &format!("Profilfenster konnte nicht geöffnet werden: {e}"),
                                );
                            }
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
pub fn preview(kind: &str, path: &Path) -> Result<(), String> {
    let view = load(None)?;
    let i = match kind {
        "gpu" => 0,
        "cpu-fan" => 1,
        "gpu-fan" => 2,
        _ => return Err("Preview-Typ gpu/cpu-fan/gpu-fan erwartet.".into()),
    };
    plot::render(
        path,
        view.rows[i]
            .curve
            .as_ref()
            .ok_or("Kurve nicht verfügbar.")?,
    )
}
pub fn run(id: Option<String>) -> Result<(), String> {
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
        mode: Cell::new(0),
        busy: Cell::new(false),
        rendering: Cell::new(false),
        dirty: Cell::new(false),
    });
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class = wide("OmenHubPPCurveEditor");
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
            wide("Omen Hub++ – GPU- und Lüfterkurven").as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            960,
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
        let thread = thread::spawn(move || worker(handle, requests, updates));
        ShowWindow(hwnd, SW_SHOW);
        dispatch(&state, Request::Load(id), "Kurven werden gelesen …");
        let mut msg: MSG = std::mem::zeroed();
        let result = loop {
            let result = GetMessageW(&mut msg, ptr::null_mut(), 0, 0);
            if result <= 0 {
                break result;
            }
            if msg.message == WM_KEYDOWN && msg.wParam == 0x53 && GetKeyState(0x11) < 0 {
                PostMessageW(hwnd, WM_COMMAND, SAVE, 0);
                continue;
            }
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        };
        let _ = state.tx.send(Request::Stop);
        if thread.is_finished() {
            let _ = thread.join();
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
