//! Native custom presets. Declarative fields, explicit save/apply and no tuners
//! in UI callbacks. Unsupported settings remain editable metadata, never writes.
use crate::{
    elevation::{self, Action},
    native_ui::{self as ui, text, wide},
    preset_fields::{self, Field, GROUPS},
    theme,
};
use omen_core::{
    curve_editor::Curve,
    features::{Feature, ValueKind},
    preset,
    profiles::{CoreDelta, DeviceBinding, PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
    transaction::{self, Access, Backend},
};
use omen_platform::{Controller, profile_store::ProfileStore};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::{Path, PathBuf},
    ptr,
    sync::mpsc,
    thread,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::{
            BST_CHECKED, BST_UNCHECKED, DRAWITEMSTRUCT, EM_SETLIMITTEXT, ODS_FOCUS, ODS_SELECTED,
        },
        Input::KeyboardAndMouse::{
            EnableWindow, GetFocus, ReleaseCapture, SetActiveWindow, SetCapture,
        },
        WindowsAndMessaging::*,
    },
};

const UPDATE: u32 = WM_APP + 31;
const SELECT: usize = 701;
const NEW: usize = 702;
const NAME: usize = 703;
const SAVE: usize = 704;
const APPLY: usize = 705;
const DELETE: usize = 706;
const STARTUP: usize = 707;
const TAB: usize = 708;
const GROUP_BASE: usize = 1200;
const FIELD_BASE: usize = 2000;
const PANE: usize = 710;
const STATUS: i32 = 711;
const FIELD_STRIDE: usize = 64;
const CLASS: &str = "OmenHubPPPresetEditor";
const PANE_CLASS: &str = "OmenHubPPPresetFields";
struct View {
    profiles: Vec<PerformanceProfile>,
    selected: Option<PerformanceProfile>,
    binding: DeviceBinding,
    capabilities: BTreeMap<Feature, bool>,
    curve: Result<omen_core::vf::VfState, String>,
    removed_gpu_options: usize,
    core_curve_conflict: bool,
    pending: bool,
    applicable: bool,
    blockers: usize,
    startup_allowed: bool,
    startup_enabled: bool,
    startup_valid: bool,
}
fn load(id: Option<&str>) -> Result<View, String> {
    let store = ProfileStore::user_data()?;
    let selected = id.map(|id| store.load(id)).transpose()?;
    let mut controller = Controller::new(true, None)?;
    let capabilities = preset_fields::fields()
        .iter()
        .map(|f| {
            (
                f.feature,
                controller.capability(f.feature).access == Access::ReadWrite,
            )
        })
        .collect();
    let curve = controller.gpu_curve();
    let core_curve_conflict = curve.as_ref().is_ok_and(|curve| {
        curve
            .nodes
            .first()
            .is_some_and(|first| curve.nodes.iter().any(|p| p.delta_khz != first.delta_khz))
    });
    let removed_gpu_options = selected.as_ref().map_or(0, |p| {
        p.settings
            .iter()
            .filter(|s| preset_fields::REMOVED_GPU_OPTIONS.contains(&s.feature))
            .count()
    });
    let plan = selected
        .as_ref()
        .map(|p| transaction::plan(p, &mut controller, false));
    let startup_allowed = selected.as_ref().is_some_and(|p| {
        transaction::plan(p, &mut controller, true).applicable
            && store.startup_eligibility(&p.id).is_ok()
    });
    let selection = store.startup()?;
    let startup_enabled = selection.enabled
        && selected
            .as_ref()
            .is_some_and(|p| selection.profile_id.as_deref() == Some(&p.id));
    let startup_valid = startup_enabled
        && store.startup_profile().is_ok_and(|p| {
            p.as_ref()
                .is_some_and(|p| omen_platform::verify_startup_profile(p).is_ok())
        });
    Ok(View {
        profiles: store.list()?,
        selected,
        binding: controller.context()?.binding,
        capabilities,
        curve,
        removed_gpu_options,
        core_curve_conflict,
        pending: store.pending()?,
        applicable: plan.as_ref().is_some_and(|p| p.applicable),
        blockers: plan.map_or(0, |p| p.blockers.len()),
        startup_allowed,
        startup_enabled,
        startup_valid,
    })
}
struct SaveData {
    baseline: Option<PerformanceProfile>,
    binding: DeviceBinding,
    name: String,
    edits: Vec<(Feature, Option<Value>)>,
}
enum Request {
    Load(Option<String>),
    Save(Box<SaveData>),
    Apply(String),
    Recover,
    Delete(String),
    Startup(String, bool),
    Stop,
}
enum Update {
    View(Box<View>, String),
    Error(String),
}
fn worker(hwnd: usize, requests: mpsc::Receiver<Request>, updates: mpsc::Sender<Update>) {
    while let Ok(request) = requests.recv() {
        if matches!(request, Request::Stop) {
            break;
        }
        let result = (|| -> Result<Update, String> {
            match request {
                Request::Load(id) => {
                    Ok(Update::View(Box::new(load(id.as_deref())?), String::new()))
                }
                Request::Save(data) => {
                    let store = ProfileStore::user_data()?;
                    let mut profile = if let Some(base) = &data.baseline {
                        preset::merge(base, &store.load(&base.id)?, &data.name, &data.edits)?
                    } else {
                        let settings = data
                            .edits
                            .iter()
                            .filter_map(|(feature, value)| {
                                value.as_ref().map(|value| Setting {
                                    feature: *feature,
                                    value: value.clone(),
                                })
                            })
                            .collect();
                        PerformanceProfile {
                            schema: PROFILE_SCHEMA,
                            id: format!("preset-{}", ui::token()),
                            name: data.name.clone(),
                            binding: data.binding.clone(),
                            settings,
                        }
                    };
                    if data
                        .baseline
                        .as_ref()
                        .is_some_and(|p| p.binding.nvidia_uuid.is_none())
                        && profile.settings.iter().any(|s| {
                            s.feature.descriptor().category == omen_core::features::Category::Gpu
                        })
                    {
                        profile.binding.nvidia_uuid = data.binding.nvidia_uuid.clone();
                        profile.binding.nvidia_driver = data.binding.nvidia_driver.clone();
                        profile.binding.nvidia_vbios = data.binding.nvidia_vbios.clone();
                    }
                    profile.binding = profile.binding.for_settings(&profile.settings);
                    store.save(&profile)?;
                    Ok(Update::View(
                        Box::new(load(Some(&profile.id))?),
                        "Preset gespeichert.".into(),
                    ))
                }
                Request::Apply(id) => {
                    elevation::run(Some(&id), Action::Apply, || false)?;
                    Ok(Update::View(
                        Box::new(load(Some(&id))?),
                        "Preset angewendet.".into(),
                    ))
                }
                Request::Recover => {
                    elevation::run(None, Action::Restore, || false)?;
                    Ok(Update::View(
                        Box::new(load(None)?),
                        "Originalzustand wiederhergestellt.".into(),
                    ))
                }
                Request::Delete(id) => {
                    ProfileStore::user_data()?.delete(&id)?;
                    Ok(Update::View(
                        Box::new(load(None)?),
                        "Preset gelöscht.".into(),
                    ))
                }
                Request::Startup(id, enabled) => {
                    if enabled {
                        elevation::run(Some(&id), Action::Startup, || false)?;
                    } else {
                        ProfileStore::user_data()?.disable_startup()?;
                    }
                    Ok(Update::View(
                        Box::new(load(Some(&id))?),
                        if enabled {
                            "Windows-Start aktiviert."
                        } else {
                            "Windows-Start deaktiviert."
                        }
                        .into(),
                    ))
                }
                Request::Stop => unreachable!(),
            }
        })();
        if updates.send(result.unwrap_or_else(Update::Error)).is_err() {
            break;
        }
        unsafe {
            PostMessageW(hwnd as HWND, UPDATE, 0, 0);
        }
    }
}
enum Editor {
    Text(HWND),
    Choice(HWND),
    Boolean(HWND),
    Curve(HWND),
    PerCore(Vec<(u8, u8, HWND, HWND)>),
}
struct Row {
    field: Field,
    check: HWND,
    editor: Editor,
    note: HWND,
    edited: Cell<bool>,
    baseline: RefCell<Option<Value>>,
}
struct State {
    theme: theme::Theme,
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Update>,
    view: RefCell<Option<View>>,
    rows: RefCell<Vec<Row>>,
    groups: Cell<[HWND; 11]>,
    expanded: RefCell<[bool; 11]>,
    pane: Cell<HWND>,
    scroll: Cell<i32>,
    extent: Cell<i32>,
    scroll_drag: Cell<Option<(i32, i32)>>,
    rendering: Cell<bool>,
    busy: Cell<bool>,
    dirty: Cell<bool>,
    closing: Cell<bool>,
    curve: RefCell<Option<Value>>,
    preview: Option<PathBuf>,
}
fn checked(handle: HWND) -> bool {
    unsafe { SendMessageW(handle, BM_GETCHECK, 0, 0) == BST_CHECKED as isize }
}
fn set_checked(handle: HWND, value: bool) {
    unsafe {
        SendMessageW(
            handle,
            BM_SETCHECK,
            if value { BST_CHECKED } else { BST_UNCHECKED } as usize,
            0,
        );
    }
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
fn status(hwnd: HWND, _state: &State, message: &str, error: bool) {
    unsafe {
        let c = GetDlgItem(hwnd, STATUS);
        text(c, message);
        theme::error(c, error);
    }
}
fn enable(hwnd: HWND, state: &State) {
    unsafe {
        let view = state.view.borrow();
        let ready = view.is_some() && !state.busy.get();
        for id in [SELECT, NEW, NAME, SAVE] {
            EnableWindow(GetDlgItem(hwnd, id as i32), ready as i32);
        }
        let saved = view.as_ref().is_some_and(|v| v.selected.is_some());
        let pending = view.as_ref().is_some_and(|v| v.pending);
        EnableWindow(
            GetDlgItem(hwnd, DELETE as i32),
            (ready && saved && !view.as_ref().is_some_and(|v| v.pending)) as i32,
        );
        text(
            GetDlgItem(hwnd, APPLY as i32),
            if pending {
                "Wiederherstellen"
            } else {
                "Anwenden"
            },
        );
        EnableWindow(
            GetDlgItem(hwnd, APPLY as i32),
            (ready
                && (pending
                    || (saved
                        && !state.dirty.get()
                        && view.as_ref().is_some_and(|v| v.applicable)))) as i32,
        );
        EnableWindow(
            GetDlgItem(hwnd, STARTUP as i32),
            (ready
                && saved
                && !state.dirty.get()
                && view
                    .as_ref()
                    .is_some_and(|v| v.startup_allowed || v.startup_enabled)) as i32,
        );
        for row in state.rows.borrow().iter() {
            EnableWindow(row.check, ready as i32);
            let included = ready && checked(row.check);
            match &row.editor {
                Editor::Text(h) | Editor::Choice(h) | Editor::Boolean(h) => {
                    EnableWindow(*h, included as i32);
                }
                Editor::Curve(h) => {
                    EnableWindow(
                        *h,
                        (ready && view.as_ref().is_some_and(|v| v.curve.is_ok())) as i32,
                    );
                }
                Editor::PerCore(items) => {
                    for (_, _, _, h) in items {
                        EnableWindow(*h, included as i32);
                    }
                }
            }
        }
    }
}
fn dispatch(hwnd: HWND, state: &State, request: Request, message: &str) {
    if state.busy.replace(true) {
        return;
    }
    status(hwnd, state, message, false);
    enable(hwnd, state);
    if state.tx.send(request).is_err() {
        state.busy.set(false);
        status(hwnd, state, "Verbindung beendet. Fenster neu öffnen.", true);
        enable(hwnd, state);
    }
}
fn choices(handle: HWND, labels: &[&str]) {
    unsafe {
        for label in labels {
            SendMessageW(handle, CB_ADDSTRING, 0, wide(label).as_ptr() as isize);
        }
        SendMessageW(handle, CB_SETCURSEL, 0, 0);
    }
}
unsafe fn build_rows(hwnd: HWND, state: &State) {
    unsafe {
        let pane = state.pane.get();
        let mut rows = state.rows.borrow_mut();
        for (index, field) in preset_fields::fields().into_iter().enumerate() {
            let id = FIELD_BASE + index * FIELD_STRIDE;
            let label = if field.unit().is_empty() {
                field.label.to_string()
            } else {
                format!("{} ({})", field.label, field.unit())
            };
            let check = ui::control(
                pane,
                state.theme.body,
                id,
                "BUTTON",
                &label,
                WS_TABSTOP | BS_AUTOCHECKBOX as u32 | BS_NOTIFY as u32,
            );
            let editor = match field.feature.descriptor().value_kind {
                ValueKind::Choice => {
                    let h = ui::control(
                        pane,
                        state.theme.body,
                        id + 1,
                        "COMBOBOX",
                        "",
                        WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                    );
                    choices(
                        h,
                        &field
                            .choices()
                            .iter()
                            .map(|c| preset_fields::choice_label(*c))
                            .collect::<Vec<_>>(),
                    );
                    Editor::Choice(h)
                }
                ValueKind::Boolean => {
                    let h = ui::control(
                        pane,
                        state.theme.body,
                        id + 1,
                        "COMBOBOX",
                        "",
                        WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                    );
                    choices(h, &["Aus", "Ein"]);
                    Editor::Boolean(h)
                }
                ValueKind::VfCurve => Editor::Curve(ui::control(
                    pane,
                    state.theme.body,
                    id + 1,
                    "BUTTON",
                    "Bearbeiten …",
                    WS_TABSTOP,
                )),
                ValueKind::PerCore => {
                    let items = (0..16)
                        .map(|core| {
                            let ccd = core / 8;
                            let i = core % 8;
                            let label = ui::control(
                                pane,
                                state.theme.small,
                                id + 2 + core * 2,
                                "STATIC",
                                &format!("CCD {} · Kern {}", ccd + 1, i + 1),
                                0,
                            );
                            let h = ui::control(
                                pane,
                                state.theme.body,
                                id + 3 + core * 2,
                                "EDIT",
                                "0",
                                WS_TABSTOP | ES_AUTOHSCROLL as u32,
                            );
                            SendMessageW(h, EM_SETLIMITTEXT, 8, 0);
                            (ccd as u8, i as u8, label, h)
                        })
                        .collect();
                    Editor::PerCore(items)
                }
                _ => {
                    let h = ui::control(
                        pane,
                        state.theme.body,
                        id + 1,
                        "EDIT",
                        "",
                        WS_TABSTOP | ES_AUTOHSCROLL as u32,
                    );
                    SendMessageW(h, EM_SETLIMITTEXT, 200, 0);
                    Editor::Text(h)
                }
            };
            let note = ui::control(pane, state.theme.small, 0, "STATIC", "", 0);
            theme::muted(note);
            rows.push(Row {
                field,
                check,
                editor,
                note,
                edited: Cell::new(false),
                baseline: RefCell::new(None),
            });
        }
        let _ = hwnd;
    }
}
fn display(hwnd: HWND, state: &State, view: View, message: &str) {
    state.rendering.set(true);
    unsafe {
        let combo = GetDlgItem(hwnd, SELECT as i32);
        SendMessageW(combo, CB_RESETCONTENT, 0, 0);
        choices(combo, &["Neues Preset"]);
        let mut index = 0;
        for (i, profile) in view.profiles.iter().enumerate() {
            SendMessageW(
                combo,
                CB_ADDSTRING,
                0,
                wide(&profile.name).as_ptr() as isize,
            );
            if view.selected.as_ref().is_some_and(|p| p.id == profile.id) {
                index = i + 1;
            }
        }
        SendMessageW(combo, CB_SETCURSEL, index, 0);
        text(
            GetDlgItem(hwnd, NAME as i32),
            view.selected.as_ref().map_or("", |p| p.name.as_str()),
        );
        set_checked(GetDlgItem(hwnd, STARTUP as i32), view.startup_enabled);
        text(
            GetDlgItem(hwnd, STARTUP as i32),
            if view.startup_enabled && !view.startup_valid {
                "Bei Windows-Start (Freigabe ungültig)"
            } else {
                "Bei Windows-Start aktivieren"
            },
        );
    }
    *state.curve.borrow_mut() = view
        .selected
        .as_ref()
        .and_then(|p| {
            p.settings
                .iter()
                .find(|s| s.feature == Feature::GpuVfCurve)
                .map(|s| s.value.clone())
        })
        .or_else(|| {
            view.curve
                .as_ref()
                .ok()
                .and_then(|curve| curve.value().ok())
        });
    for row in state.rows.borrow().iter() {
        let stored = view.selected.as_ref().and_then(|p| {
            p.settings
                .iter()
                .find(|s| s.feature == row.field.feature)
                .map(|s| s.value.clone())
        });
        set_checked(row.check, stored.is_some());
        row.edited.set(false);
        *row.baseline.borrow_mut() = stored.clone();
        let value = stored.unwrap_or_else(|| row.field.initial());
        unsafe {
            match (&row.editor, &value) {
                (Editor::Text(h), Value::Integer(value)) => {
                    text(*h, &preset::format_number(*value, row.field.scale()))
                }
                (Editor::Text(h), Value::CoreRatios(values)) => text(
                    *h,
                    &values
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                (Editor::Choice(h), Value::Choice(value)) => {
                    let index = row
                        .field
                        .choices()
                        .iter()
                        .position(|v| v == value)
                        .unwrap_or(0);
                    SendMessageW(*h, CB_SETCURSEL, index, 0);
                }
                (Editor::Boolean(h), Value::Boolean(value)) => {
                    SendMessageW(*h, CB_SETCURSEL, usize::from(*value), 0);
                }
                (Editor::PerCore(items), Value::PerCore(points)) => {
                    for (ccd, core, _, h) in items {
                        text(
                            *h,
                            &points
                                .iter()
                                .find(|p| p.ccd == *ccd && p.core == *core)
                                .map_or(0, |p| p.steps)
                                .to_string(),
                        );
                    }
                }
                _ => {}
            }
        }
        text(
            row.note,
            if row.field.feature == Feature::GpuCoreOffsetMhz && view.core_curve_conflict {
                "V/F-Kurve aktiv"
            } else if view.capabilities.get(&row.field.feature) == Some(&true) {
                ""
            } else {
                "Noch nicht verfügbar"
            },
        );
    }
    let summary = if view.pending {
        "Unvollständige Änderung: Originalzustand wiederherstellen.".into()
    } else if view.removed_gpu_options > 0 {
        format!(
            "{} entfernte GPU-Optionen. Speichern bereinigt dieses Preset.",
            view.removed_gpu_options
        )
    } else if view.startup_enabled && !view.startup_valid {
        "Windows-Start ist ungültig: deaktivieren, Preset anwenden und erneut aktivieren.".into()
    } else if view.blockers > 0 {
        format!(
            "{} {} gewählte Einstellungen noch nicht verfügbar.",
            message, view.blockers
        )
        .trim()
        .to_string()
    } else if !message.is_empty() {
        message.to_string()
    } else if view.selected.is_none() {
        "Preset benennen und gewünschte Optionen auswählen.".into()
    } else {
        "Preset geladen.".into()
    };
    *state.view.borrow_mut() = Some(view);
    state.dirty.set(false);
    state.busy.set(false);
    state.rendering.set(false);
    status(hwnd, state, &summary, false);
    enable(hwnd, state);
    unsafe {
        layout_pane(state.pane.get(), state);
    }
}
fn collect(state: &State) -> Result<SaveData, String> {
    let view = state.view.borrow();
    let view = view.as_ref().ok_or("Preset wird noch geladen.")?;
    let hwnd = unsafe { GetParent(state.pane.get()) };
    let name = ui::value(unsafe { GetDlgItem(hwnd, NAME as i32) })
        .trim()
        .to_owned();
    if name.is_empty() {
        return Err("Preset-Namen eingeben.".into());
    }
    let mut edits = vec![];
    for row in state.rows.borrow().iter() {
        let value = if !checked(row.check) {
            None
        } else if !row.edited.get() && row.baseline.borrow().is_some() {
            row.baseline.borrow().clone()
        } else {
            Some(unsafe {
                match &row.editor {
                    Editor::Text(h) => {
                        if row.field.feature.descriptor().value_kind == ValueKind::CoreRatios {
                            Value::CoreRatios(
                                ui::value(*h)
                                    .split([',', ';', ' '])
                                    .filter(|s| !s.is_empty())
                                    .map(|s| {
                                        s.parse::<u8>().map_err(|_| "Gültige Core-Ratios eingeben.")
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                            )
                        } else {
                            Value::Integer(
                                preset::parse_number(&ui::value(*h), row.field.scale())
                                    .map_err(|e| format!("{}: {e}", row.field.label))?,
                            )
                        }
                    }
                    Editor::Choice(h) => {
                        let i = SendMessageW(*h, CB_GETCURSEL, 0, 0);
                        Value::Choice(
                            *row.field
                                .choices()
                                .get(i as usize)
                                .ok_or("Auswahl ungültig")?,
                        )
                    }
                    Editor::Boolean(h) => match SendMessageW(*h, CB_GETCURSEL, 0, 0) {
                        0 => Value::Boolean(false),
                        1 => Value::Boolean(true),
                        _ => return Err("Ein/Aus-Auswahl ungültig".into()),
                    },
                    Editor::Curve(_) => state
                        .curve
                        .borrow()
                        .clone()
                        .ok_or("GPU-Kurve konnte nicht gelesen werden.")?,
                    Editor::PerCore(items) => Value::PerCore(
                        items
                            .iter()
                            .map(|(ccd, core, _, h)| {
                                Ok(CoreDelta {
                                    ccd: *ccd,
                                    core: *core,
                                    steps: ui::value(*h).parse().map_err(
                                        |_| "Gültige Curve-Optimizer-Schritte eingeben.",
                                    )?,
                                })
                            })
                            .collect::<Result<Vec<_>, String>>()?,
                    ),
                }
            })
        };
        if let Some(value) = &value {
            Setting {
                feature: row.field.feature,
                value: value.clone(),
            }
            .validate()?;
        }
        edits.push((row.field.feature, value));
    }
    // Explicit Save retires obsolete instructions from older presets. Other
    // hidden settings remain untouched, and merge still rejects concurrent edits.
    if let Some(profile) = &view.selected {
        edits.extend(
            profile
                .settings
                .iter()
                .filter(|s| preset_fields::REMOVED_GPU_OPTIONS.contains(&s.feature))
                .map(|s| (s.feature, None)),
        );
    }
    Ok(SaveData {
        baseline: view.selected.clone(),
        binding: view.binding.clone(),
        name,
        edits,
    })
}
unsafe fn show(handle: HWND, visible: bool) {
    unsafe {
        ShowWindow(handle, if visible { SW_SHOW } else { SW_HIDE });
    }
}
unsafe fn layout_pane(hwnd: HWND, state: &State) {
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect);
        let w = rect.right;
        let h = rect.bottom;
        let mut y = 0;
        let scroll = state.scroll.get();
        for group in 0..GROUPS.len() {
            if group == 0 || group == 1 {
                let heading = GetDlgItem(hwnd, if group == 0 { 720 } else { 721 });
                ui::place(heading, 4, y - scroll, w - 20, 32);
                y += 40;
            }
            let header = state.groups.get()[group];
            ui::place(header, 0, y - scroll, w - 18, 48);
            y += 56;
            let expanded = state.expanded.borrow()[group];
            for row in state
                .rows
                .borrow()
                .iter()
                .filter(|r| r.field.group == group)
            {
                show(row.check, expanded);
                show(row.note, expanded);
                if expanded {
                    ui::place(row.check, 16, y - scroll, w - 390, 32);
                    ui::place(row.note, w - 370, y - scroll + 7, 156, 24);
                }
                match &row.editor {
                    Editor::Text(input)
                    | Editor::Choice(input)
                    | Editor::Boolean(input)
                    | Editor::Curve(input) => {
                        show(*input, expanded);
                        if expanded {
                            ui::place(
                                *input,
                                w - 202,
                                y - scroll,
                                176,
                                if matches!(row.editor, Editor::Choice(_) | Editor::Boolean(_)) {
                                    280
                                } else {
                                    32
                                },
                            );
                        }
                    }
                    Editor::PerCore(items) => {
                        for (ccd, core, label, input) in items {
                            show(*label, expanded);
                            show(*input, expanded);
                            if expanded {
                                let col = *ccd as i32;
                                let column = (w - 56) / 2;
                                let x = 18 + col * column;
                                let top = y + 42 + *core as i32 * 36 - scroll;
                                ui::place(*label, x, top + 5, column - 104, 25);
                                ui::place(*input, x + column - 100, top, 78, 29);
                            }
                        }
                    }
                }
                if expanded {
                    y += if matches!(row.editor, Editor::PerCore(_)) {
                        342
                    } else {
                        44
                    };
                }
            }
            if expanded {
                y += 12;
            }
        }
        state.extent.set(y);
        let maximum = (y - h).max(0);
        if state.scroll.get() > maximum {
            state.scroll.set(maximum);
            layout_pane(hwnd, state);
            return;
        }
        InvalidateRect(hwnd, ptr::null(), 0);
    }
}
unsafe fn layout(hwnd: HWND, state: &State) {
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        let w = r.right;
        let h = r.bottom;
        ui::place(GetDlgItem(hwnd, 730), 28, 20, w - 56, 40);
        ui::place(GetDlgItem(hwnd, TAB as i32), 28, 72, 174, 36);
        ui::place(GetDlgItem(hwnd, 731), 28, 125, 270, 22);
        ui::place(GetDlgItem(hwnd, SELECT as i32), 28, 150, 268, 320);
        ui::place(GetDlgItem(hwnd, NEW as i32), 308, 150, 112, 34);
        ui::place(GetDlgItem(hwnd, 732), 444, 125, w - 472, 22);
        ui::place(GetDlgItem(hwnd, NAME as i32), 444, 150, w - 472, 34);
        ui::place(state.pane.get(), 28, 208, w - 56, h - 348);
        ui::place(GetDlgItem(hwnd, STARTUP as i32), 28, h - 115, 360, 30);
        ui::place(GetDlgItem(hwnd, STATUS), 28, h - 67, w - 470, 48);
        for (id, offset) in [(APPLY, 422), (SAVE, 288), (DELETE, 154)] {
            ui::place(GetDlgItem(hwnd, id as i32), w - offset, h - 65, 126, 40);
        }
        layout_pane(state.pane.get(), state);
    }
}
unsafe fn scroll(hwnd: HWND, state: &State, delta: i32) {
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        state
            .scroll
            .set((state.scroll.get() + delta).clamp(0, (state.extent.get() - r.bottom).max(0)));
        layout_pane(hwnd, state);
    }
}
// The scrolling content keeps native child controls and keyboard focus. Only
// the track is painted here so the OS's classic white scrollbar cannot obscure
// the dark settings surface. Wheel, page clicks and thumb dragging share bounds.
unsafe fn scroll_thumb(hwnd: HWND, state: &State) -> RECT {
    unsafe {
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut client);
        let height = client.bottom.max(0);
        let extent = state.extent.get().max(height).max(1);
        let thumb = ((i64::from(height) * i64::from(height) / i64::from(extent)) as i32)
            .max(34)
            .min(height);
        let maximum = (extent - height).max(1);
        let top =
            (i64::from(state.scroll.get()) * i64::from(height - thumb) / i64::from(maximum)) as i32;
        RECT {
            left: client.right - 12,
            top,
            right: client.right - 4,
            bottom: top + thumb,
        }
    }
}
unsafe fn paint_pane(hwnd: HWND, state: &State, dc: HDC) {
    unsafe {
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut client);
        ui::fill(dc, &client, state.theme.bg());
        if state.extent.get() > client.bottom {
            let thumb = scroll_thumb(hwnd, state);
            let track = RECT {
                left: thumb.left,
                top: 0,
                right: thumb.right,
                bottom: client.bottom,
            };
            ui::fill(dc, &track, state.theme.panel_color());
            ui::fill(dc, &thumb, state.theme.muted());
        }
    }
}
unsafe fn group_draw(state: &State, item: &DRAWITEMSTRUCT) {
    unsafe {
        let group = item.CtlID as usize - GROUP_BASE;
        ui::fill(
            item.hDC,
            &item.rcItem,
            if item.itemState & ODS_SELECTED != 0 {
                state.theme.field_color()
            } else {
                state.theme.panel_color()
            },
        );
        let mut label = RECT {
            left: 16,
            top: 0,
            right: item.rcItem.right - 44,
            bottom: item.rcItem.bottom,
        };
        ui::draw_text(
            item.hDC,
            state.theme.body,
            &mut label,
            GROUPS[group],
            state.theme.text(),
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        let x = item.rcItem.right - 25;
        let y = item.rcItem.bottom / 2;
        let pen = CreatePen(PS_SOLID, 2, state.theme.muted());
        let old = SelectObject(item.hDC, pen);
        let direction = if state.expanded.borrow()[group] {
            -1
        } else {
            1
        };
        MoveToEx(item.hDC, x - 5, y - direction * 2, ptr::null_mut());
        LineTo(item.hDC, x, y + direction * 3);
        LineTo(item.hDC, x + 5, y - direction * 2);
        SelectObject(item.hDC, old);
        DeleteObject(pen);
        if item.itemState & ODS_FOCUS != 0 {
            let mut focus = item.rcItem;
            InflateRect(&mut focus, -3, -3);
            DrawFocusRect(item.hDC, &focus);
        }
    }
}
unsafe extern "system" fn pane_proc(hwnd: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if m == WM_NCCREATE {
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                (*(l as *const CREATESTRUCTW)).lpCreateParams as isize,
            );
        }
        let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
        if raw.is_null() {
            return DefWindowProcW(hwnd, m, w, l);
        }
        let state = &*raw;
        if let Some(color) = state.theme.color(m, w, l) {
            return color;
        }
        match m {
            WM_ERASEBKGND | WM_PRINTCLIENT => {
                paint_pane(hwnd, state, w as HDC);
                1
            }
            WM_PAINT => {
                let mut paint: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(hwnd, &mut paint);
                paint_pane(hwnd, state, dc);
                EndPaint(hwnd, &paint);
                0
            }
            WM_SIZE => {
                layout_pane(hwnd, state);
                0
            }
            WM_VSCROLL => {
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                let delta = match (w & 0xffff) as i32 {
                    SB_LINEUP => -44,
                    SB_LINEDOWN => 44,
                    SB_PAGEUP => -client.bottom,
                    SB_PAGEDOWN => client.bottom,
                    SB_THUMBTRACK | SB_THUMBPOSITION => (w >> 16) as i32 - state.scroll.get(),
                    SB_TOP => -state.scroll.get(),
                    SB_BOTTOM => state.extent.get(),
                    _ => 0,
                };
                scroll(hwnd, state, delta);
                0
            }
            WM_LBUTTONDOWN => {
                let x = l as u16 as i16 as i32;
                let y = (l >> 16) as u16 as i16 as i32;
                let mut client: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut client);
                if x >= client.right - 18 && state.extent.get() > client.bottom {
                    let thumb = scroll_thumb(hwnd, state);
                    if y >= thumb.top && y < thumb.bottom {
                        state.scroll_drag.set(Some((y, state.scroll.get())));
                        SetCapture(hwnd);
                    } else {
                        scroll(
                            hwnd,
                            state,
                            if y < thumb.top {
                                -client.bottom
                            } else {
                                client.bottom
                            },
                        );
                    }
                }
                0
            }
            WM_MOUSEMOVE => {
                if let Some((origin, initial)) = state.scroll_drag.get() {
                    let mut client: RECT = std::mem::zeroed();
                    GetClientRect(hwnd, &mut client);
                    let thumb = scroll_thumb(hwnd, state);
                    let travel = (client.bottom - (thumb.bottom - thumb.top)).max(1);
                    let maximum = (state.extent.get() - client.bottom).max(0);
                    let y = (l >> 16) as u16 as i16 as i32;
                    let target = initial
                        + ((i64::from(y - origin) * i64::from(maximum)) / i64::from(travel)) as i32;
                    scroll(hwnd, state, target - state.scroll.get());
                }
                0
            }
            WM_LBUTTONUP => {
                if state.scroll_drag.take().is_some() {
                    ReleaseCapture();
                }
                0
            }
            WM_CAPTURECHANGED => {
                state.scroll_drag.set(None);
                0
            }
            WM_MOUSEWHEEL => {
                scroll(hwnd, state, -((w >> 16) as u16 as i16 as i32) / 120 * 108);
                0
            }
            WM_DRAWITEM => {
                let item = &*(l as *const DRAWITEMSTRUCT);
                if (GROUP_BASE..GROUP_BASE + 11).contains(&(item.CtlID as usize)) {
                    group_draw(state, item);
                } else {
                    state.theme.button(item, state.theme.body, false);
                }
                1
            }
            WM_COMMAND => {
                let notification = (w >> 16) & 0xffff;
                if [
                    EN_SETFOCUS as usize,
                    BN_SETFOCUS as usize,
                    CBN_SETFOCUS as usize,
                ]
                .contains(&notification)
                    && GetFocus() == l as HWND
                {
                    let mut r = std::mem::zeroed();
                    GetWindowRect(l as HWND, &mut r);
                    MapWindowPoints(ptr::null_mut(), hwnd, (&mut r as *mut RECT).cast(), 2);
                    let mut client = std::mem::zeroed();
                    GetClientRect(hwnd, &mut client);
                    if r.top < 0 {
                        scroll(hwnd, state, r.top - 8);
                    } else if r.bottom > client.bottom {
                        scroll(hwnd, state, r.bottom - client.bottom + 8);
                    }
                }
                SendMessageW(GetParent(hwnd), m, w, l);
                0
            }
            _ => DefWindowProcW(hwnd, m, w, l),
        }
    }
}
fn dirty(hwnd: HWND, state: &State) {
    state.dirty.set(true);
    status(hwnd, state, "Ungespeicherte Änderungen.", false);
    enable(hwnd, state);
}
fn edit_gpu_curve(hwnd: HWND, state: &State) {
    let draft = (|| {
        let view = state.view.borrow();
        let view = view.as_ref().ok_or("Preset wird noch geladen.")?;
        let curve = Curve::gpu(view.curve.clone()?, state.curve.borrow().as_ref())?;
        Ok::<_, String>((curve, view.binding.clone()))
    })();
    let (curve, binding) = match draft {
        Ok(draft) => draft,
        Err(error) => {
            status(hwnd, state, &error, true);
            return;
        }
    };
    // Keep the parent draft stable while the owned chart has protected focus.
    // The curve editor receives only an in-memory model, never a profile writer.
    state.busy.set(true);
    unsafe {
        EnableWindow(hwnd, 0);
    }
    let result = crate::curve_ui::edit_gpu_draft(hwnd, curve, binding);
    unsafe {
        EnableWindow(hwnd, 1);
    }
    state.busy.set(false);
    match result {
        Ok(Some(value)) => {
            *state.curve.borrow_mut() = Some(value);
            for row in state.rows.borrow().iter() {
                match row.field.feature {
                    Feature::GpuVfCurve => {
                        set_checked(row.check, true);
                        row.edited.set(true);
                    }
                    Feature::GpuCoreOffsetMhz => {
                        set_checked(row.check, false);
                        row.edited.set(true);
                    }
                    _ => {}
                }
            }
            dirty(hwnd, state);
            status(
                hwnd,
                state,
                "Kurve übernommen. Preset noch nicht gespeichert.",
                false,
            );
        }
        Ok(None) => status(
            hwnd,
            state,
            if state.dirty.get() {
                "Kurvenänderungen verworfen. Preset-Entwurf bleibt ungespeichert."
            } else {
                "Kurvenbearbeitung verworfen."
            },
            false,
        ),
        Err(error) => status(hwnd, state, &error, true),
    }
    enable(hwnd, state);
    unsafe {
        if state.closing.get() {
            DestroyWindow(hwnd);
        } else {
            SetActiveWindow(hwnd);
            if let Some(row) = state
                .rows
                .borrow()
                .iter()
                .find(|row| row.field.feature == Feature::GpuVfCurve)
            {
                if let Editor::Curve(button) = row.editor {
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(button);
                }
            }
        }
    }
}
fn discard(hwnd: HWND, state: &State) -> bool {
    !state.dirty.get()
        || unsafe {
            MessageBoxW(
                hwnd,
                wide("Ungespeicherte Änderungen verwerfen?").as_ptr(),
                wide("Preset wechseln").as_ptr(),
                MB_YESNO | MB_ICONQUESTION,
            )
        } == IDYES
}
unsafe extern "system" fn procedure(hwnd: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if m == WM_NCCREATE {
            SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                (*(l as *const CREATESTRUCTW)).lpCreateParams as isize,
            );
        }
        let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
        if raw.is_null() {
            return DefWindowProcW(hwnd, m, w, l);
        }
        let state = &*raw;
        if let Some(color) = state.theme.color(m, w, l) {
            return color;
        }
        match m {
            WM_CREATE => {
                state.theme.frame(hwnd);
                ui::control(
                    hwnd,
                    state.theme.title,
                    730,
                    "STATIC",
                    "Leistungsprofile",
                    0,
                );
                ui::control(
                    hwnd,
                    state.theme.body,
                    TAB,
                    "BUTTON",
                    "Custom Presets",
                    WS_TABSTOP,
                );
                ui::control(hwnd, state.theme.small, 731, "STATIC", "Preset", 0);
                ui::control(hwnd, state.theme.small, 732, "STATIC", "Name", 0);
                ui::control(
                    hwnd,
                    state.theme.body,
                    SELECT,
                    "COMBOBOX",
                    "",
                    WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                );
                ui::control(hwnd, state.theme.body, NEW, "BUTTON", "Neu", WS_TABSTOP);
                let name = ui::control(
                    hwnd,
                    state.theme.body,
                    NAME,
                    "EDIT",
                    "",
                    WS_TABSTOP | ES_AUTOHSCROLL as u32,
                );
                SendMessageW(name, EM_SETLIMITTEXT, 120, 0);
                let pane = CreateWindowExW(
                    WS_EX_CONTROLPARENT,
                    wide(PANE_CLASS).as_ptr(),
                    wide("").as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
                    0,
                    0,
                    10,
                    10,
                    hwnd,
                    PANE as HMENU,
                    windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(ptr::null()),
                    raw as *const core::ffi::c_void,
                );
                state.pane.set(pane);
                ui::control(pane, state.theme.heading, 720, "STATIC", "GPU", 0);
                ui::control(
                    pane,
                    state.theme.heading,
                    721,
                    "STATIC",
                    "CPU und System",
                    0,
                );
                state.groups.set(std::array::from_fn(|i| {
                    ui::control(
                        pane,
                        state.theme.body,
                        GROUP_BASE + i,
                        "BUTTON",
                        GROUPS[i],
                        WS_TABSTOP | BS_OWNERDRAW as u32 | BS_NOTIFY as u32,
                    )
                }));
                build_rows(hwnd, state);
                ui::control(
                    hwnd,
                    state.theme.body,
                    STARTUP,
                    "BUTTON",
                    "Bei Windows-Start aktivieren",
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                );
                ui::control(
                    hwnd,
                    state.theme.small,
                    STATUS as usize,
                    "STATIC",
                    "Presets werden geladen …",
                    0,
                );
                for (id, label) in [
                    (APPLY, "Anwenden"),
                    (SAVE, "Speichern"),
                    (DELETE, "Löschen"),
                ] {
                    ui::control(hwnd, state.theme.body, id, "BUTTON", label, WS_TABSTOP);
                }
                layout(hwnd, state);
                enable(hwnd, state);
                0
            }
            WM_ERASEBKGND | WM_PRINTCLIENT => {
                let mut r = std::mem::zeroed();
                GetClientRect(hwnd, &mut r);
                ui::fill(w as HDC, &r, state.theme.bg());
                1
            }
            WM_SIZE => {
                layout(hwnd, state);
                0
            }
            WM_GETMINMAXINFO => {
                (*(l as *mut MINMAXINFO)).ptMinTrackSize = POINT { x: 920, y: 720 };
                0
            }
            WM_DRAWITEM => {
                state.theme.button(
                    &*(l as *const DRAWITEMSTRUCT),
                    state.theme.body,
                    w == APPLY || w == TAB,
                );
                1
            }
            WM_MOUSEWHEEL => {
                SendMessageW(state.pane.get(), m, w, l);
                0
            }
            WM_COMMAND => {
                let id = w & 0xffff;
                let notification = (w >> 16) & 0xffff;
                if state.rendering.get() || state.busy.get() {
                    return 0;
                }
                if (GROUP_BASE..GROUP_BASE + 11).contains(&id)
                    && notification == BN_CLICKED as usize
                {
                    let group = id - GROUP_BASE;
                    state.expanded.borrow_mut()[group] ^= true;
                    layout_pane(state.pane.get(), state);
                    return 0;
                }
                if id >= FIELD_BASE {
                    let index = (id - FIELD_BASE) / FIELD_STRIDE;
                    let offset = (id - FIELD_BASE) % FIELD_STRIDE;
                    let is_curve = state
                        .rows
                        .borrow()
                        .get(index)
                        .is_some_and(|row| matches!(row.editor, Editor::Curve(_)));
                    if is_curve && offset == 1 && notification == BN_CLICKED as usize {
                        edit_gpu_curve(hwnd, state);
                        return 0;
                    }
                    if let Some(row) = state.rows.borrow().get(index) {
                        if notification == EN_CHANGE as usize
                            || notification == CBN_SELCHANGE as usize
                            || (offset == 0 && notification == BN_CLICKED as usize)
                        {
                            row.edited.set(true);
                            if checked(row.check)
                                && matches!(
                                    row.field.feature,
                                    Feature::GpuVfCurve | Feature::GpuCoreOffsetMhz
                                )
                            {
                                let other = if row.field.feature == Feature::GpuVfCurve {
                                    Feature::GpuCoreOffsetMhz
                                } else {
                                    Feature::GpuVfCurve
                                };
                                for other_row in state
                                    .rows
                                    .borrow()
                                    .iter()
                                    .filter(|r| r.field.feature == other)
                                {
                                    set_checked(other_row.check, false);
                                    other_row.edited.set(true);
                                }
                            }
                            dirty(hwnd, state);
                        }
                    }
                    return 0;
                }
                if id == NAME && notification == EN_CHANGE as usize {
                    dirty(hwnd, state);
                    return 0;
                }
                if id == SELECT && notification == CBN_SELCHANGE as usize {
                    if discard(hwnd, state) {
                        let i = SendMessageW(GetDlgItem(hwnd, SELECT as i32), CB_GETCURSEL, 0, 0);
                        let id = state
                            .view
                            .borrow()
                            .as_ref()
                            .and_then(|v| v.profiles.get((i as usize).wrapping_sub(1)))
                            .map(|p| p.id.clone());
                        dispatch(hwnd, state, Request::Load(id), "Preset wird geladen …");
                    } else {
                        // CBN_SELCHANGE arrives after the native selector has
                        // moved. Keep its identity aligned with the retained
                        // draft when the user declines discarding that draft.
                        let view = state.view.borrow();
                        let current = view.as_ref().map_or(0, |v| {
                            v.selected
                                .as_ref()
                                .and_then(|p| v.profiles.iter().position(|q| q.id == p.id))
                                .map_or(0, |i| i + 1)
                        });
                        SendMessageW(GetDlgItem(hwnd, SELECT as i32), CB_SETCURSEL, current, 0);
                    }
                    return 0;
                }
                if notification != BN_CLICKED as usize {
                    return 0;
                }
                match id {
                    NEW => {
                        if discard(hwnd, state) {
                            dispatch(hwnd, state, Request::Load(None), "Neues Preset …");
                        }
                    }
                    SAVE => match collect(state) {
                        Ok(data) => dispatch(
                            hwnd,
                            state,
                            Request::Save(Box::new(data)),
                            "Preset wird gespeichert …",
                        ),
                        Err(e) => status(hwnd, state, &e, true),
                    },
                    APPLY => {
                        if state.view.borrow().as_ref().is_some_and(|v| v.pending) {
                            dispatch(
                                hwnd,
                                state,
                                Request::Recover,
                                "Originalzustand wird wiederhergestellt …",
                            );
                        } else if !state.dirty.get() {
                            if let Some(id) = selected(state) {
                                dispatch(hwnd, state, Request::Apply(id), "Windows-Freigabe …");
                            }
                        }
                    }
                    DELETE => {
                        if let Some(id) = selected(state) {
                            dispatch(hwnd, state, Request::Delete(id), "Preset wird gelöscht …");
                        }
                    }
                    STARTUP => {
                        if let Some(id) = selected(state) {
                            dispatch(
                                hwnd,
                                state,
                                Request::Startup(id, checked(GetDlgItem(hwnd, STARTUP as i32))),
                                "Windows-Start wird aktualisiert …",
                            );
                        }
                    }
                    _ => {}
                }
                0
            }
            UPDATE => {
                while let Ok(update) = state.rx.try_recv() {
                    match update {
                        Update::View(view, message) => display(hwnd, state, *view, &message),
                        Update::Error(error) => {
                            state.busy.set(false);
                            status(hwnd, state, &error, true);
                            enable(hwnd, state);
                        }
                    }
                }
                if let Some(path) = &state.preview {
                    if !state.busy.get() {
                        let result = crate::curve_widget::render_window(hwnd, path);
                        if let Err(error) = result {
                            status(hwnd, state, &error, true);
                        }
                        DestroyWindow(hwnd);
                    }
                }
                if state.closing.get() && !state.busy.get() {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_CLOSE => {
                if state.busy.get() {
                    state.closing.set(true);
                    status(hwnd, state, "Vorgang wird abgeschlossen …", false);
                } else if state.preview.is_some() || discard(hwnd, state) {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_DESTROY => {
                let _ = state.tx.send(Request::Stop);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, m, w, l),
        }
    }
}
pub fn run(id: Option<String>) -> Result<(), String> {
    run_window(id, None, None, None)
}
pub fn preview(path: &Path, group: Option<usize>, size: Option<[i32; 2]>) -> Result<(), String> {
    run_window(None, Some(path.to_owned()), group, size)
}
fn run_window(
    id: Option<String>,
    preview: Option<PathBuf>,
    preview_group: Option<usize>,
    size: Option<[i32; 2]>,
) -> Result<(), String> {
    let [width, height] = size.unwrap_or([1100, 900]);
    if !(920..=2560).contains(&width) || !(720..=1600).contains(&height) {
        return Err("Preset preview dimensions outside supported capture bounds".into());
    }
    if let Some(id) = &id {
        omen_core::profiles::validate_id(id)?;
    }
    let (tx, requests) = mpsc::channel();
    let (updates, rx) = mpsc::channel();
    let mut expanded = [false; 11];
    if let Some(group) = preview_group.filter(|g| *g < 11) {
        expanded[group] = true;
    }
    let state = Box::new(State {
        theme: theme::Theme::new(),
        tx,
        rx,
        view: RefCell::new(None),
        rows: RefCell::new(vec![]),
        groups: Cell::new([ptr::null_mut(); 11]),
        expanded: RefCell::new(expanded),
        pane: Cell::new(ptr::null_mut()),
        scroll: Cell::new(0),
        extent: Cell::new(0),
        scroll_drag: Cell::new(None),
        rendering: Cell::new(true),
        busy: Cell::new(false),
        dirty: Cell::new(false),
        closing: Cell::new(false),
        curve: RefCell::new(None),
        preview,
    });
    unsafe {
        ui::register(CLASS, Some(procedure))?;
        ui::register(PANE_CLASS, Some(pane_proc))?;
        let hwnd = ui::create(
            CLASS,
            "Omen Hub++ – Leistungsprofile",
            width,
            height,
            (&*state as *const State).cast_mut().cast(),
            state.preview.is_some(),
        )?;
        state.rendering.set(false);
        ShowWindow(hwnd, SW_SHOW);
        let handle = hwnd as usize;
        let thread = thread::spawn(move || worker(handle, requests, updates));
        dispatch(hwnd, &state, Request::Load(id), "Presets werden geladen …");
        ui::messages(hwnd);
        let _ = state.tx.send(Request::Stop);
        if thread.is_finished() {
            let _ = thread.join();
        }
    }
    Ok(())
}
