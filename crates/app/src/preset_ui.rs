//! Native custom presets. Declarative fields, explicit save/apply and no tuners
//! in UI callbacks. Unsupported settings remain editable metadata, never writes.
use crate::{
    elevation::{self, Action},
    i18n::{t, tf},
    native_ui::{self as ui, text, wide},
    preset_fields::{self, Field, GROUPS, SECTIONS},
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

pub(crate) const UPDATE: u32 = WM_APP + 31;
const SELECT: usize = 701;
const NEW: usize = 702;
const NAME: usize = 703;
const SAVE: usize = 704;
const APPLY: usize = 705;
const DELETE: usize = 706;
const STARTUP: usize = 707;
const TAB: usize = 708;
/// Draft back to the system defaults.
const RESET: usize = 709;
const GROUP_BASE: usize = 1200;
const FIELD_BASE: usize = 2000;
const PANE: usize = 710;
const STATUS: i32 = 711;
const FIELD_STRIDE: usize = 64;
const FIELD_HEIGHT: i32 = 36;
/// Child-id offset of a row's info button inside its id block.
const INFO_OFFSET: usize = FIELD_STRIDE - 1;
const INFO_SIZE: i32 = 26;
pub const CLASS: &str = "OmenHubPPPresetEditor";
const PANE_CLASS: &str = "OmenHubPPPresetFields";
pub(crate) struct View {
    pub(crate) profiles: Vec<PerformanceProfile>,
    pub(crate) selected: Option<PerformanceProfile>,
    pub(crate) binding: DeviceBinding,
    /// This device's own values, shown greyed and used by Reset.
    pub(crate) defaults: BTreeMap<Feature, Value>,
    pub(crate) curve: Result<omen_core::vf::VfState, String>,
    pub(crate) removed_options: usize,
    pub(crate) pending: bool,
    pub(crate) applicable: bool,
    pub(crate) blockers: Vec<String>,
    pub(crate) startup_enabled: bool,
    pub(crate) startup_valid: bool,
}
fn load(id: Option<&str>) -> Result<View, String> {
    let store = ProfileStore::user_data()?;
    let selected = id.map(|id| store.load(id)).transpose()?;
    let mut controller = Controller::new(true, None)?;
    let capabilities: BTreeMap<Feature, bool> = preset_fields::fields()
        .iter()
        .map(|f| {
            (
                f.feature,
                controller.capability(f.feature).access == Access::ReadWrite,
            )
        })
        .collect();
    let binding = controller.context()?.binding;
    // Record the firmware's own values only while untouched since boot; a
    // preset, mode or other control may have changed them afterwards.
    if !store
        .controlled_since(omen_platform::boot_unix_s())
        .unwrap_or(true)
    {
        let readings: Vec<Setting> = preset_fields::DEVICE_DEFAULTS
            .iter()
            .filter(|f| capabilities.get(f) == Some(&true))
            .filter_map(|f| {
                controller
                    .read(*f)
                    .ok()
                    .map(|value| Setting { feature: *f, value })
            })
            .collect();
        let _ = store.save_device_defaults(&binding.board, &binding.bios, &readings);
    }
    let defaults = store
        .device_defaults(&binding.board, &binding.bios)
        .into_iter()
        .map(|s| (s.feature, s.value))
        .collect();
    let curve = controller.gpu_curve();
    let removed_options = selected.as_ref().map_or(0, |p| {
        p.settings
            .iter()
            .filter(|s| preset_fields::retired(s.feature))
            .count()
    });
    let plan = selected
        .as_ref()
        .map(|p| transaction::plan(p, &mut controller, false));
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
        binding,
        defaults,
        curve,
        removed_options,
        pending: store.pending()?,
        applicable: plan.as_ref().is_some_and(|p| p.applicable),
        blockers: plan.map_or_else(Vec::new, |p| {
            p.blockers.into_iter().map(|b| b.reason).collect()
        }),
        startup_enabled,
        startup_valid,
    })
}
pub(crate) struct SaveData {
    pub(crate) baseline: Option<PerformanceProfile>,
    pub(crate) binding: DeviceBinding,
    pub(crate) name: String,
    pub(crate) edits: Vec<(Feature, Option<Value>)>,
    /// Autostart chosen in the draft; `None` leaves it unchanged.
    pub(crate) startup: Option<bool>,
}
pub(crate) enum Request {
    List,
    Load(Option<String>),
    Save(Box<SaveData>),
    Apply(String),
    Recover,
    Delete(String),
    Startup(String, bool),
    Stop,
}
pub(crate) enum Update {
    List(
        Result<
            (
                Vec<PerformanceProfile>,
                Option<omen_core::battery::Selection>,
                bool,
            ),
            String,
        >,
    ),
    View(Box<View>, String),
    Error(String),
}
pub(crate) fn worker(
    hwnd: usize,
    requests: mpsc::Receiver<Request>,
    updates: mpsc::Sender<Update>,
) {
    while let Ok(request) = requests.recv() {
        if matches!(request, Request::Stop) {
            break;
        }
        if matches!(request, Request::List) {
            let result = (|| {
                let store = ProfileStore::user_data()?;
                Ok((store.list()?, store.ac_selection(), store.pending()?))
            })();
            if updates.send(Update::List(result)).is_err() {
                break;
            }
            unsafe {
                PostMessageW(hwnd as HWND, UPDATE, 0, 0);
            }
            continue;
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
                        && profile.settings.iter().any(|s| s.feature.needs_nvidia())
                    {
                        profile.binding.nvidia_uuid = data.binding.nvidia_uuid.clone();
                        profile.binding.nvidia_driver = data.binding.nvidia_driver.clone();
                        profile.binding.nvidia_vbios = data.binding.nvidia_vbios.clone();
                    }
                    profile.binding = profile.binding.for_settings(&profile.settings);
                    store.save(&profile)?;
                    // A changed sign-in preset is applied and approved again
                    // instead of failing at the next sign-in (user rule, 30
                    // October 2026).
                    let startup = data.startup.or_else(|| {
                        let selection = store.startup().ok()?;
                        let digest = omen_platform::profile_store::profile_digest(&profile).ok();
                        (selection.enabled
                            && selection.profile_id.as_deref() == Some(profile.id.as_str())
                            && selection.profile_digest != digest)
                            .then_some(true)
                    });
                    // Autostart ticked while creating or editing: apply the saved
                    // preset, then enable it, with the existing approved commands.
                    let message = match startup {
                        Some(true) => {
                            let mut controller = Controller::new(true, None)?;
                            if !transaction::plan(&profile, &mut controller, true).applicable {
                                return Err(t("preset.startup_blocked").into());
                            }
                            elevation::run_serial(Some(&profile.id), Action::Replace)?;
                            elevation::run(Some(&profile.id), Action::Startup, || false)?;
                            t("preset.saved_startup")
                        }
                        Some(false) => {
                            store.disable_startup()?;
                            t("preset.saved")
                        }
                        None => t("preset.saved"),
                    };
                    Ok(Update::View(
                        Box::new(load(Some(&profile.id))?),
                        message.into(),
                    ))
                }
                Request::Apply(id) => {
                    let record = elevation::run(Some(&id), Action::Apply, || false)?;
                    Ok(Update::View(
                        Box::new(load(Some(&id))?),
                        elevation::applied_status(&record),
                    ))
                }
                Request::Recover => {
                    elevation::run(None, Action::Restore, || false)?;
                    Ok(Update::View(
                        Box::new(load(None)?),
                        t("preset.restored").into(),
                    ))
                }
                Request::Delete(id) => {
                    ProfileStore::user_data()?.delete(&id)?;
                    Ok(Update::View(
                        Box::new(load(None)?),
                        t("preset.deleted").into(),
                    ))
                }
                Request::Startup(id, enabled) => {
                    if enabled {
                        // Not applied in this exact revision yet: apply it first.
                        let store = ProfileStore::user_data()?;
                        if store.startup_eligibility(&id).is_err() {
                            let mut controller = Controller::new(true, None)?;
                            if !transaction::plan(&store.load(&id)?, &mut controller, true)
                                .applicable
                            {
                                return Err(t("preset.startup_blocked").into());
                            }
                            elevation::run_serial(Some(&id), Action::Replace)?;
                        }
                        elevation::run(Some(&id), Action::Startup, || false)?;
                    } else {
                        ProfileStore::user_data()?.disable_startup()?;
                    }
                    Ok(Update::View(
                        Box::new(load(Some(&id))?),
                        if enabled {
                            t("preset.startup_on")
                        } else {
                            t("preset.startup_off")
                        }
                        .into(),
                    ))
                }
                Request::Stop | Request::List => unreachable!(),
            }
        })();
        if updates
            .send(result.unwrap_or_else(|error| Update::Error(crate::i18n::error_message(&error))))
            .is_err()
        {
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
    /// On/off field: the row's own checkbox is the value (always included).
    Toggle,
    Curve(HWND),
    PerCore(Vec<(u8, u8, HWND, HWND)>),
}
struct Row {
    field: Field,
    check: HWND,
    /// Round "i" button that explains the setting.
    info: HWND,
    /// Measured label width, so the info button sits right after the text.
    label_width: i32,
    editor_width: i32,
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
    groups: Cell<[HWND; GROUPS.len()]>,
    expanded: RefCell<[bool; GROUPS.len()]>,
    pane: Cell<HWND>,
    scroll: Cell<i32>,
    extent: Cell<i32>,
    scroll_drag: Cell<Option<(i32, i32)>>,
    name_frame: Cell<RECT>,
    rendering: Cell<bool>,
    busy: Cell<bool>,
    dirty: Cell<bool>,
    closing: Cell<bool>,
    curve: RefCell<Option<Value>>,
    preview: Option<PathBuf>,
    preview_group: Option<usize>,
    /// Action buttons last placed, so they move only when the set changes.
    actions: Cell<Option<(bool, bool)>>,
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
                t("preset.restore_button")
            } else {
                t("preset.apply_button")
            },
        );
        EnableWindow(
            GetDlgItem(hwnd, APPLY as i32),
            (ready
                && (pending
                    || (saved
                        && !state.dirty.get()
                        && view
                            .as_ref()
                            .is_some_and(|v| v.applicable && v.removed_options == 0))))
                as i32,
        );
        EnableWindow(
            GetDlgItem(hwnd, STARTUP as i32),
            (ready
                && view
                    .as_ref()
                    .is_some_and(|v| v.removed_options == 0 || v.startup_enabled))
                as i32,
        );
        EnableWindow(GetDlgItem(hwnd, RESET as i32), ready as i32);
        // A new preset can only be reset and saved; Apply, Delete and the
        // sign-in option belong to saved presets. A pending restore always shows.
        let visible = actions(saved, pending);
        for id in [APPLY, RESET, SAVE, DELETE] {
            ShowWindow(
                GetDlgItem(hwnd, id as i32),
                if visible.contains(&id) {
                    SW_SHOW
                } else {
                    SW_HIDE
                },
            );
        }
        ShowWindow(
            GetDlgItem(hwnd, STARTUP as i32),
            if saved { SW_SHOW } else { SW_HIDE },
        );
        if state.actions.replace(Some((saved, pending))) != Some((saved, pending)) {
            let mut layout = ui::Layout::new(hwnd);
            place_actions(hwnd, &mut layout, &visible);
            layout.apply();
        }
        for row in state.rows.borrow().iter() {
            EnableWindow(row.check, ready as i32);
            let included = ready && checked(row.check);
            match &row.editor {
                Editor::Text(h) | Editor::Choice(h) => {
                    EnableWindow(*h, included as i32);
                }
                Editor::Toggle => {}
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
/// Buttons in the action row, left to right.
fn actions(saved: bool, pending: bool) -> Vec<usize> {
    match (saved, pending) {
        (true, _) => vec![APPLY, RESET, SAVE, DELETE],
        (false, true) => vec![APPLY, RESET, SAVE],
        (false, false) => vec![RESET, SAVE],
    }
}
/// Right-aligned action buttons; hidden ones leave no gap.
fn place_actions(hwnd: HWND, layout: &mut ui::Layout, visible: &[usize]) {
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        for (i, id) in visible.iter().rev().enumerate() {
            layout.place(
                GetDlgItem(hwnd, *id as i32),
                r.right - 154 - i as i32 * 134,
                r.bottom - 65,
                126,
                40,
            );
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
        status(hwnd, state, t("preset.connection_lost"), true);
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
        for field in preset_fields::fields() {
            let id = FIELD_BASE + field.control_slot() * FIELD_STRIDE;
            let label = if field.unit().is_empty() {
                field.label().to_string()
            } else {
                format!("{} ({})", field.label(), field.unit())
            };
            let check = ui::control(
                pane,
                state.theme.body,
                id,
                "BUTTON",
                &label,
                WS_TABSTOP | BS_AUTOCHECKBOX as u32 | BS_NOTIFY as u32 | BS_MULTILINE as u32,
            );
            let label_width = ui::text_width(pane, state.theme.body, &label);
            let info = ui::control(
                pane,
                state.theme.small,
                id + INFO_OFFSET,
                "BUTTON",
                "i",
                WS_TABSTOP | BS_OWNERDRAW as u32,
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
                ValueKind::Boolean => Editor::Toggle,
                ValueKind::VfCurve => Editor::Curve(ui::control(
                    pane,
                    state.theme.body,
                    id + 1,
                    "BUTTON",
                    t("preset.edit_curve"),
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
                                &tf("preset.core_label", &[&(ccd + 1), &(i + 1)]),
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
            let editor_width = match &editor {
                Editor::Choice(handle) => ui::dropdown_width(
                    *handle,
                    state.theme.body,
                    &field
                        .choices()
                        .iter()
                        .map(|choice| preset_fields::choice_label(*choice))
                        .collect::<Vec<_>>(),
                ),
                _ => 176,
            };
            let note = ui::control(pane, state.theme.small, 0, "STATIC", "", 0);
            theme::muted(note);
            rows.push(Row {
                field,
                check,
                info,
                label_width,
                editor_width,
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
        choices(combo, &[t("preset.new_entry")]);
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
                t("preset.startup_invalid_label")
            } else {
                t("preset.startup_label")
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
        // Without a saved curve: the driver's factory curve. The live curve
        // may carry an applied undervolt and must not become the default.
        .or_else(|| {
            view.curve
                .as_ref()
                .ok()
                .and_then(|curve| curve.factory().ok())
                .and_then(|curve| curve.value().ok())
        });
    let mut added = false;
    for row in state.rows.borrow().iter() {
        let stored = view
            .selected
            .as_ref()
            .and_then(|p| preset_fields::display_value(p, row.field.feature).cloned());
        // On/off fields are always part of a preset. A saved preset without
        // one shows its default (cTGP on, others off); Save stores it.
        added |=
            matches!(row.editor, Editor::Toggle) && view.selected.is_some() && stored.is_none();
        set_checked(row.check, stored.is_some());
        row.edited.set(false);
        *row.baseline.borrow_mut() = stored.clone();
        show_value(
            row,
            &stored.unwrap_or_else(|| default_value(&row.field, &view.defaults)),
        );
        // Explanations live only in the "i" buttons (user rule); an empty
        // note takes no space in the layout.
        text(row.note, "");
    }
    let summary = if view.pending {
        t("preset.pending").into()
    } else if view.removed_options > 0 {
        tf("preset.removed_options", &[&view.removed_options])
    } else if added {
        t("preset.toggles_added").into()
    } else if view.startup_enabled && !view.startup_valid {
        t("preset.startup_invalid").into()
    } else if !view.blockers.is_empty() {
        tf(
            "preset.preflight_blocked",
            &[&view
                .blockers
                .iter()
                .map(|reason| crate::i18n::error_message(reason))
                .collect::<Vec<_>>()
                .join("\r\n")],
        )
    } else if !message.is_empty() {
        message.to_string()
    } else if view.selected.is_none() {
        t("preset.hint_new").into()
    } else {
        t("preset.loaded").into()
    };
    *state.view.borrow_mut() = Some(view);
    // The added on/off defaults are a draft change: Apply follows Save.
    state.dirty.set(added);
    state.busy.set(false);
    state.rendering.set(false);
    status(hwnd, state, &summary, false);
    enable(hwnd, state);
    unsafe {
        layout_pane(state.pane.get(), state);
    }
}
/// Shows `value` in the row's editor without changing whether it is included.
fn show_value(row: &Row, value: &Value) {
    unsafe {
        match (&row.editor, value) {
            (Editor::Text(h), Value::Integer(value)) => text(
                *h,
                &crate::i18n::number(&preset::format_number(*value, row.field.scale())),
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
            (Editor::Toggle, Value::Boolean(value)) => set_checked(row.check, *value),
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
}
/// The device's own value where one was read, else the field's seed.
fn default_value(field: &Field, defaults: &BTreeMap<Feature, Value>) -> Value {
    defaults
        .get(&field.feature)
        .cloned()
        .unwrap_or_else(|| field.initial())
}
/// Draft back to the system defaults: no setting is selected, so the preset
/// changes nothing until options are chosen again, and each editor shows the
/// device's default value. Only the draft changes; Save keeps it.
fn reset(hwnd: HWND, state: &State) {
    state.rendering.set(true);
    let view = state.view.borrow();
    let defaults = view.as_ref().map(|v| &v.defaults);
    for row in state.rows.borrow().iter() {
        // On/off fields take their default from `show_value` below.
        set_checked(row.check, false);
        row.edited.set(true);
        show_value(
            row,
            &defaults.map_or_else(
                || row.field.initial(),
                |defaults| default_value(&row.field, defaults),
            ),
        );
    }
    // The curve editor starts again from the driver's factory curve, not
    // from a curve a preset applied earlier.
    *state.curve.borrow_mut() = view.as_ref().and_then(|v| {
        v.curve
            .as_ref()
            .ok()
            .and_then(|c| c.factory().ok())
            .and_then(|c| c.value().ok())
    });
    drop(view);
    set_checked(unsafe { GetDlgItem(hwnd, STARTUP as i32) }, false);
    state.rendering.set(false);
    state.dirty.set(true);
    status(hwnd, state, t("preset.reset_done"), false);
    enable(hwnd, state);
}
fn collect(state: &State) -> Result<SaveData, String> {
    let view = state.view.borrow();
    let view = view.as_ref().ok_or(t("preset.still_loading"))?;
    let hwnd = unsafe { GetParent(state.pane.get()) };
    let name = ui::value(unsafe { GetDlgItem(hwnd, NAME as i32) })
        .trim()
        .to_owned();
    if name.is_empty() {
        return Err(t("preset.name_required").into());
    }
    let mut edits = vec![];
    for row in state.rows.borrow().iter() {
        // On/off fields are always part of the preset: no tick writes off.
        let value = if matches!(row.editor, Editor::Toggle) {
            Some(Value::Boolean(checked(row.check)))
        } else if !checked(row.check) {
            None
        } else if !row.edited.get() && row.baseline.borrow().is_some() {
            row.baseline.borrow().clone()
        } else {
            Some(unsafe {
                match &row.editor {
                    Editor::Text(h) => Value::Integer(
                        preset::parse_number(&ui::value(*h), row.field.scale())
                            .map_err(|e| format!("{}: {e}", row.field.label()))?,
                    ),
                    Editor::Choice(h) => {
                        let i = SendMessageW(*h, CB_GETCURSEL, 0, 0);
                        Value::Choice(
                            *row.field
                                .choices()
                                .get(i as usize)
                                .ok_or(t("preset.choice_invalid"))?,
                        )
                    }
                    Editor::Toggle => Value::Boolean(checked(row.check)),
                    Editor::Curve(_) => state
                        .curve
                        .borrow()
                        .clone()
                        .ok_or(t("preset.curve_unreadable"))?,
                    Editor::PerCore(items) => Value::PerCore(
                        items
                            .iter()
                            .map(|(ccd, core, _, h)| {
                                Ok(CoreDelta {
                                    ccd: *ccd,
                                    core: *core,
                                    steps: ui::value(*h)
                                        .parse()
                                        .map_err(|_| t("preset.co_steps_invalid"))?,
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
                .filter(|s| preset_fields::retired(s.feature))
                .map(|s| (s.feature, None)),
        );
    }
    let wanted = checked(unsafe { GetDlgItem(hwnd, STARTUP as i32) });
    Ok(SaveData {
        baseline: view.selected.clone(),
        binding: view.binding.clone(),
        name,
        edits,
        startup: (wanted != view.startup_enabled).then_some(wanted),
    })
}
unsafe fn show(handle: HWND, visible: bool) {
    unsafe {
        if (GetWindowLongPtrW(handle, GWL_STYLE) as u32 & WS_VISIBLE != 0) != visible {
            SetWindowPos(
                handle,
                ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE
                    | SWP_NOSIZE
                    | SWP_NOZORDER
                    | SWP_NOACTIVATE
                    | SWP_NOREDRAW
                    | if visible {
                        SWP_SHOWWINDOW
                    } else {
                        SWP_HIDEWINDOW
                    },
            );
        }
    }
}
unsafe fn layout_pane(hwnd: HWND, state: &State) {
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect);
        let (w, h) = (rect.right, rect.bottom);
        if w < 200 || h <= 0 {
            return;
        }
        let mut layout = ui::Layout::new(hwnd);
        let mut y = 0;
        let scroll = state.scroll.get();
        for group in 0..GROUPS.len() {
            if let Some(section) = SECTIONS.iter().position(|(start, _)| *start == group) {
                layout.place(
                    GetDlgItem(hwnd, 720 + section as i32),
                    4,
                    y - scroll,
                    w - 20,
                    32,
                );
                y += 40;
            }
            layout.place(state.groups.get()[group], 0, y - scroll, w - 18, 48);
            y += 56;
            let expanded = state.expanded.borrow()[group];
            for row in state
                .rows
                .borrow()
                .iter()
                .filter(|r| r.field.group == group)
            {
                let has_note = !ui::value(row.note).is_empty();
                let shape =
                    crate::preset_layout::row(w, row.label_width, row.editor_width, has_note);
                show(row.check, expanded);
                show(row.info, expanded);
                show(row.note, expanded && has_note);
                if expanded {
                    layout.place(
                        row.check,
                        16,
                        y - scroll,
                        shape.check_width,
                        shape.label_height,
                    );
                    layout.place(
                        row.info,
                        20 + shape.check_width,
                        y - scroll + 3,
                        INFO_SIZE,
                        INFO_SIZE,
                    );
                    layout.place(
                        row.note,
                        16,
                        y - scroll + shape.note_y,
                        (shape.editor_x - 28).max(100),
                        26,
                    );
                }
                match &row.editor {
                    Editor::Text(input) | Editor::Choice(input) | Editor::Curve(input) => {
                        show(*input, expanded);
                        if expanded {
                            layout.place(
                                *input,
                                shape.editor_x,
                                y - scroll + shape.editor_y,
                                shape.editor_width,
                                if matches!(row.editor, Editor::Choice(_)) {
                                    280
                                } else {
                                    32
                                },
                            );
                        }
                    }
                    Editor::Toggle => {}
                    Editor::PerCore(items) => {
                        for (ccd, core, label, input) in items {
                            show(*label, expanded);
                            show(*input, expanded);
                            if expanded {
                                let column = (w - 56) / 2;
                                let x = 18 + *ccd as i32 * column;
                                let top = y + 42 + *core as i32 * 36 - scroll;
                                layout.place(*label, x, top + 5, column - 104, 25);
                                layout.place(*input, x + column - 100, top, 78, 29);
                            }
                        }
                    }
                }
                if expanded {
                    y += if matches!(row.editor, Editor::PerCore(_)) {
                        342
                    } else {
                        shape.height
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
        layout.apply();
    }
}
unsafe fn layout(hwnd: HWND, state: &State) {
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        let (w, h) = (r.right, r.bottom);
        if w < 200 || h < 200 {
            return;
        }
        let mut layout = ui::Layout::new(hwnd);
        layout.place(GetDlgItem(hwnd, 730), 28, 20, w - 56, 40);
        layout.place(GetDlgItem(hwnd, TAB as i32), 28, 72, 174, 36);
        let column = (w - 56 - 2 * 16) / 3;
        let [select, new, name] = [0, 1, 2].map(|i| 28 + i * (column + 16));
        layout.place(GetDlgItem(hwnd, 731), select, 125, column, 22);
        let combo = GetDlgItem(hwnd, SELECT as i32);
        layout.place(combo, select, 150, column, 320);
        layout.place(GetDlgItem(hwnd, NEW as i32), new, 150, column, FIELD_HEIGHT);
        layout.place(GetDlgItem(hwnd, 732), name, 125, column, 22);
        // Vertically center the original single-line EDIT in its painted field.
        // No custom text drawing: caret, selection, IME and keyboard stay native.
        state.name_frame.set(RECT {
            left: name,
            top: 150,
            right: name + column,
            bottom: 150 + FIELD_HEIGHT,
        });
        layout.place(
            GetDlgItem(hwnd, NAME as i32),
            name + 8,
            157,
            column - 16,
            22,
        );
        layout.place(state.pane.get(), 28, 208, w - 56, h - 414);
        layout.place(GetDlgItem(hwnd, STARTUP as i32), 28, h - 191, w - 56, 30);
        layout.place(GetDlgItem(hwnd, STATUS), 28, h - 149, w - 56, 68);
        let (saved, pending) = state
            .view
            .borrow()
            .as_ref()
            .map_or((false, false), |v| (v.selected.is_some(), v.pending));
        state.actions.set(Some((saved, pending)));
        place_actions(hwnd, &mut layout, &actions(saved, pending));
        layout.apply();
        SendMessageW(
            combo,
            CB_SETITEMHEIGHT,
            usize::MAX,
            (FIELD_HEIGHT - 6) as isize,
        );
        layout_pane(state.pane.get(), state);
    }
}
unsafe fn scroll(hwnd: HWND, state: &State, delta: i32) {
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        let next = state
            .scroll
            .get()
            .saturating_add(delta)
            .clamp(0, (state.extent.get() - r.bottom).max(0));
        if next != state.scroll.replace(next) {
            layout_pane(hwnd, state);
        }
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
            t(GROUPS[group]),
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
                // Precision touchpads send deltas below WHEEL_DELTA (120); scale
                // before dividing so they still scroll.
                scroll(hwnd, state, -((w >> 16) as u16 as i16 as i32) * 108 / 120);
                0
            }
            WM_DRAWITEM => {
                let item = &*(l as *const DRAWITEMSTRUCT);
                let id = item.CtlID as usize;
                if (GROUP_BASE..GROUP_BASE + GROUPS.len()).contains(&id) {
                    group_draw(state, item);
                } else if id >= FIELD_BASE && (id - FIELD_BASE) % FIELD_STRIDE == INFO_OFFSET {
                    state.theme.info(item);
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
/// Shows what a setting changes and what it does.
fn field_index(state: &State, id: usize) -> Option<usize> {
    let slot = id.checked_sub(FIELD_BASE)? / FIELD_STRIDE;
    state
        .rows
        .borrow()
        .iter()
        .position(|row| row.field.control_slot() == slot)
}
fn explain(hwnd: HWND, state: &State, index: usize) {
    if let Some(row) = state.rows.borrow().get(index) {
        unsafe {
            MessageBoxW(
                hwnd,
                wide(row.field.info()).as_ptr(),
                wide(row.field.label()).as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
}
fn dirty(hwnd: HWND, state: &State) {
    state.dirty.set(true);
    status(hwnd, state, t("preset.unsaved"), false);
    enable(hwnd, state);
}
fn edit_gpu_curve(hwnd: HWND, state: &State) {
    let draft = (|| {
        let view = state.view.borrow();
        let view = view.as_ref().ok_or(t("preset.still_loading"))?;
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
            status(hwnd, state, t("preset.curve_taken"), false);
        }
        Ok(None) => status(
            hwnd,
            state,
            if state.dirty.get() {
                t("preset.curve_discarded_dirty")
            } else {
                t("preset.curve_discarded")
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
                && let Editor::Curve(button) = row.editor
            {
                windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(button);
            }
        }
    }
}
fn discard(hwnd: HWND, state: &State) -> bool {
    !state.dirty.get()
        || unsafe {
            MessageBoxW(
                hwnd,
                wide(t("preset.discard_question")).as_ptr(),
                wide(t("preset.discard_title")).as_ptr(),
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
                ui::control(hwnd, state.theme.title, 730, "STATIC", t("preset.title"), 0);
                ui::control(
                    hwnd,
                    state.theme.body,
                    TAB,
                    "BUTTON",
                    t("preset.tab"),
                    WS_TABSTOP,
                );
                ui::control(
                    hwnd,
                    state.theme.small,
                    731,
                    "STATIC",
                    t("preset.preset_label"),
                    0,
                );
                ui::control(
                    hwnd,
                    state.theme.body,
                    SELECT,
                    "COMBOBOX",
                    "",
                    WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                );
                ui::control(
                    hwnd,
                    state.theme.body,
                    NEW,
                    "BUTTON",
                    t("preset.new"),
                    WS_TABSTOP,
                );
                ui::control(
                    hwnd,
                    state.theme.small,
                    732,
                    "STATIC",
                    t("preset.name_label"),
                    0,
                );
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
                    WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
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
                for (section, (_, label)) in SECTIONS.iter().enumerate() {
                    ui::control(
                        pane,
                        state.theme.heading,
                        720 + section,
                        "STATIC",
                        t(label),
                        0,
                    );
                }
                state.groups.set(std::array::from_fn(|i| {
                    ui::control(
                        pane,
                        state.theme.body,
                        GROUP_BASE + i,
                        "BUTTON",
                        t(GROUPS[i]),
                        WS_TABSTOP | BS_OWNERDRAW as u32 | BS_NOTIFY as u32,
                    )
                }));
                build_rows(hwnd, state);
                ui::control(
                    hwnd,
                    state.theme.body,
                    STARTUP,
                    "BUTTON",
                    t("preset.startup_label"),
                    WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                );
                ui::control(
                    hwnd,
                    state.theme.small,
                    STATUS as usize,
                    "EDIT",
                    t("preset.status_loading"),
                    WS_TABSTOP
                        | WS_VSCROLL
                        | ES_MULTILINE as u32
                        | ES_READONLY as u32
                        | ES_AUTOVSCROLL as u32,
                );
                for (id, label) in [
                    (APPLY, t("preset.apply_button")),
                    (RESET, t("preset.reset")),
                    (SAVE, t("preset.save")),
                    (DELETE, t("preset.delete")),
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
                FillRect(w as HDC, &state.name_frame.get(), state.theme.field);
                1
            }
            WM_LBUTTONDOWN => {
                let point = POINT {
                    x: l as u16 as i16 as i32,
                    y: (l >> 16) as u16 as i16 as i32,
                };
                if PtInRect(&state.name_frame.get(), point) != 0 {
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus(GetDlgItem(
                        hwnd,
                        NAME as i32,
                    ));
                }
                0
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
                if id >= FIELD_BASE
                    && (id - FIELD_BASE) % FIELD_STRIDE == INFO_OFFSET
                    && notification == BN_CLICKED as usize
                {
                    if let Some(index) = field_index(state, id) {
                        explain(hwnd, state, index);
                    }
                    return 0;
                }
                if state.rendering.get() || state.busy.get() {
                    return 0;
                }
                if (GROUP_BASE..GROUP_BASE + GROUPS.len()).contains(&id)
                    && notification == BN_CLICKED as usize
                {
                    let group = id - GROUP_BASE;
                    state.expanded.borrow_mut()[group] ^= true;
                    layout_pane(state.pane.get(), state);
                    return 0;
                }
                if id >= FIELD_BASE {
                    let Some(index) = field_index(state, id) else {
                        return 0;
                    };
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
                    if let Some(row) = state.rows.borrow().get(index)
                        && (notification == EN_CHANGE as usize
                            || notification == CBN_SELCHANGE as usize
                            || (offset == 0 && notification == BN_CLICKED as usize))
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
                        // Dynamic Boost proposes cTGP on and the Hub's
                        // Unleashed TPP of 72 W, visible and editable.
                        if offset == 0
                            && checked(row.check)
                            && row.field.feature == Feature::GpuUnleashed
                        {
                            for other_row in state.rows.borrow().iter() {
                                let value = match other_row.field.feature {
                                    Feature::GpuCustomTgp => Value::Boolean(true),
                                    Feature::HpSharedPowerMw => Value::Integer(72_000),
                                    _ => continue,
                                };
                                if !checked(other_row.check) {
                                    set_checked(other_row.check, true);
                                    show_value(other_row, &value);
                                    other_row.edited.set(true);
                                }
                            }
                        }
                        dirty(hwnd, state);
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
                        dispatch(hwnd, state, Request::Load(id), t("preset.loading"));
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
                            dispatch(hwnd, state, Request::Load(None), t("preset.new_loading"));
                        }
                    }
                    RESET => reset(hwnd, state),
                    SAVE => match collect(state) {
                        Ok(data) => dispatch(
                            hwnd,
                            state,
                            Request::Save(Box::new(data)),
                            t("preset.saving"),
                        ),
                        Err(e) => status(hwnd, state, &e, true),
                    },
                    APPLY => {
                        if state.view.borrow().as_ref().is_some_and(|v| v.pending) {
                            dispatch(hwnd, state, Request::Recover, t("preset.restoring"));
                        } else if !state.dirty.get()
                            && let Some(id) = selected(state)
                        {
                            dispatch(hwnd, state, Request::Apply(id), t("common.uac"));
                        }
                    }
                    DELETE => {
                        if let Some(id) = selected(state) {
                            dispatch(hwnd, state, Request::Delete(id), t("preset.deleting"));
                        }
                    }
                    STARTUP if selected(state).is_none() || state.dirty.get() => {
                        // Part of the draft; Save applies and enables it.
                        dirty(hwnd, state);
                        status(hwnd, state, t("preset.startup_on_save"), false);
                    }
                    STARTUP => {
                        if let Some(id) = selected(state) {
                            dispatch(
                                hwnd,
                                state,
                                Request::Startup(id, checked(GetDlgItem(hwnd, STARTUP as i32))),
                                t("preset.startup_updating"),
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
                        Update::List(_) => (),
                        Update::View(view, message) => display(hwnd, state, *view, &message),
                        Update::Error(error) => {
                            state.busy.set(false);
                            // The auto checkbox toggled itself before the request;
                            // a failed or cancelled change must not stay ticked.
                            set_checked(
                                GetDlgItem(hwnd, STARTUP as i32),
                                state
                                    .view
                                    .borrow()
                                    .as_ref()
                                    .is_some_and(|v| v.startup_enabled),
                            );
                            status(hwnd, state, &error, true);
                            enable(hwnd, state);
                        }
                    }
                }
                if let Some(path) = &state.preview {
                    if let Some(group) = state
                        .preview_group
                        .filter(|group| *group > 0 && *group < GROUPS.len())
                    {
                        let mut rect: RECT = std::mem::zeroed();
                        GetWindowRect(state.groups.get()[group], &mut rect);
                        MapWindowPoints(
                            ptr::null_mut(),
                            state.pane.get(),
                            (&mut rect as *mut RECT).cast(),
                            2,
                        );
                        scroll(state.pane.get(), state, rect.top - 48);
                    }
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
                    status(hwnd, state, t("common.finishing"), false);
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
    crate::slint_ui::entry(1, id)
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
        return Err(t("error.preview_size").into());
    }
    if let Some(id) = &id {
        omen_core::profiles::validate_id(id)?;
    }
    let (tx, requests) = mpsc::channel();
    let (updates, rx) = mpsc::channel();
    let mut expanded = [false; GROUPS.len()];
    if let Some(group) = preview_group.filter(|g| *g < GROUPS.len()) {
        expanded[group] = true;
    }
    let state = Box::new(State {
        theme: theme::Theme::new(),
        tx,
        rx,
        view: RefCell::new(None),
        rows: RefCell::new(vec![]),
        groups: Cell::new([ptr::null_mut(); GROUPS.len()]),
        expanded: RefCell::new(expanded),
        pane: Cell::new(ptr::null_mut()),
        scroll: Cell::new(0),
        extent: Cell::new(0),
        scroll_drag: Cell::new(None),
        name_frame: Cell::new(unsafe { std::mem::zeroed() }),
        rendering: Cell::new(true),
        busy: Cell::new(false),
        dirty: Cell::new(false),
        closing: Cell::new(false),
        curve: RefCell::new(None),
        preview,
        preview_group,
        actions: Cell::new(None),
    });
    unsafe {
        ui::register(CLASS, Some(procedure))?;
        ui::register(PANE_CLASS, Some(pane_proc))?;
        let hwnd = ui::create(
            CLASS,
            t("preset.window_title"),
            width,
            height,
            (&*state as *const State).cast_mut().cast(),
            state.preview.is_some(),
        )?;
        state.rendering.set(false);
        ShowWindow(hwnd, SW_SHOW);
        let handle = hwnd as usize;
        let thread = thread::spawn(move || worker(handle, requests, updates));
        dispatch(hwnd, &state, Request::Load(id), t("preset.status_loading"));
        ui::messages(hwnd);
        let _ = state.tx.send(Request::Stop);
        if thread.is_finished() {
            let _ = thread.join();
        }
    }
    Ok(())
}
