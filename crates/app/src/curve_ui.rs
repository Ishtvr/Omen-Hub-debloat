//! Native, metadata-only workbench for V/F and independent CPU/GPU fan curves.
use crate::{
    curve_widget as plot,
    native_ui::{input, positioned_control as control, text, token, wide},
};
use omen_core::{
    curve_editor::{Curve, Kind, Point},
    features::Feature,
    profiles::{DeviceBinding, PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
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
    Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::{
            BST_CHECKED, BST_UNCHECKED, DRAWITEMSTRUCT, EM_SETLIMITTEXT, EM_SETREADONLY, EM_SETSEL,
            ODT_BUTTON, SetWindowTheme,
        },
        Input::KeyboardAndMouse::{EnableWindow, GetKeyState, SetFocus},
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
const UNDO: usize = 521;
const REDO: usize = 522;
const ZOOM_IN: usize = 523;
const ZOOM_OUT: usize = 524;
const FIT: usize = 525;
const HELP: usize = 526;
const FOCUS: usize = 527;
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
    undo: HWND,
    redo: HWND,
    zoom: [HWND; 3],
    help: HWND,
    focus: HWND,
}
struct State {
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Update>,
    view: RefCell<Option<View>>,
    controls: Cell<Option<Controls>>,
    font: Cell<HFONT>,
    mode: Cell<usize>,
    allowed: [bool; 3],
    busy: Cell<bool>,
    rendering: Cell<bool>,
    dirty: Cell<bool>,
    graph_only: Cell<bool>,
    background: Cell<HBRUSH>,
    surface: Cell<HBRUSH>,
    theme: crate::theme::Theme,
    draft: bool,
    draft_result: RefCell<Option<Value>>,
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
        let model = plot::curve(c.canvas);
        EnableWindow(
            c.undo,
            (ready && model.as_ref().is_some_and(Curve::can_undo)) as i32,
        );
        EnableWindow(
            c.redo,
            (ready && model.as_ref().is_some_and(Curve::can_redo)) as i32,
        );
        for h in c.zoom {
            EnableWindow(h, ready as i32);
        }
        EnableWindow(c.save, (view.is_some() && !s.busy.get()) as i32);
        EnableWindow(
            c.open,
            (s.draft || (selected(s).is_some() && !s.dirty.get() && !s.busy.get())) as i32,
        );
    }
}
fn mode(s: &State, index: usize, capture: bool) {
    if !s.allowed.get(index).copied().unwrap_or(false) {
        return;
    }
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
            if s.draft {
                "Kurvenentwurf geändert · Im Preset speichern."
            } else {
                "Ungespeicherter Entwurf. Ziehen und Tastatur ändern nur die Kurve; Speichern ändert keine Hardware."
            },
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
            "Entwurf · Speichern verändert keine Hardware. Shift: Kurve/Bereich · Strg + Z: Rückgängig · F1: Tastenkürzel"
        } else {
            status
        },
    );
}
fn layout(hwnd: HWND, s: &State) {
    let Some(c) = s.controls.get() else {
        return;
    };
    unsafe {
        for (i, handle) in c.modes.iter().enumerate() {
            ShowWindow(*handle, if s.allowed[i] { SW_SHOW } else { SW_HIDE });
        }
    }
    let mut r: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetClientRect(hwnd, &mut r);
    }
    let w = r.right.max(780);
    let h = r.bottom.max(620);
    let regular = [
        c.profiles, c.name, c.new, c.reload, c.include, c.source, c.points, c.xlabel, c.x,
        c.ylabel, c.y, c.set, c.add, c.remove, c.plateau, c.reset, c.open,
    ];
    unsafe {
        for handle in regular {
            ShowWindow(handle, if s.graph_only.get() { SW_HIDE } else { SW_SHOW });
        }
    }
    if s.graph_only.get() {
        let narrow = w < 1130;
        let top = if narrow { 94 } else { 54 };
        let toolbar = if narrow { 54 } else { 14 };
        let mut boxes = vec![
            (c.canvas, [8, top, w - 16, h - top - 54]),
            (c.save, [if narrow { 20 } else { 400 }, toolbar, 115, 30]),
            (c.undo, [if narrow { 150 } else { 530 }, toolbar, 125, 30]),
            (c.redo, [if narrow { 290 } else { 670 }, toolbar, 135, 30]),
            (c.help, [w - 300, 14, 135, 30]),
            (c.focus, [w - 155, 14, 135, 30]),
            (c.status, [20, h - 42, w - 375, 32]),
            (c.zoom[0], [w - 340, h - 44, 90, 30]),
            (c.zoom[1], [w - 240, h - 44, 90, 30]),
            (c.zoom[2], [w - 140, h - 44, 120, 30]),
        ];
        for (slot, (_, handle)) in c
            .modes
            .iter()
            .enumerate()
            .filter(|(i, _)| s.allowed[*i])
            .enumerate()
        {
            boxes.push((*handle, [20 + slot as i32 * 155, 14, 150, 28]));
        }
        if s.draft {
            unsafe {
                ShowWindow(c.open, SW_SHOW);
            }
            boxes.push((c.open, [180, 14, 140, 30]));
        }
        unsafe {
            for (handle, b) in boxes {
                MoveWindow(handle, b[0], b[1], b[2], b[3], 1);
            }
        }
        text(
            c.focus,
            if s.draft {
                "Werte …"
            } else {
                "Profil / Werte …"
            },
        );
        return;
    }
    text(
        c.focus,
        if s.draft {
            "Diagramm"
        } else {
            "Diagrammansicht"
        },
    );
    let plot_top = if s.draft { 128 } else { 202 };
    let plot_height = (h - if s.draft { 343 } else { 417 }).max(200);
    let bottom = plot_top + plot_height;
    let mut boxes = vec![
        (c.profiles, [20, 54, 290, 240]),
        (c.name, [326, 54, w - 510, 32]),
        (c.save, [w - 170, 54, 150, 34]),
        (c.new, [20, 14, 100, 30]),
        (c.reload, [132, 14, 140, 30]),
        (c.undo, [286, 14, 125, 30]),
        (c.redo, [425, 14, 135, 30]),
        (c.help, [w - 300, 14, 135, 30]),
        (c.focus, [w - 155, 14, 135, 30]),
        (c.include, [580, 104, w - 600, 28]),
        (c.source, [20, 142, w - 40, 50]),
        (c.canvas, [20, plot_top, w - 40, plot_height]),
        (c.points, [20, bottom + 12, w - 40, 240]),
        (c.xlabel, [20, bottom + 55, 176, 28]),
        (c.x, [200, bottom + 52, 100, 32]),
        (c.ylabel, [324, bottom + 55, 150, 28]),
        (c.y, [480, bottom + 52, 100, 32]),
        (c.set, [600, bottom + 52, w - 620, 34]),
        (c.add, [20, bottom + 96, 145, 34]),
        (c.remove, [175, bottom + 96, 145, 34]),
        (c.plateau, [20, bottom + 96, 170, 34]),
        (c.reset, [200, bottom + 96, 200, 34]),
        (c.zoom[0], [w - 340, bottom + 96, 90, 34]),
        (c.zoom[1], [w - 240, bottom + 96, 90, 34]),
        (c.zoom[2], [w - 140, bottom + 96, 120, 34]),
        (c.status, [20, h - 68, w - 255, 58]),
        (c.open, [w - 220, h - 68, 200, 58]),
    ];
    for (slot, (_, handle)) in c
        .modes
        .iter()
        .enumerate()
        .filter(|(i, _)| s.allowed[*i])
        .enumerate()
    {
        boxes.push((*handle, [20 + slot as i32 * 185, 104, 175, 28]));
    }
    unsafe {
        for (handle, b) in boxes {
            MoveWindow(handle, b[0], b[1], b[2], b[3], 1);
        }
        ShowWindow(c.add, if s.mode.get() != 0 { SW_SHOW } else { SW_HIDE });
        ShowWindow(c.remove, if s.mode.get() != 0 { SW_SHOW } else { SW_HIDE });
        ShowWindow(c.plateau, if s.mode.get() == 0 { SW_SHOW } else { SW_HIDE });
        if s.mode.get() != 0 {
            MoveWindow(c.reset, 340, bottom + 96, 180, 34, 1);
        }
        if s.draft {
            for handle in [c.profiles, c.name, c.new, c.reload, c.include] {
                ShowWindow(handle, SW_HIDE);
            }
            for handle in c.modes {
                ShowWindow(handle, SW_HIDE);
            }
            MoveWindow(c.save, 20, 14, 150, 34, 1);
            MoveWindow(c.undo, 186, 14, 125, 30, 1);
            MoveWindow(c.redo, 325, 14, 135, 30, 1);
            MoveWindow(c.source, 20, 54, w - 40, 50, 1);
        }
    }
}
fn help(hwnd: HWND, draft: bool) {
    let message = "GPU-Kurve\r\nZiehen: Punkt oder ausgewählten Bereich vertikal verschieben.\r\nShift + Punkt ziehen: geladene Ausgangskurve/Auswahl verschieben.\r\nAlt + Punkt ziehen: aktuellen Entwurf/Auswahl gleichmäßig verschieben.\r\nStrg + Punkt ziehen: Kurve um den gegenüberliegenden Randpunkt neigen.\r\nShift + Ziehen im freien Diagramm: Spannungsbereich auswählen.\r\nShift + Links/Rechts: Auswahl erweitern. Strg + A: alle Punkte.\r\nShift + Enter: Auswahl abflachen; ohne Auswahl Plateau ab dem Punkt.\r\nEsc oder Doppelklick: Auswahl aufheben; Esc beim Ziehen: abbrechen.\r\n\r\nGemeinsame Bedienung\r\nLinks/Rechts: Punkt wählen. Oben/Unten: 1 MHz bzw. 1 %.\r\nStrg + Oben/Unten: 10 Schritte. Enter: Zahlenfeld bearbeiten.\r\nStrg + Z: Rückgängig. Strg + Y / Strg + Shift + Z: Wiederholen.\r\nStrg + D: geladenen Entwurf zurücksetzen. Strg + S: Profil speichern.\r\nMausrad oder +/−: Spannung/Temperatur zoomen. Strg: Y-Achse.\r\nShift + Mausrad: horizontal verschieben. Strg + 0: ganze Kurve.\r\n\r\nLüfter\r\nStrg + Links/Rechts: Temperatur bearbeiten. Shift: größere Schritte.\r\nEinfg/Entf: Punkt hinzufügen/entfernen. Separate CPU-/GPU-Kurven.\r\n\r\nSpeichern ändert keine Hardware. Anwenden erfolgt im Profilfenster.\r\nLüfteranwendung, fester Spannungs-/Takt-Lock und OC Scanner sind noch nicht verfügbar.";
    let message = if draft {
        message.replace("Strg + S: Profil speichern.", "Strg + S: Kurve in den Preset-Entwurf übernehmen.")
            .replace("Speichern ändert keine Hardware. Anwenden erfolgt im Profilfenster.", "Übernehmen ändert nur den Preset-Entwurf. Erst dort speichern und ausdrücklich anwenden.")
    } else {
        message.to_string()
    };
    unsafe {
        MessageBoxW(
            hwnd,
            wide(&message).as_ptr(),
            wide("Kurveneditor – Tastenkürzel").as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}
unsafe fn draw_button(s: &State, item: &DRAWITEMSTRUCT) {
    unsafe {
        s.theme
            .button(item, s.font.get(), item.CtlID == SAVE as u32);
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
        if let Some(color) = s.theme.color(message, w, l) {
            return color;
        }
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
                    let style = if class == "BUTTON"
                        && style & (BS_AUTORADIOBUTTON | BS_AUTOCHECKBOX) as u32 == 0
                    {
                        style | BS_OWNERDRAW as u32
                    } else {
                        style
                    };
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
                    reset: child(RESET, "BUTTON", "&Zurücksetzen", WS_TABSTOP),
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
                    open: child(OPEN, "BUTTON", "&Profil öffnen …", WS_TABSTOP),
                    reload: child(RELOAD, "BUTTON", "Neu &laden", WS_TABSTOP),
                    undo: child(UNDO, "BUTTON", "Rückgängig", WS_TABSTOP),
                    redo: child(REDO, "BUTTON", "Wiederholen", WS_TABSTOP),
                    zoom: [
                        child(ZOOM_IN, "BUTTON", "Zoom +", WS_TABSTOP),
                        child(ZOOM_OUT, "BUTTON", "Zoom −", WS_TABSTOP),
                        child(FIT, "BUTTON", "Ganze Kurve", WS_TABSTOP),
                    ],
                    help: child(HELP, "BUTTON", "Tastenkürzel", WS_TABSTOP),
                    focus: child(FOCUS, "BUTTON", "Profil / Werte …", WS_TABSTOP),
                };
                // Public theme API: classic native radios/checks honour the
                // window's WM_CTLCOLOR foreground and retain native semantics.
                for handle in c.modes.into_iter().chain([c.include]) {
                    SetWindowTheme(handle, wide("").as_ptr(), wide("").as_ptr());
                }
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
                info.ptMinTrackSize = POINT { x: 960, y: 710 };
                0
            }
            WM_ERASEBKGND => {
                let mut r: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut r);
                s.theme.fill(hwnd, w as HDC);
                1
            }
            WM_PRINTCLIENT => {
                let mut r: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut r);
                s.theme.fill(hwnd, w as HDC);
                0
            }
            WM_DRAWITEM => {
                let item = &*(l as *const DRAWITEMSTRUCT);
                if item.CtlType == ODT_BUTTON {
                    draw_button(s, item);
                    1
                } else {
                    0
                }
            }
            plot::EDIT_VALUE => {
                if let Some(c) = s.controls.get() {
                    if l == c.canvas as isize {
                        s.graph_only.set(false);
                        layout(hwnd, s);
                        SetFocus(c.y);
                        SendMessageW(c.y, EM_SETSEL, 0, -1);
                    }
                }
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
                if !matches!(id, NAME | SELECT | POINT | X | Y) && code != BN_CLICKED {
                    return 0;
                }
                if s.rendering.get() || s.busy.get() {
                    return 0;
                }
                if s.draft && matches!(id, NAME | SELECT | NEW | RELOAD | INCLUDE) {
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
                    GPU | CPU_FAN | GPU_FAN => {
                        mode(s, id - GPU, true);
                        layout(hwnd, s);
                    }
                    UNDO | REDO => {
                        SendMessageW(
                            c.canvas,
                            if id == UNDO { plot::UNDO } else { plot::REDO },
                            0,
                            0,
                        );
                    }
                    ZOOM_IN | ZOOM_OUT | FIT => {
                        SendMessageW(
                            c.canvas,
                            match id {
                                ZOOM_IN => plot::ZOOM_IN,
                                ZOOM_OUT => plot::ZOOM_OUT,
                                _ => plot::FIT,
                            },
                            0,
                            0,
                        );
                    }
                    HELP => help(hwnd, s.draft),
                    FOCUS => {
                        s.graph_only.set(!s.graph_only.get());
                        layout(hwnd, s);
                    }
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
                                curve.select(i, false);
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
                        if s.draft {
                            match plot::curve(c.canvas)
                                .ok_or_else(|| "GPU-Kurve nicht verfügbar.".to_string())
                                .and_then(|curve| curve.value())
                            {
                                Ok(value) => {
                                    *s.draft_result.borrow_mut() = Some(value);
                                    DestroyWindow(hwnd);
                                }
                                Err(error) => text(c.status, &error),
                            }
                            return 0;
                        }
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
                    OPEN if s.draft => {
                        DestroyWindow(hwnd);
                    }
                    OPEN if !s.dirty.get() => {
                        if let Some(id) = selected(s) {
                            if let Err(e) = std::env::current_exe().and_then(|exe| {
                                std::process::Command::new(exe)
                                    .args(["presets-ui", &id])
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
    if matches!(kind, "gpu-draft" | "gpu-draft-small" | "gpu-draft-controls") {
        let view = load(None)?;
        let curve = view.rows[0]
            .curve
            .clone()
            .ok_or_else(|| view.rows[0].source.clone())?;
        let small = kind == "gpu-draft-small";
        return run_editor(
            None,
            Some(Preview {
                path: path.into(),
                mode: 0,
                regular: kind == "gpu-draft-controls",
                width: if small { 960 } else { 1380 },
                height: if small { 710 } else { 940 },
                example: false,
            }),
            Some(false),
            Some((ptr::null_mut(), curve, view.binding)),
        )
        .map(|_| ());
    }
    let preview = match kind {
        "gpu-window" | "gpu-example" => Some((0, false, 1380, 940)),
        "gpu-small" => Some((0, false, 960, 710)),
        "gpu-controls" => Some((0, true, 1380, 940)),
        "cpu-fan-window" => Some((1, false, 1380, 940)),
        _ => None,
    };
    if let Some((mode, regular, width, height)) = preview {
        return run_window(
            None,
            Some(Preview {
                path: path.into(),
                mode,
                regular,
                width,
                height,
                example: kind == "gpu-example",
            }),
            None,
        );
    }
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
struct Preview {
    path: std::path::PathBuf,
    mode: usize,
    regular: bool,
    width: i32,
    height: i32,
    example: bool,
}
pub fn run(id: Option<String>) -> Result<(), String> {
    run_window(id, None, None)
}
pub fn run_gpu(id: Option<String>) -> Result<(), String> {
    run_window(id, None, Some(false))
}
pub fn run_fans(id: Option<String>) -> Result<(), String> {
    run_window(id, None, Some(true))
}
/// Edit an owned, validated curve in memory. No profile file or hardware write;
/// the caller decides when to save the surrounding preset.
pub fn edit_gpu_draft(
    owner: HWND,
    curve: Curve,
    binding: DeviceBinding,
) -> Result<Option<Value>, String> {
    if curve.kind != Kind::Gpu {
        return Err("GPU-Kurve erwartet.".into());
    }
    run_editor(None, None, Some(false), Some((owner, curve, binding)))
}
fn run_window(
    id: Option<String>,
    preview: Option<Preview>,
    fans: Option<bool>,
) -> Result<(), String> {
    run_editor(id, preview, fans, None).map(|_| ())
}
fn run_editor(
    id: Option<String>,
    preview: Option<Preview>,
    fans: Option<bool>,
    draft: Option<(HWND, Curve, DeviceBinding)>,
) -> Result<Option<Value>, String> {
    if let Some(id) = &id {
        omen_core::profiles::validate_id(id)?;
    }
    let (tx, requests) = mpsc::channel();
    let (updates, rx) = mpsc::channel();
    let state = Box::new(State {
        theme: crate::theme::Theme::new(),
        tx,
        rx,
        view: RefCell::new(None),
        controls: Cell::new(None),
        font: Cell::new(ptr::null_mut()),
        mode: Cell::new(
            preview
                .as_ref()
                .map_or(if fans == Some(true) { 1 } else { 0 }, |p| p.mode),
        ),
        allowed: match fans {
            Some(true) => [false, true, true],
            Some(false) => [true, false, false],
            None => [true; 3],
        },
        busy: Cell::new(false),
        rendering: Cell::new(false),
        dirty: Cell::new(false),
        graph_only: Cell::new(preview.as_ref().is_none_or(|p| !p.regular)),
        background: Cell::new(unsafe { CreateSolidBrush(plot::BACKGROUND) }),
        surface: Cell::new(unsafe { CreateSolidBrush(plot::SURFACE) }),
        draft: draft.is_some(),
        draft_result: RefCell::new(None),
    });
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let class = wide("OmenHubPPCurveEditor");
        let mut registration: WNDCLASSW = std::mem::zeroed();
        registration.lpfnWndProc = Some(procedure);
        registration.hInstance = instance;
        registration.lpszClassName = class.as_ptr();
        registration.hCursor = LoadCursorW(ptr::null_mut(), IDC_ARROW);
        // Every instance paints its own client. No class brush may outlive the
        // first modal editor, which can be reopened in the same preset process.
        registration.hbrBackground = ptr::null_mut();
        if RegisterClassW(&registration) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide(match fans {
                Some(true) => "Omen Hub++ – Lüfterkurve",
                Some(false) => "Omen Hub++ – GPU-Leistungskurve",
                None => "Omen Hub++ – GPU- und Lüfterkurven",
            })
            .as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            preview.as_ref().map_or(1380, |p| p.width),
            preview.as_ref().map_or(940, |p| p.height),
            draft.as_ref().map_or(ptr::null_mut(), |d| d.0),
            ptr::null_mut(),
            instance,
            (&*state as *const State).cast(),
        );
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let handle = hwnd as usize;
        let dark: i32 = 1;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            (&dark as *const i32).cast(),
            size_of::<i32>() as u32,
        );
        let thread = if let Some((_, curve, binding)) = draft {
            let rows = std::array::from_fn(|i| {
                Row {
                curve: if i == 0 { Some(curve.clone()) } else { None },
                enabled: i == 0,
                source: "GPU-Spannungsanker sind fest. Übernehmen ändert nur den Preset-Entwurf; keine Hardwareänderung.".into(),
            }
            });
            show(
                &state,
                View {
                    profiles: vec![],
                    selected: None,
                    binding,
                    rows,
                },
                "Kurvenentwurf · Speichern erfolgt im Preset.",
            );
            if let Some(c) = state.controls.get() {
                text(c.save, "Übernehmen");
                text(c.open, "Abbrechen");
            }
            if preview.is_some() {
                PostMessageW(hwnd, UPDATE, 0, 0);
            }
            None
        } else {
            let thread = thread::spawn(move || worker(handle, requests, updates));
            dispatch(&state, Request::Load(id), "Kurven werden gelesen …");
            Some(thread)
        };
        ShowWindow(hwnd, if preview.is_some() { SW_HIDE } else { SW_SHOW });
        if state.draft {
            if let Some(c) = state.controls.get() {
                SetFocus(c.canvas);
            }
        }
        let mut msg: MSG = std::mem::zeroed();
        let mut preview_error = None;
        let result = loop {
            let result = GetMessageW(&mut msg, ptr::null_mut(), 0, 0);
            if result <= 0 {
                break result;
            }
            if msg.message == WM_KEYDOWN && msg.wParam == 0x53 && GetKeyState(0x11) < 0 {
                PostMessageW(hwnd, WM_COMMAND, SAVE, 0);
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam == 0x70 {
                help(hwnd, state.draft);
                continue;
            }
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if let Some(preview) = &preview {
                if msg.message == UPDATE && !state.busy.get() {
                    if state.view.borrow().is_some() {
                        if preview.example {
                            if let Some(c) = state.controls.get() {
                                let edited = plot::mutate(c.canvas, |curve| {
                                    let start = curve.points.clone();
                                    curve.drag_gpu(
                                        &start,
                                        omen_core::curve_editor::DragMode::CurrentOffset,
                                        120,
                                    )?;
                                    let index = curve
                                        .points
                                        .iter()
                                        .position(|p| p.x >= 1000)
                                        .ok_or("Kein 1000-mV-Anker für den Beispielentwurf.")?;
                                    curve.select(index, false);
                                    curve.plateau(curve.points[index].y)?;
                                    curve.select_voltage_range(1000, 1250);
                                    Ok(true)
                                });
                                if let Err(e) = edited {
                                    preview_error = Some(e);
                                }
                                sync(&state);
                                text(
                                    c.status,
                                    "Beispielentwurf · nicht gespeichert oder angewendet. Speichern ändert keine Hardware.",
                                );
                            }
                        }
                        if let Err(e) = plot::render_window(hwnd, &preview.path) {
                            preview_error = Some(e);
                        }
                    } else {
                        preview_error =
                            Some("Der native Preview konnte die Kurven nicht laden.".into());
                    }
                    DestroyWindow(hwnd);
                    break 1;
                }
            }
        };
        let _ = state.tx.send(Request::Stop);
        if let Some(thread) = thread.filter(|thread| thread.is_finished()) {
            let _ = thread.join();
        }
        if !state.font.get().is_null() {
            DeleteObject(state.font.get());
        }
        DeleteObject(state.background.get());
        DeleteObject(state.surface.get());
        if let Some(e) = preview_error {
            Err(e)
        } else if result < 0 {
            Err(std::io::Error::last_os_error().to_string())
        } else {
            Ok(state.draft_result.borrow().clone())
        }
    }
}
