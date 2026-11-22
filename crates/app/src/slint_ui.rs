//! Slint presentation over the existing controllers, stores and bounded workers.
//! The native owner receives tray/power/worker messages; it has no visible controls.
use crate::{
    i18n::{t, tf},
    preset_fields, preset_ui, settings, ui,
};
use omen_core::{
    Snapshot,
    app_settings::LanguagePreference,
    battery::Selection,
    curve_editor::{Curve, Kind, Point},
    curve_view::Viewport,
    features::{Feature, ValueKind},
    hp_mode::HpMode,
    keyboard::KeyboardState,
    power::Sampling,
    profiles::{CoreDelta, DeviceBinding, PerformanceProfile, Value},
    quick_switch::{FanMode, GraphicsMode},
};
use omen_platform::profile_store::ProfileStore;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ptr,
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    thread,
};
use windows_sys::Win32::{
    Foundation::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Shell::*, WindowsAndMessaging::*},
};

mod appearance;
mod apply;
mod chrome;
mod colors;
mod cooling;
mod curve;
mod lighting;
mod notices;
mod pacing;
mod surface;
slint::include_modules!();

const AUX_DONE: u32 = WM_APP + 92;
fn navigate_message() -> u32 {
    unsafe { RegisterWindowMessageW(crate::native_ui::wide("OmenHubPP.Navigate").as_ptr()) }
}
const MODES: [(&str, HpMode); 4] = [
    ("mode.quiet", HpMode::Quiet),
    ("mode.balanced", HpMode::Balanced),
    ("mode.performance", HpMode::Performance),
    ("mode.unleashed", HpMode::Unleashed),
];

fn strings(values: impl IntoIterator<Item = String>) -> ModelRc<slint::SharedString> {
    Rc::new(VecModel::from(
        values.into_iter().map(Into::into).collect::<Vec<_>>(),
    ))
    .into()
}
fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

struct FieldDraft {
    value: Value,
    included: bool,
    text: String,
}
/// Choices of a preset's battery behaviour: every other preset, the
/// battery-optimised template until it exists (saving the preset creates it),
/// and the stored choice even when it is an older HP mode or a deleted preset.
fn battery_entries(
    profiles: &[PerformanceProfile],
    current: Option<&str>,
    saved: Option<&Selection>,
) -> Vec<Selection> {
    let mut entries = profiles
        .iter()
        .filter(|p| Some(p.id.as_str()) != current)
        .map(|p| Selection::Preset(p.id.clone()))
        .collect::<Vec<_>>();
    let template = Selection::Preset(crate::battery_preset::ID.into());
    if current != Some(crate::battery_preset::ID) && !entries.contains(&template) {
        entries.push(template);
    }
    if let Some(saved) = saved
        && !entries.contains(saved)
    {
        entries.push(saved.clone());
    }
    entries
}
fn selection_label(selection: &Selection, profiles: &[PerformanceProfile]) -> String {
    match selection {
        Selection::Mode(mode) => MODES
            .iter()
            .find(|(_, m)| m == mode)
            .map_or_else(String::new, |(key, _)| tf("mode.default_entry", &[&t(key)])),
        Selection::Preset(id) => profiles.iter().find(|p| p.id == *id).map_or_else(
            || {
                if id == crate::battery_preset::ID {
                    t("battery.optimized_new").to_owned()
                } else {
                    tf("ui.missing_preset", &[&id])
                }
            },
            |p| p.name.clone(),
        ),
    }
}
enum Aux {
    FanLibrary(String, String),
    SaveFanPreset(
        omen_core::fan_profile::FanPreset,
        Option<omen_core::fan_profile::FanPreset>,
    ),
    DeleteFanPreset(omen_core::fan_profile::FanPreset),
    RequestFanPreset(omen_core::fan_profile::FanPreset),
    Settings,
    SaveSettings(
        settings::Snapshot,
        omen_core::app_settings::AppSettings,
        omen_core::appearance::Appearance,
        bool,
    ),
    Lighting(Option<(KeyboardState, [[u8; 3]; 4], bool)>),
    HpService(bool),
    Stop,
}
enum AuxResult {
    FanLibrary(Result<cooling::Snapshot, String>),
    FanPresetSaved(
        Result<
            (
                omen_core::fan_profile::FanPreset,
                Vec<omen_core::fan_profile::FanPreset>,
            ),
            String,
        >,
    ),
    FanPresetDeleted(Result<(String, String, Vec<omen_core::fan_profile::FanPreset>), String>),
    FanPolicyRequested(Result<(), String>, bool),
    Settings(Result<settings::Snapshot, String>, Option<bool>),
    SettingsSaved(Result<settings::Snapshot, String>),
    Lighting(Result<KeyboardState, String>, bool),
    Hp(Result<bool, String>),
}
fn aux_worker(hwnd: usize, rx: mpsc::Receiver<Aux>, tx: mpsc::Sender<AuxResult>) {
    while let Ok(request) = rx.recv() {
        let result = match request {
            Aux::FanLibrary(board, bios) => AuxResult::FanLibrary((|| {
                let store = ProfileStore::user_data()?;
                Ok(cooling::Snapshot {
                    presets: store.fan_presets()?,
                    active: store.active_fan_curve(),
                    board,
                    bios,
                })
            })()),
            Aux::SaveFanPreset(preset, baseline) => AuxResult::FanPresetSaved((|| {
                let store = ProfileStore::user_data()?;
                store.save_fan_preset(&preset, baseline.as_ref())?;
                Ok((preset, store.fan_presets()?))
            })()),
            Aux::DeleteFanPreset(preset) => AuxResult::FanPresetDeleted((|| {
                let store = ProfileStore::user_data()?;
                store.delete_fan_preset(&preset)?;
                Ok((preset.board, preset.bios, store.fan_presets()?))
            })()),
            Aux::RequestFanPreset(preset) => {
                let result = ProfileStore::user_data()
                    .and_then(|store| crate::profile_fan::request_library(&store, &preset));
                let pending = ProfileStore::user_data()
                    .and_then(|s| Ok(s.pending()? || s.fan_policy_pending()?))
                    .unwrap_or(true);
                AuxResult::FanPolicyRequested(result, pending)
            }
            Aux::Settings => AuxResult::Settings(
                settings::load(),
                omen_platform::hp_control::oem_service()
                    .ok()
                    .flatten()
                    .map(|s| s.disabled),
            ),
            Aux::SaveSettings(base, options, appearance, startup) => {
                AuxResult::SettingsSaved(settings::save(&base, &options, &appearance, startup))
            }
            Aux::Lighting(target) => {
                let applied = target.is_some();
                AuxResult::Lighting(
                    omen_platform::DirectLighting::new().and_then(|mut control| match target {
                        Some((expected, colors, on)) => control.apply(&expected, &colors, on),
                        None => control.read(),
                    }),
                    applied,
                )
            }
            Aux::HpService(disable) => AuxResult::Hp(
                crate::elevation::run(None, crate::elevation::Action::HpService(disable), || false)
                    .and_then(|_| {
                        omen_platform::hp_control::oem_service()?
                            .map(|s| s.disabled)
                            .ok_or_else(|| t("control.unavailable").to_owned())
                    }),
            ),
            Aux::Stop => break,
        };
        if tx.send(result).is_err() {
            break;
        }
        // SAFETY: Opaque notification only; the owner lives through worker shutdown.
        unsafe {
            PostMessageW(hwnd as HWND, AUX_DONE, 0, 0);
        }
    }
}

struct Host {
    fan_library: RefCell<Option<cooling::Library>>,
    library_attempted: Cell<bool>,
    profile_fan_points: RefCell<Option<Vec<omen_core::profiles::FanPoint>>>,
    profile_fan_changed: Cell<bool>,
    profile_fan_invalid: Cell<bool>,
    profile_fan_choices: RefCell<Vec<cooling::Choice>>,
    window: slint::Weak<AppWindow>,
    about: RefCell<Option<AboutWindow>>,
    hwnd: Cell<HWND>,
    icon_hwnd: Cell<HWND>,
    test: bool,
    test_settings: Option<ProfileStore>,
    tx: mpsc::Sender<ui::Request>,
    control: mpsc::Sender<ui::ControlRequest>,
    shared: Arc<Mutex<Option<Snapshot>>>,
    power: Arc<Mutex<Sampling>>,
    manual: Arc<Mutex<Option<ui::ControlOutcome>>>,
    automatic: Arc<Mutex<Option<String>>>,
    preset: mpsc::Sender<preset_ui::Request>,
    preset_results: mpsc::Receiver<preset_ui::Update>,
    aux: mpsc::Sender<Aux>,
    aux_results: mpsc::Receiver<AuxResult>,
    view: RefCell<Option<preset_ui::View>>,
    fields: RefCell<Vec<FieldDraft>>,
    group_rows: Vec<Rc<VecModel<FieldData>>>,
    groups: Rc<VecModel<GroupData>>,
    cores: RefCell<Vec<CoreDelta>>,
    baseline_settings: RefCell<Option<settings::Snapshot>>,
    profiles: RefCell<Vec<PerformanceProfile>>,
    battery_entries: RefCell<Vec<Selection>>,
    lighting: RefCell<Option<KeyboardState>>,
    lighting_draft: RefCell<Option<omen_core::lighting_editor::Draft>>,
    lighting_attempted: Cell<bool>,
    curve: RefCell<Option<curve::Editor>>,
    gpu_draft: RefCell<Option<Curve>>,
    fan_draft: RefCell<Option<Curve>>,
    tray: crate::tray::TrayIcon,
    taskbar: u32,
    visible: Cell<bool>,
    exit_after: Cell<bool>,
    pending_select: Cell<Option<i32>>,
    new_curve_on_load: Cell<bool>,
    settings_loaded: Cell<bool>,
    loading_presets: Cell<bool>,
    appearance_draft: RefCell<omen_core::appearance::Appearance>,
    appearance_tokens: Cell<omen_core::appearance::Tokens>,
    pacing: RefCell<pacing::Pacer>,
    pacing_timer: slint::Timer,
    color_picker: RefCell<Option<colors::Picker>>,
    notice_timer: slint::Timer,
    preset_activity: Cell<Option<notices::PresetActivity>>,
    after_save: RefCell<Option<notices::AfterSave>>,
    apply: apply::Handle,
    operation_busy: Cell<bool>,
    was_minimized: Cell<bool>,
    surface_timer: slint::Timer,
}

impl Host {
    fn show_about(&self) {
        if self.about.borrow().is_none() {
            match AboutWindow::new() {
                Ok(window) => {
                    window.set_window_title(t("ui.about").into());
                    window.set_version_text(
                        format!("{} · Slint 1.18.1", env!("CARGO_PKG_VERSION")).into(),
                    );
                    window.set_close_text(t("settings.close").into());
                    let weak = window.as_weak();
                    window.on_dismiss(move || {
                        if let Some(w) = weak.upgrade() {
                            let _ = w.hide();
                        }
                    });
                    *self.about.borrow_mut() = Some(window);
                }
                Err(e) => {
                    self.status(&error(e), true);
                    return;
                }
            }
        }
        if let Some(window) = self.about.borrow().as_ref() {
            let _ = window.show();
        }
        self.paint_about();
    }
    fn set_lists(&self, profiles: Vec<PerformanceProfile>) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let data = w.global::<Workbench>();
        data.set_profiles(strings(
            std::iter::once(t("preset.new_entry").to_owned())
                .chain(profiles.iter().map(|p| p.name.clone())),
        ));
        // Keep the battery behaviour chosen in the draft across a list refresh.
        let selected = usize::try_from(data.get_battery_index() - 1)
            .ok()
            .and_then(|i| self.battery_entries.borrow().get(i).cloned());
        let current = self
            .view
            .borrow()
            .as_ref()
            .and_then(|v| v.selected.as_ref().map(|p| p.id.clone()));
        self.show_battery(&profiles, current.as_deref(), selected);
        *self.profiles.borrow_mut() = profiles;
    }
    /// Fills the battery-behaviour choices of the preset being edited.
    fn show_battery(
        &self,
        profiles: &[PerformanceProfile],
        current: Option<&str>,
        selected: Option<Selection>,
    ) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let data = w.global::<Workbench>();
        let entries = battery_entries(profiles, current, selected.as_ref());
        data.set_battery_choices(strings(
            std::iter::once(t("battery.off").to_owned())
                .chain(entries.iter().map(|s| selection_label(s, profiles))),
        ));
        data.set_battery_index(
            selected
                .and_then(|s| entries.iter().position(|e| *e == s))
                .map_or(0, |i| i as i32 + 1),
        );
        *self.battery_entries.borrow_mut() = entries;
    }
    /// The battery behaviour chosen in the draft; `None` is off.
    fn battery_choice(&self) -> Option<Selection> {
        let index = self
            .window
            .upgrade()?
            .global::<Workbench>()
            .get_battery_index();
        usize::try_from(index - 1)
            .ok()
            .and_then(|i| self.battery_entries.borrow().get(i).cloned())
    }
    fn visible_hwnd(&self) -> HWND {
        self.window
            .upgrade()
            .and_then(|w| {
                w.window()
                    .with_winit_window(|native| {
                        native
                            .window_handle()
                            .ok()
                            .and_then(|handle| match handle.as_raw() {
                                RawWindowHandle::Win32(h) => Some(h.hwnd.get() as HWND),
                                _ => None,
                            })
                    })
                    .flatten()
            })
            .unwrap_or(self.hwnd.get())
    }
    fn update_icon(&self) {
        let hwnd = self.visible_hwnd();
        if hwnd.is_null() || hwnd == self.hwnd.get() || self.icon_hwnd.replace(hwnd) == hwnd {
            return;
        }
        if let Some(w) = self.window.upgrade() {
            w.window().with_winit_window(|native| {
                native.set_theme(Some(if self.appearance_tokens.get().dark {
                    winit::window::Theme::Dark
                } else {
                    winit::window::Theme::Light
                }))
            });
        }
        unsafe {
            SendMessageW(
                hwnd,
                WM_SETICON,
                ICON_BIG as usize,
                crate::native_ui::app_icon(SM_CXICON) as isize,
            );
            SendMessageW(
                hwnd,
                WM_SETICON,
                ICON_SMALL as usize,
                crate::native_ui::app_icon(SM_CXSMICON) as isize,
            );
        }
    }
    fn status(&self, text: &str, failed: bool) {
        if let Some(window) = self.window.upgrade() {
            let data = window.global::<Workbench>();
            data.set_status(text.into());
        }
        if failed {
            self.notice(t("notice.failed"), text, true);
        }
    }
    fn busy(&self, value: bool) {
        self.operation_busy.set(value);
        self.apply.pause(value);
        if let Some(w) = self.window.upgrade() {
            let d = w.global::<Workbench>();
            d.set_busy(value || d.get_apply_busy());
        }
    }
    fn is_busy(&self) -> bool {
        self.window
            .upgrade()
            .is_some_and(|w| w.global::<Workbench>().get_busy())
    }
    fn blocked(&self) -> bool {
        self.is_busy()
            || self
                .window
                .upgrade()
                .is_some_and(|w| w.global::<Workbench>().get_confirm_kind() != 0)
    }
    fn dirty(&self) {
        if let Some(w) = self.window.upgrade() {
            w.global::<Workbench>().set_dirty(true);
        }
    }
    fn send_preset(&self, request: preset_ui::Request) {
        if self.blocked() {
            return;
        }
        self.preset_activity.set(Some(match &request {
            preset_ui::Request::Load(_) => notices::PresetActivity::Load,
            preset_ui::Request::Save(_) => notices::PresetActivity::Save,
            _ => notices::PresetActivity::Other,
        }));
        self.busy(true);
        self.status(
            t(match &request {
                preset_ui::Request::Load(_) => "preset.loading",
                preset_ui::Request::Save(_) => "preset.saving",
                preset_ui::Request::Recover => "preset.restoring",
                preset_ui::Request::Delete(_) => "preset.deleting",
                preset_ui::Request::Startup(..) => "preset.startup_updating",
                _ => "control.working",
            }),
            false,
        );
        if self.preset.send(request).is_err() {
            let activity = self.preset_activity.take();
            if activity == Some(notices::PresetActivity::Load) {
                self.new_curve_on_load.set(false);
            }
            if activity == Some(notices::PresetActivity::Save) {
                self.abort_saved_selection();
            }
            self.busy(false);
            self.status(t("control.unavailable"), true);
        }
    }
    fn send_aux(&self, request: Aux) {
        if self.blocked() {
            return;
        }
        if self.test {
            self.status(t("ui.test_no_write"), true);
            return;
        }
        self.busy(true);
        self.status(
            t(match &request {
                Aux::Settings | Aux::FanLibrary(..) | Aux::Lighting(None) => "ui.loading",
                Aux::SaveSettings(..) => "ui.settings_saving",
                Aux::Lighting(Some(_)) => "lighting.applying",
                Aux::HpService(true) => "hp_service.disabling",
                Aux::HpService(false) => "hp_service.restoring",
                Aux::SaveFanPreset(..) => "preset.saving",
                Aux::DeleteFanPreset(..) => "preset.deleting",
                Aux::RequestFanPreset(..) => "control.working",
                Aux::Stop => "control.working",
            }),
            false,
        );
        if self.aux.send(request).is_err() {
            self.busy(false);
            self.status(t("control.unavailable"), true);
        }
    }
    fn send_control(&self, request: ui::ControlRequest) {
        if self.blocked() {
            return;
        }
        if self.test {
            self.status(t("ui.test_no_write"), true);
            return;
        }
        self.busy(true);
        self.status(t("control.working"), false);
        if self.control.send(request).is_err() {
            self.busy(false);
            self.status(t("control.unavailable"), true);
        }
    }
    fn update_fields(&self) {
        for index in 0..self.fields.borrow().len() {
            self.update_field(index);
        }
    }
    fn update_field(&self, index: usize) {
        let fields = self.fields.borrow();
        let catalog = preset_fields::fields();
        let (Some(field), Some(draft)) = (catalog.get(index), fields.get(index)) else {
            return;
        };
        let model = &self.group_rows[field.group];
        let local = preset_fields::fields()[..index]
            .iter()
            .filter(|f| f.group == field.group)
            .count();
        // Hub-style watt sliders; the CPU slider follows the Dynamic Boost row.
        let dynamic_boost = catalog
            .iter()
            .zip(fields.iter())
            .any(|(f, d)| f.feature == Feature::GpuUnleashed && d.included);
        let slider = preset_fields::slider_watts(field.feature, dynamic_boost);
        let kind = match field.feature.descriptor().value_kind {
            _ if slider.is_some() => 5,
            ValueKind::Choice => 1,
            ValueKind::Boolean => 2,
            ValueKind::VfCurve => 3,
            ValueKind::PerCore => 4,
            _ => 0,
        };
        let data = FieldData {
            index: index as i32,
            label: field.label().into(),
            unit: field.unit().into(),
            included: draft.included,
            value: draft.text.as_str().into(),
            kind,
            choices: strings(
                field
                    .choices()
                    .iter()
                    .map(|c| preset_fields::choice_label(*c).to_owned()),
            ),
            selected: field
                .choices()
                .iter()
                .position(|c| draft.value == Value::Choice(*c))
                .unwrap_or(0) as i32,
            info: field.info().into(),
            slider_min: slider.map_or(0.0, |(lo, _)| lo as f32),
            slider_max: slider.map_or(0.0, |(_, hi)| hi as f32),
        };
        if local < model.row_count() {
            model.set_row_data(local, data);
        } else {
            model.push(data);
        }
    }
    fn display(&self, view: preset_ui::View, message: &str) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let data = w.global::<Workbench>();
        let mut fields = vec![];
        for field in preset_fields::fields() {
            let saved = view
                .selected
                .as_ref()
                .and_then(|p| preset_fields::display_value(p, field.feature));
            let value = saved
                .cloned()
                .or_else(|| view.defaults.get(&field.feature).cloned())
                .unwrap_or_else(|| field.initial());
            let text = match &value {
                Value::Integer(v) => {
                    crate::i18n::number(&omen_core::preset::format_number(*v, field.scale()))
                }
                _ => String::new(),
            };
            let included = match value {
                Value::Boolean(value) => value,
                _ => saved.is_some(),
            };
            fields.push(FieldDraft {
                value,
                included,
                text,
            });
        }
        let cores = fields
            .iter()
            .find_map(|f| {
                if let Value::PerCore(values) = &f.value {
                    Some(values.clone())
                } else {
                    None
                }
            })
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                (0..16)
                    .map(|i| CoreDelta {
                        ccd: i / 8,
                        core: i % 8,
                        steps: 0,
                    })
                    .collect()
            });
        data.set_cores(
            Rc::new(VecModel::from(
                cores
                    .iter()
                    .map(|c| CoreRow {
                        label: format!("CCD{} · {}", c.ccd, c.core).into(),
                        value: c.steps.to_string().into(),
                    })
                    .collect::<Vec<_>>(),
            ))
            .into(),
        );
        *self.cores.borrow_mut() = cores;
        data.set_profiles(strings(
            std::iter::once(t("preset.new_entry").to_owned())
                .chain(view.profiles.iter().map(|p| p.name.clone())),
        ));
        data.set_profile_index(
            view.selected
                .as_ref()
                .and_then(|s| view.profiles.iter().position(|p| p.id == s.id))
                .map_or(0, |i| i as i32 + 1),
        );
        data.set_preset_name(
            view.selected
                .as_ref()
                .map_or("", |p| p.name.as_str())
                .into(),
        );
        data.set_preset_ready(true);
        data.set_preset_saved(view.selected.is_some());
        data.set_recovery_pending(view.pending);
        data.set_preset_applicable(view.applicable && view.removed_options == 0);
        data.set_preset_startup(view.startup_enabled);
        let missing_toggles = view.selected.as_ref().is_some_and(|p| {
            preset_fields::fields().iter().any(|f| {
                f.feature.descriptor().value_kind == ValueKind::Boolean
                    && preset_fields::display_value(p, f.feature).is_none()
            })
        });
        data.set_dirty(missing_toggles);
        *self.fields.borrow_mut() = fields;
        *self.gpu_draft.borrow_mut() = None;
        self.set_lists(view.profiles.clone());
        self.show_battery(
            &view.profiles,
            view.selected.as_ref().map(|p| p.id.as_str()),
            view.battery.clone(),
        );
        let blockers = view
            .blockers
            .iter()
            .map(|b| crate::i18n::message(b))
            .collect::<Vec<_>>()
            .join("\n");
        if message.is_empty() {
            self.status(
                if blockers.is_empty() {
                    t("ui.ready")
                } else {
                    &blockers
                },
                !blockers.is_empty(),
            );
        } else {
            self.status(message, false);
        }
        if view.startup_enabled && !view.startup_valid {
            self.status(t("preset.startup_blocked"), true);
        }
        if view.removed_options > 0 {
            self.status(
                &tf("preset.removed_options", &[&view.removed_options]),
                false,
            );
        }
        if missing_toggles {
            self.status(t("preset.toggles_added"), false);
        }
        if !self.test {
            data.set_last_apply(ui::last_apply_status().unwrap_or_default().into());
        }
        if view.pending {
            self.status(t("preset.pending"), true);
        }
        *self.view.borrow_mut() = Some(view);
        self.reset_profile_fan();
        self.update_fields();
    }
    fn select(&self, index: i32) {
        if self.is_busy() {
            return;
        }
        let Some(w) = self.window.upgrade() else {
            return;
        };
        if w.global::<Workbench>().get_dirty() || self.gpu_curve_dirty() {
            self.pending_select.set(Some(index));
            w.global::<Workbench>().set_confirm_kind(1);
            return;
        }
        self.load_preset(index);
    }
    fn cancel_preset_edit(&self) {
        if self.blocked() {
            return;
        }
        let baseline = self.view.borrow().clone();
        let Some(baseline) = baseline else {
            return;
        };
        self.new_curve_on_load.set(false);
        self.pending_select.set(None);
        self.after_save.borrow_mut().take();
        if self
            .curve
            .borrow()
            .as_ref()
            .is_some_and(|e| e.curve.kind == Kind::Gpu)
        {
            self.curve.borrow_mut().take();
        }
        let pending = baseline.pending;
        self.display(baseline, t("preset.edits_discarded"));
        if let Some(w) = self.window.upgrade() {
            let d = w.global::<Workbench>();
            d.set_dirty(false);
            d.set_curve_open(false);
        }
        if !pending {
            self.completed(t("preset.edits_discarded"));
        }
    }
    fn load_preset(&self, index: i32) {
        let id = usize::try_from(index - 1)
            .ok()
            .and_then(|i| self.profiles.borrow().get(i).map(|p| p.id.clone()));
        self.new_curve_on_load.set(index == 0);
        if self.test {
            let view = {
                let profiles = self.profiles.borrow();
                test_view(id.as_deref(), &profiles)
            };
            self.loaded_preset(view, t("ui.test_mode"));
        } else {
            self.send_preset(preset_ui::Request::Load(id));
        }
    }
    fn loaded_preset(&self, view: preset_ui::View, message: &str) {
        // Only an explicit New selection opens the editor. Startup and ordinary
        // profile refreshes keep the telemetry-first dashboard.
        let open_factory = self.new_curve_on_load.replace(false) && view.selected.is_none();
        self.display(view, message);
        if open_factory {
            self.open_new_gpu_curve();
        }
    }
    fn save_data(&self) -> Result<preset_ui::SaveData, String> {
        if self.gpu_curve_dirty() {
            return Err(t("ui.use_curve_first").into());
        }
        let w = self.window.upgrade().ok_or(t("control.unavailable"))?;
        let data = w.global::<Workbench>();
        let borrowed = self.view.borrow();
        let view = borrowed.as_ref().ok_or(t("preset.still_loading"))?;
        let name = data.get_preset_name().trim().to_owned();
        if name.is_empty() {
            return Err(t("preset.name_required").into());
        }
        let mut edits = vec![];
        for (field, draft) in preset_fields::fields()
            .iter()
            .zip(self.fields.borrow().iter())
        {
            let value = if field.feature.descriptor().value_kind == ValueKind::Boolean {
                Some(Value::Boolean(draft.included))
            } else if !draft.included {
                None
            } else {
                Some(match field.feature.descriptor().value_kind {
                    ValueKind::Integer => {
                        Value::Integer(omen_core::preset::parse_number(&draft.text, field.scale())?)
                    }
                    ValueKind::PerCore => Value::PerCore(
                        self.cores
                            .borrow()
                            .iter()
                            .enumerate()
                            .map(|(i, core)| {
                                let mut core = core.clone();
                                core.steps = data
                                    .get_cores()
                                    .row_data(i)
                                    .ok_or(t("preset.co_steps_invalid"))?
                                    .value
                                    .parse()
                                    .map_err(|_| t("preset.co_steps_invalid"))?;
                                Ok(core)
                            })
                            .collect::<Result<Vec<_>, String>>()?,
                    ),
                    _ => draft.value.clone(),
                })
            };
            if let Some(value) = &value {
                omen_core::profiles::Setting {
                    feature: field.feature,
                    value: value.clone(),
                }
                .validate()?;
            }
            edits.push((field.feature, value));
        }
        if let Some(base) = &view.selected {
            edits.extend(
                base.settings
                    .iter()
                    .filter(|s| preset_fields::retired(s.feature))
                    .map(|s| (s.feature, None)),
            );
        }
        Ok(preset_ui::SaveData {
            // Fan linkage is an explicit point snapshot, not a mutable library id.
            baseline: view.selected.clone(),
            binding: view.binding.clone(),
            name,
            edits: {
                self.append_profile_fan(&mut edits, data.get_preset_startup())?;
                edits
            },
            startup: (data.get_preset_startup() != view.startup_enabled)
                .then_some(data.get_preset_startup()),
            battery: {
                let chosen = self.battery_choice();
                (chosen != view.battery).then_some(chosen)
            },
        })
    }
    fn save_preset(&self) {
        match self.save_data() {
            Err(e) => {
                self.abort_saved_selection();
                self.status(&crate::i18n::error_message(&e), true);
            }
            Ok(data) if self.test => {
                let profile = if let Some(base) = &data.baseline {
                    match omen_core::preset::merge(base, base, &data.name, &data.edits) {
                        Ok(p) => p,
                        Err(e) => {
                            self.abort_saved_selection();
                            self.status(&crate::i18n::error_message(&e), true);
                            return;
                        }
                    }
                } else {
                    PerformanceProfile {
                        schema: 1,
                        id: data.baseline.as_ref().map_or_else(
                            || format!("ui-test-{}", crate::native_ui::token()),
                            |p| p.id.clone(),
                        ),
                        name: data.name,
                        binding: data.binding,
                        settings: data
                            .edits
                            .into_iter()
                            .filter_map(|(feature, value)| {
                                value.map(|value| omen_core::profiles::Setting { feature, value })
                            })
                            .collect(),
                    }
                };
                let mut profiles = self.profiles.borrow().clone();
                profiles.retain(|p| p.id != profile.id);
                profiles.push(profile.clone());
                self.display(test_view(Some(&profile.id), &profiles), t("ui.test_saved"));
                self.saved(t("ui.test_saved"));
                self.follow_saved_selection();
            }
            Ok(data) => self.send_preset(preset_ui::Request::Save(Box::new(data))),
        }
    }
    fn navigate(&self, page: i32) {
        if self
            .window
            .upgrade()
            .is_some_and(|w| w.global::<Workbench>().get_confirm_kind() != 0)
        {
            return;
        }
        if let Some(w) = self.window.upgrade() {
            if w.global::<Workbench>().get_curve_open() {
                self.curve_action("close");
            }
            w.global::<Workbench>().set_page(page);
            if self.test {
                return;
            }
            if !self.is_busy() && page < 2 && !w.global::<Workbench>().get_preset_ready() {
                self.loading_presets.set(false);
            }
            if !self.is_busy() && page == 4 && !w.global::<Workbench>().get_settings_ready() {
                self.settings_loaded.set(false);
            }
            if page < 2 && !self.loading_presets.get() && !self.test && !self.is_busy() {
                self.loading_presets.set(true);
                self.send_preset(preset_ui::Request::Load(None));
            }
            if page == 3
                && self.lighting.borrow().is_none()
                && !self.lighting_attempted.get()
                && !self.is_busy()
            {
                self.lighting_attempted.set(true);
                self.send_aux(Aux::Lighting(None));
            }
            if page == 4 && !self.settings_loaded.get() && !self.is_busy() {
                self.settings_loaded.set(true);
                self.send_aux(Aux::Settings);
            }
        }
    }
    fn sync_quick(&self) {
        if self.test {
            return;
        }
        if let (Some(w), Ok(store)) = (self.window.upgrade(), ProfileStore::user_data()) {
            let d = w.global::<Workbench>();
            d.set_fan_index(store.fan_mode().index() as i32);
            if let Some(graphics) = store.graphics_mode() {
                d.set_graphics_index(graphics.mode.index() as i32);
                d.set_graphics_status(
                    t(if graphics.restart_pending(omen_platform::boot_unix_s()) {
                        "ui.restart_pending"
                    } else {
                        "ui.saved_preference"
                    })
                    .into(),
                );
            }
            d.set_cooling_status(t("ui.fan_status_note").into());
        }
    }
    fn show(&self) {
        if let Some(w) = self.window.upgrade() {
            let _ = w.show();
            w.window().with_winit_window(|native| {
                native.set_minimized(false);
                native.focus_window();
            });
        }
        self.update_icon();
        self.visible.set(true);
        self.arm_notice();
        self.refresh_surface();
        let _ = self.tx.send(ui::Request::Visibility(true));
    }
    fn hide(&self) {
        if unsafe { self.tray.ensure(self.hwnd.get()) } {
            self.icon_hwnd.set(ptr::null_mut());
            if let Some(w) = self.window.upgrade() {
                let _ = w.hide();
            }
            self.visible.set(false);
            self.notice_timer.stop();
            self.pacing_timer.stop();
            self.pacing.borrow_mut().reset();
            let _ = self.tx.send(ui::Request::Visibility(false));
        }
    }
    fn exit(&self) {
        if self.is_busy() {
            self.exit_after.set(true);
            self.status(t("common.finishing"), false);
            return;
        }
        self.close_color_picker(false);
        if let Some(w) = self.window.upgrade()
            && (w.global::<Workbench>().get_dirty()
                || self.curves_dirty()
                || self.settings_dirty()
                || self.lighting_dirty())
        {
            w.global::<Workbench>().set_confirm_kind(3);
            self.show();
            return;
        }
        let _ = slint::quit_event_loop();
    }
    fn drain(&self, message: u32) {
        if message == apply::DONE {
            while let Ok(outcome) = self.apply.results.try_recv() {
                if !self.apply.current(outcome.generation) {
                    continue;
                }
                if let Some(w) = self.window.upgrade() {
                    let d = w.global::<Workbench>();
                    d.set_apply_busy(false);
                    d.set_busy(self.operation_busy.get());
                    if let Some(pending) = outcome.pending {
                        d.set_recovery_pending(pending);
                        if let Some(view) = self.view.borrow_mut().as_mut() {
                            view.pending = pending;
                        }
                    }
                }
                match outcome.result {
                    Ok(message) => {
                        self.status(&message, false);
                        self.completed(&message);
                    }
                    Err(error) => self.status(&crate::i18n::error_message(&error), true),
                }
                if let Some(w) = self.window.upgrade() {
                    w.global::<Workbench>()
                        .set_last_apply(ui::last_apply_status().unwrap_or_default().into());
                }
            }
        }

        if message == ui::UPDATE {
            if let Some(snapshot) = self
                .shared
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
            {
                self.telemetry(&snapshot);
            }
            if !self.is_busy()
                && let Some(n) =
                    crate::preset_hold::take_notice().or_else(crate::fan_watch::take_notice)
            {
                self.status(&n, true);
            }
        }
        if message == ui::MODE_RESULT {
            self.busy(false);
            if let Some(text) = self.manual.lock().unwrap_or_else(|e| e.into_inner()).take() {
                self.status(&crate::i18n::message(&text.message), text.failed);
                if !text.failed {
                    self.completed(&crate::i18n::message(&text.message));
                }
            }
            self.sync_quick();
            if let Some(w) = self.window.upgrade() {
                w.global::<Workbench>()
                    .set_last_apply(ui::last_apply_status().unwrap_or_default().into());
            }
        }
        if message == ui::AUTO_RESULT
            && let Some(text) = self
                .automatic
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            && !self.is_busy()
        {
            self.status(&text, false);
            self.notice(t("notice.background"), &text, false);
        }
        if message == preset_ui::UPDATE {
            while let Ok(update) = self.preset_results.try_recv() {
                match update {
                    preset_ui::Update::List(result) => match result {
                        Ok((profiles, applied, pending)) => {
                            let initial = applied.as_ref().and_then(|s| match s {
                                Selection::Preset(id) if profiles.iter().any(|p| p.id == *id) => {
                                    Some(id.clone())
                                }
                                _ => None,
                            });
                            self.set_lists(profiles);
                            if let Some(w) = self.window.upgrade() {
                                w.global::<Workbench>().set_recovery_pending(pending);
                            }
                            if pending {
                                self.status(t("preset.pending"), true);
                            }
                            self.sync_quick();
                            if self.view.borrow().is_none()
                                && !self.loading_presets.get()
                                && !self.is_busy()
                                && self
                                    .window
                                    .upgrade()
                                    .is_some_and(|w| w.global::<Workbench>().get_page() < 2)
                            {
                                self.loading_presets.set(true);
                                self.send_preset(preset_ui::Request::Load(initial));
                            }
                        }
                        Err(e) => self.status(&crate::i18n::error_message(&e), true),
                    },
                    preset_ui::Update::View(view, status) => {
                        let activity = self.preset_activity.take();
                        self.busy(false);
                        if activity == Some(notices::PresetActivity::Load) {
                            self.loaded_preset(*view, &status);
                        } else {
                            self.display(*view, &status);
                        }
                        if activity == Some(notices::PresetActivity::Save) {
                            self.saved(&status);
                            self.follow_saved_selection();
                        } else if activity == Some(notices::PresetActivity::Other) {
                            self.completed(&status);
                        }
                    }
                    preset_ui::Update::Error(e) => {
                        let activity = self.preset_activity.take();
                        if activity == Some(notices::PresetActivity::Load) {
                            self.new_curve_on_load.set(false);
                        }
                        if activity == Some(notices::PresetActivity::Save) {
                            self.abort_saved_selection();
                        }
                        self.busy(false);
                        self.status(&e, true);
                    }
                }
            }
        }
        if message == AUX_DONE {
            while let Ok(result) = self.aux_results.try_recv() {
                self.busy(false);
                match result {
                    AuxResult::FanLibrary(result) => match result {
                        Ok(snapshot) => self.show_fan_library(snapshot, None),
                        Err(e) => self.status(&crate::i18n::error_message(&e), true),
                    },
                    AuxResult::FanPresetSaved(result) => match result {
                        Ok((preset, presets)) => {
                            let id = preset.id.clone();
                            self.show_fan_library(
                                cooling::Snapshot {
                                    presets,
                                    active: Ok(None),
                                    board: preset.board,
                                    bios: preset.bios,
                                },
                                Some(id),
                            );
                            self.saved(t("fan_profile.saved"));
                        }
                        Err(e) => self.status(&crate::i18n::error_message(&e), true),
                    },
                    AuxResult::FanPresetDeleted(result) => match result {
                        Ok((board, bios, presets)) => {
                            self.show_fan_library(
                                cooling::Snapshot {
                                    presets,
                                    active: Ok(None),
                                    board,
                                    bios,
                                },
                                None,
                            );
                            self.completed(t("fan_profile.deleted"));
                        }
                        Err(e) => self.status(&crate::i18n::error_message(&e), true),
                    },
                    AuxResult::FanPolicyRequested(result, pending) => {
                        if let Some(w) = self.window.upgrade() {
                            w.global::<Workbench>().set_recovery_pending(pending);
                        }
                        if let Some(v) = self.view.borrow_mut().as_mut() {
                            v.pending = pending;
                        }
                        match result {
                            Ok(()) => self.completed(t("fan_profile.requested")),
                            Err(e) => self.status(&crate::i18n::error_message(&e), true),
                        }
                    }
                    AuxResult::Settings(result, hp) => {
                        if let Some(w) = self.window.upgrade() {
                            let data = w.global::<Workbench>();
                            data.set_hp_available(hp.is_some());
                            data.set_hp_disabled(hp.unwrap_or(false));
                        }
                        match result {
                            Ok(s) => self.show_settings(s, false),
                            Err(e) => self.status(&e, true),
                        }
                    }
                    AuxResult::Hp(result) => match result {
                        Ok(disabled) => {
                            if let Some(w) = self.window.upgrade() {
                                w.global::<Workbench>().set_hp_disabled(disabled);
                            }
                            self.status(
                                t(if disabled {
                                    "hp_service.disabled"
                                } else {
                                    "hp_service.restored"
                                }),
                                false,
                            );
                        }
                        Err(e) => self.status(&crate::i18n::error_message(&e), true),
                    },
                    AuxResult::SettingsSaved(result) => match result {
                        Ok(s) => self.show_settings(s, true),
                        Err(e) => self.status(&e, true),
                    },
                    AuxResult::Lighting(result, applied) => match result {
                        Ok(state) => {
                            self.show_lighting(state);
                            if applied {
                                self.completed(t("notice.lighting_applied"));
                            }
                        }
                        Err(e) => self.status(&crate::i18n::error_message(&e), true),
                    },
                }
            }
        }
        if self.exit_after.get() && !self.is_busy() {
            self.exit_after.set(false);
            self.exit();
        }
        if !self.is_busy() {
            self.follow_saved_selection();
        }
        if !self.is_busy()
            && !self.library_attempted.get()
            && self
                .window
                .upgrade()
                .is_some_and(|w| w.global::<Workbench>().get_page() < 3)
        {
            self.load_fan_library();
        }
        if !self.is_busy()
            && !self.settings_loaded.get()
            && self
                .window
                .upgrade()
                .is_some_and(|w| w.global::<Workbench>().get_page() == 4)
        {
            self.navigate(4);
        }
        if !self.is_busy()
            && !self.loading_presets.get()
            && self
                .window
                .upgrade()
                .is_some_and(|w| w.global::<Workbench>().get_page() < 2)
        {
            self.navigate(0);
        }
        if !self.is_busy()
            && !self.lighting_attempted.get()
            && self
                .window
                .upgrade()
                .is_some_and(|w| w.global::<Workbench>().get_page() == 3)
        {
            self.navigate(3);
        }
    }
    fn telemetry(&self, s: &Snapshot) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        let number = |value: Option<f64>, unit: &str| {
            value.map_or_else(
                || t("common.unavailable").into(),
                |v| format!("{} {unit}", crate::i18n::number(&format!("{v:.1}"))),
            )
        };
        d.set_identity(
            format!(
                "{} · {} · BIOS {}",
                s.inventory.model, s.inventory.board, s.inventory.bios
            )
            .into(),
        );
        d.set_gpu_name(
            s.gpu
                .as_ref()
                .map_or_else(|| s.inventory.gpus.join(" · "), |g| g.name.clone())
                .into(),
        );
        d.set_gpu_power(number(s.gpu.as_ref().and_then(|g| g.power_w), "W").into());
        d.set_gpu_temp(
            number(
                s.gpu.as_ref().and_then(|g| g.temperature_c.map(f64::from)),
                "°C",
            )
            .into(),
        );
        d.set_gpu_clock(
            number(
                s.gpu.as_ref().and_then(|g| g.clock_mhz.map(f64::from)),
                "MHz",
            )
            .into(),
        );
        d.set_cpu_name(s.inventory.cpus.join(" · ").into());
        d.set_cpu_power(number(s.cpu.as_ref().and_then(|c| c.package_power_w), "W").into());
        d.set_cpu_temp(number(s.cpu.as_ref().and_then(|c| c.temperature_c), "°C").into());
        d.set_cpu_load(crate::dashboard::cpu_load_line(s).into());
        d.set_cpu_source(crate::dashboard::cpu_detail(s).into());
        d.set_sensor_details(
            s.warnings
                .iter()
                .chain(s.inventory.warnings.iter())
                .map(|w| crate::i18n::message(w))
                .collect::<Vec<_>>()
                .join("\n")
                .into(),
        );
        d.set_power_source(
            format!(
                "{}{}",
                t(if s.power.ac_connected == Some(true) {
                    "ui.ac"
                } else if s.power.ac_connected == Some(false) {
                    "ui.battery"
                } else {
                    "common.unavailable"
                }),
                s.power
                    .battery_percent
                    .map_or_else(String::new, |p| format!(" · {p} %"))
            )
            .into(),
        );
    }
    fn show_settings(&self, s: settings::Snapshot, saved: bool) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        self.show_appearance(s.appearance.clone());
        d.set_language_index(match s.options.language {
            LanguagePreference::System => 0,
            LanguagePreference::German => 1,
            LanguagePreference::English => 2,
        });
        d.set_autostart(s.run_entry.is_some());
        d.set_start_in_tray(s.options.start_in_tray);
        d.set_close_index(i32::from(!s.options.close_to_tray));
        d.set_hold_preset(s.options.hold_preset);
        d.set_settings_ready(true);
        d.set_settings_dirty(false);
        if saved {
            self.status(
                t(
                    if crate::i18n::preference_code(s.options.language)
                        != crate::i18n::language_code()
                    {
                        "settings.saved_restart"
                    } else {
                        "settings.saved"
                    },
                ),
                false,
            );
        }
        if saved {
            self.saved(w.global::<Workbench>().get_status().as_str());
        }
        *self.baseline_settings.borrow_mut() = Some(s);
    }
    fn settings_target(
        &self,
    ) -> Option<(
        settings::Snapshot,
        omen_core::app_settings::AppSettings,
        omen_core::appearance::Appearance,
        bool,
    )> {
        let w = self.window.upgrade()?;
        let d = w.global::<Workbench>();
        let base = self.baseline_settings.borrow().clone()?;
        let mut options = base.options.clone();
        options.language = match d.get_language_index() {
            1 => LanguagePreference::German,
            2 => LanguagePreference::English,
            _ => LanguagePreference::System,
        };
        options.start_in_tray = d.get_start_in_tray();
        options.close_to_tray = d.get_close_index() == 0;
        options.hold_preset = d.get_hold_preset();
        Some((
            base,
            options,
            self.appearance_draft.borrow().clone(),
            d.get_autostart(),
        ))
    }
    fn save_settings(&self) {
        if let Some((base, options, appearance, startup)) = self.settings_target() {
            if let Err(e) = options.validate().and_then(|_| appearance.validate()) {
                self.status(&crate::i18n::error_message(&e), true);
                return;
            }
            if self.test {
                if let Some(store) = &self.test_settings
                    && let Err(e) = settings::persist_options(store, &options, &appearance)
                {
                    self.status(&crate::i18n::error_message(&e), true);
                    return;
                }
                self.show_settings(
                    settings::Snapshot {
                        options,
                        appearance,
                        run_entry: startup.then(|| "UI TEST".to_owned()),
                    },
                    false,
                );
                self.status(
                    t(if self.test_settings.is_some() {
                        "ui.test_settings_saved"
                    } else {
                        "ui.test_saved"
                    }),
                    false,
                );
                if let Some(w) = self.window.upgrade() {
                    self.saved(w.global::<Workbench>().get_status().as_str());
                }
                return;
            }
            self.send_aux(Aux::SaveSettings(base, options, appearance, startup));
        }
    }
    fn settings_dirty(&self) -> bool {
        self.settings_target()
            .is_some_and(|(base, options, appearance, startup)| {
                base.options != options
                    || base.appearance != appearance
                    || base.run_entry.is_some() != startup
            })
    }
}

unsafe extern "system" fn event_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: The Rc<Host> is retained until after DestroyWindow; all dispatch is
    // on the Slint/Win32 event thread. Worker messages contain no Rust pointers.
    unsafe {
        if message == WM_NCCREATE {
            let c = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, c.lpCreateParams as isize);
        }
        let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Host;
        if raw.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let host = &*raw;
        if message == crate::single_instance::activation_message() && message != 0 {
            host.show();
            return 0;
        }
        if message == navigate_message() && message != 0 && wparam <= 4 {
            host.show();
            host.navigate(wparam as i32);
            return 0;
        }
        if message == host.taskbar && message != 0 {
            if !host.tray.ensure(hwnd) && !host.tray.retrying() {
                host.show();
            }
            return 0;
        }
        match message {
            ui::UPDATE
            | ui::MODE_RESULT
            | ui::AUTO_RESULT
            | preset_ui::UPDATE
            | AUX_DONE
            | apply::DONE => {
                host.drain(message);
                0
            }
            ui::CPU_READER_PID => host
                .shared
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .and_then(|s| s.cpu_reader_pid)
                .unwrap_or(0) as isize,
            ui::POWER_STATE => {
                let p = *host.power.lock().unwrap_or_else(|e| e.into_inner());
                ((p.generation << 2) | u64::from(p.suspended) | (u64::from(!p.display_on) << 1))
                    as isize
            }
            WM_POWERBROADCAST => {
                let event = crate::power_events::decode(wparam as u32, lparam);
                // Firmware and other tools often reset limits around these
                // events; one debounced check follows the last of them.
                if event.is_some_and(omen_core::power::Event::may_reset_controls) {
                    crate::preset_hold::nudge();
                }
                if let Some(e) = event
                    && host
                        .power
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .observe(e)
                {
                    let _ = host.tx.send(ui::Request::Power);
                }
                1
            }
            WM_TIMER if wparam == crate::tray::RETRY_TIMER => {
                if !host.tray.retry(hwnd) {
                    host.show();
                }
                0
            }
            crate::tray::CALLBACK => {
                match lparam as u32 & 0xffff {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_SELECT | crate::tray::KEY_SELECT => {
                        host.show()
                    }
                    WM_RBUTTONUP | WM_CONTEXTMENU => {
                        let menu = CreatePopupMenu();
                        AppendMenuW(
                            menu,
                            MF_STRING,
                            1,
                            crate::native_ui::wide(t("tray.open")).as_ptr(),
                        );
                        AppendMenuW(
                            menu,
                            MF_STRING,
                            2,
                            crate::native_ui::wide(t("settings.title")).as_ptr(),
                        );
                        AppendMenuW(
                            menu,
                            MF_STRING,
                            3,
                            crate::native_ui::wide(t("tray.exit")).as_ptr(),
                        );
                        let mut point = POINT { x: 0, y: 0 };
                        GetCursorPos(&mut point);
                        SetForegroundWindow(hwnd);
                        let selected = TrackPopupMenu(
                            menu,
                            TPM_RETURNCMD | TPM_RIGHTBUTTON,
                            point.x,
                            point.y,
                            0,
                            hwnd,
                            ptr::null(),
                        );
                        DestroyMenu(menu);
                        match selected {
                            1 => host.show(),
                            2 => {
                                host.show();
                                host.navigate(4);
                            }
                            3 => host.exit(),
                            _ => (),
                        };
                        PostMessageW(hwnd, WM_NULL, 0, 0);
                    }
                    _ => (),
                };
                0
            }
            WM_CLOSE => {
                host.exit();
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

pub fn run(
    start_in_tray: bool,
    page: i32,
    test: bool,
    selected: Option<String>,
) -> Result<(), String> {
    // An OpenGL driver's process-global allocations outlive a hidden window.
    // The bounded utility renders smoothly in software without that tray cost.
    // Explicit developer backend overrides remain available for comparison.
    let backend = std::env::var("SLINT_BACKEND").unwrap_or_default();
    let software = backend.trim().is_empty() || backend == "winit-software";
    if std::env::var("SLINT_BACKEND")
        .ok()
        .is_none_or(|value| value.trim().is_empty())
    {
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("software".into())
            .select()
            .map_err(error)?;
    }
    let window = AppWindow::new().map_err(error)?;
    // Optional, explicit developer fixture persistence. A marker and a direct
    // child of the OS temp directory prevent writes into real user data.
    let test_settings = if test {
        std::env::var_os("OMEN_UI_TEST_DIR")
            .map(|path| -> Result<ProfileStore, String> {
                let path = std::fs::canonicalize(path).map_err(error)?;
                let temp = std::fs::canonicalize(std::env::temp_dir()).map_err(error)?;
                if path.parent() != Some(temp.as_path())
                    || !path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("omen-ui-settings-"))
                    || !std::fs::read_to_string(path.join("ui-test-only.marker"))
                        .is_ok_and(|text| text.trim_end() == "Omen Hub++ isolated UI settings")
                {
                    return Err("Invalid isolated UI settings directory".into());
                }
                ProfileStore::at(path)
            })
            .transpose()?
    } else {
        None
    };
    let d = window.global::<Workbench>();
    d.on_tr(|key| t(key.as_str()).into());
    d.set_test_mode(test);
    d.set_admin(crate::single_instance::elevated());
    d.set_page(page);
    let (tx, rx) = mpsc::channel();
    let (control, controls) = mpsc::channel();
    let (preset, presets) = mpsc::channel();
    let (ptx, prx) = mpsc::channel();
    let (aux, auxq) = mpsc::channel();
    let (atx, arx) = mpsc::channel();
    let (apply, apply_tx) = apply::Handle::new();
    let group_rows = (0..preset_fields::GROUPS.len())
        .map(|_| Rc::new(VecModel::default()))
        .collect::<Vec<_>>();
    let groups = Rc::new(VecModel::from(
        preset_fields::GROUPS
            .iter()
            .enumerate()
            .map(|(i, k)| GroupData {
                title: t(k).into(),
                // GPU, Dynamic Boost and the CPU limits open, as before the regrouping.
                expanded: i <= 2,
                rows: group_rows[i].clone().into(),
            })
            .collect::<Vec<_>>(),
    ));
    d.set_groups(groups.clone().into());
    let host = Rc::new(Host {
        fan_library: Default::default(),
        library_attempted: Cell::new(false),
        profile_fan_points: Default::default(),
        profile_fan_changed: Cell::new(false),
        profile_fan_invalid: Cell::new(false),
        profile_fan_choices: Default::default(),
        window: window.as_weak(),
        about: RefCell::new(None),
        hwnd: Cell::new(ptr::null_mut()),
        icon_hwnd: Cell::new(ptr::null_mut()),
        test,
        test_settings,
        tx,
        control,
        shared: Arc::new(Mutex::new(None)),
        power: Arc::new(Mutex::new(Sampling::default())),
        manual: Arc::new(Mutex::new(None)),
        automatic: Arc::new(Mutex::new(None)),
        preset,
        preset_results: prx,
        aux,
        aux_results: arx,
        view: RefCell::new(None),
        fields: RefCell::new(vec![]),
        group_rows,
        groups,
        cores: RefCell::new(vec![]),
        baseline_settings: RefCell::new(None),
        profiles: RefCell::new(vec![]),
        battery_entries: RefCell::new(vec![]),
        lighting: RefCell::new(None),
        lighting_draft: RefCell::new(None),
        lighting_attempted: Cell::new(test),
        curve: RefCell::new(None),
        gpu_draft: RefCell::new(None),
        fan_draft: RefCell::new(None),
        tray: Default::default(),
        taskbar: unsafe {
            RegisterWindowMessageW(crate::native_ui::wide("TaskbarCreated").as_ptr())
        },
        visible: Cell::new(!start_in_tray),
        exit_after: Cell::new(false),
        pending_select: Cell::new(None),
        new_curve_on_load: Cell::new(false),
        settings_loaded: Cell::new(false),
        loading_presets: Cell::new(false),
        appearance_draft: Default::default(),
        appearance_tokens: Cell::new(
            omen_core::appearance::Colors::default()
                .tokens()
                .expect("Valid built-in palette"),
        ),
        pacing: RefCell::new(pacing::Pacer::new(software)),
        pacing_timer: Default::default(),
        color_picker: Default::default(),
        notice_timer: Default::default(),
        preset_activity: Cell::new(None),
        after_save: Default::default(),
        apply,
        operation_busy: Cell::new(false),
        was_minimized: Cell::new(false),
        surface_timer: Default::default(),
    });
    let initial_appearance = if test {
        host.test_settings
            .as_ref()
            .map(|s| s.ui_appearance())
            .transpose()?
            .unwrap_or_default()
    } else {
        ProfileStore::user_data()
            .and_then(|s| s.ui_appearance())
            .unwrap_or_default()
    };
    host.show_appearance(initial_appearance);
    host.keyboard_layout(if test {
        crate::i18n::language_code() == "de"
    } else {
        (unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout(0) } as usize
            & 0x3ff)
            == 7
    });

    bind(&window, &host);
    let class = crate::native_ui::wide(if test {
        "OmenHubSlintTestEvents"
    } else {
        ui::CLASS
    });
    let module = unsafe { GetModuleHandleW(ptr::null()) };
    let wc = WNDCLASSW {
        lpfnWndProc: Some(event_proc),
        hInstance: module,
        lpszClassName: class.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    if unsafe { RegisterClassW(&wc) } == 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            crate::native_ui::wide(omen_core::APP_NAME).as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            module,
            Rc::as_ptr(&host).cast(),
        )
    };
    if hwnd.is_null() {
        unsafe {
            UnregisterClassW(class.as_ptr(), module);
        }
        return Err(error(std::io::Error::last_os_error()));
    }
    host.hwnd.set(hwnd);
    let notifications = if test {
        None
    } else {
        match crate::power_events::Notifications::register(hwnd) {
            Ok(notifications) => Some(notifications),
            Err(e) => {
                unsafe {
                    DestroyWindow(hwnd);
                    UnregisterClassW(class.as_ptr(), module);
                }
                return Err(e);
            }
        }
    };
    let mut workers = vec![];
    if test {
        window.global::<Workbench>().set_hp_disabled(true);
        let profiles = if std::env::var_os("OMEN_UI_TEST_LONG_CHOICES").is_some() {
            let binding = test_view(None, &[]).binding;
            (1..=24)
                .map(|i| PerformanceProfile {
                    schema: omen_core::profiles::PROFILE_SCHEMA,
                    id: format!("ui-long-{i}"),
                    name: tf("ui.test_long_preset", &[&i]),
                    binding: binding.clone(),
                    settings: vec![],
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        host.display(test_view(None, &profiles), t("ui.test_mode"));
        host.init_lighting(
            [
                [115, 160, 200],
                [165, 120, 175],
                [170, 140, 100],
                [110, 175, 150],
            ],
            true,
        );
        host.show_settings(
            settings::Snapshot {
                options: host
                    .test_settings
                    .as_ref()
                    .map(|s| s.app_settings())
                    .transpose()?
                    .unwrap_or_default(),
                appearance: host
                    .test_settings
                    .as_ref()
                    .map(|s| s.ui_appearance())
                    .transpose()?
                    .unwrap_or_default(),
                run_entry: None,
            },
            false,
        );
        let shared = host.shared.clone();
        let notify = hwnd as usize;
        workers.push(thread::spawn(move || {
            *shared.lock().unwrap_or_else(|e| e.into_inner()) = Some(Snapshot {
                inventory: omen_core::Inventory {
                    model: t("ui.test_device").into(),
                    board: "TEST".into(),
                    bios: "TEST".into(),
                    cpus: vec![format!("CPU · {}", t("ui.test_data"))],
                    gpus: vec![format!("GPU · {}", t("ui.test_data"))],
                    ..Default::default()
                },
                ..Default::default()
            });
            let _ = ptx.send(preset_ui::Update::List(Ok((profiles, None, false))));
            unsafe {
                PostMessageW(notify as HWND, ui::UPDATE, 0, 0);
                PostMessageW(notify as HWND, preset_ui::UPDATE, 0, 0);
            }
        }));
    } else {
        unsafe {
            crate::single_instance::accept_activation(hwnd);
            ChangeWindowMessageFilterEx(hwnd, host.taskbar, MSGFLT_ALLOW, ptr::null_mut());
            ChangeWindowMessageFilterEx(hwnd, navigate_message(), MSGFLT_ALLOW, ptr::null_mut());
            host.tray.ensure(hwnd);
        }
        let raw = hwnd as usize;
        let manual = host.manual.clone();
        workers.push(host.apply.spawn(hwnd as usize, apply_tx));
        workers.push(thread::spawn(move || {
            ui::control_worker(raw, controls, manual)
        }));
        let raw = hwnd as usize;
        let shared = host.shared.clone();
        let automatic = host.automatic.clone();
        let power = host.power.clone();
        let visible = !start_in_tray || !host.tray.registered();
        // Automatic AC/battery switches go through the apply queue; the UI
        // test mode never writes hardware.
        let queue = host.apply.automatic();
        let test = host.test;
        let submit: Box<dyn Fn(omen_core::battery::Switch) -> bool + Send> =
            Box::new(move |switch| test || queue.submit(switch));
        workers.push(thread::spawn(move || {
            ui::worker(
                raw,
                rx,
                shared,
                visible,
                automatic,
                true,
                power,
                Some(submit),
            )
        }));
        let raw = hwnd as usize;
        workers.push(thread::spawn(move || preset_ui::worker(raw, presets, ptx)));
        let raw = hwnd as usize;
        workers.push(thread::spawn(move || aux_worker(raw, auxq, atx)));
        thread::spawn(crate::autostart::migrate);
        crate::preset_hold::spawn();
        crate::fan_watch::spawn();
        let _ = host.preset.send(preset_ui::Request::List);
        if page < 2 && selected.is_some() {
            host.loading_presets.set(true);
            host.send_preset(preset_ui::Request::Load(selected));
        }
        if page == 4 {
            host.navigate(4);
        }
        if page == 3 {
            host.navigate(3);
        }
        host.status(
            &ui::last_apply_status().unwrap_or_else(|| t("ui.loading").into()),
            false,
        );
    }
    window.window().on_close_requested({
        let h = host.clone();
        move || {
            h.close_window();
            slint::CloseRequestResponse::KeepWindowShown
        }
    });
    window.window().on_winit_window_event({
        let h = host.clone();
        move |window, event| {
            if matches!(
                event,
                winit::event::WindowEvent::Resized(_)
                    | winit::event::WindowEvent::Focused(_)
                    | winit::event::WindowEvent::Occluded(_)
            ) {
                window.with_winit_window(|n| {
                    let minimized = n.is_minimized().unwrap_or(false);
                    let restored = h.was_minimized.replace(minimized) && !minimized;
                    let visible = n.is_visible().unwrap_or(true) && !minimized;
                    if h.visible.replace(visible) != visible {
                        let _ = h.tx.send(ui::Request::Visibility(visible));
                        if visible {
                            h.arm_notice();
                        } else {
                            h.notice_timer.stop();
                        }
                    }
                    if restored || matches!(event, winit::event::WindowEvent::Occluded(false)) {
                        h.refresh_surface();
                    }
                });
            }
            if matches!(
                event,
                winit::event::WindowEvent::Moved(_)
                    | winit::event::WindowEvent::Resized(_)
                    | winit::event::WindowEvent::ScaleFactorChanged { .. }
            ) {
                h.pacing.borrow_mut().refresh_known = false;
            }
            if matches!(event, winit::event::WindowEvent::RedrawRequested)
                && h.pacing.borrow().enabled
                && h.visible.get()
            {
                if !h.pacing.borrow().refresh_known {
                    let rate = window
                        .with_winit_window(|w| {
                            w.current_monitor()
                                .and_then(|m| m.refresh_rate_millihertz())
                        })
                        .flatten();
                    h.pacing.borrow_mut().set_refresh(rate);
                }
                let delay = h.pacing.borrow_mut().delay(std::time::Instant::now());
                if let Some(delay) = delay {
                    if !h.pacing_timer.running() {
                        let weak = Rc::downgrade(&h);
                        h.pacing_timer.start(
                            slint::TimerMode::SingleShot,
                            pacing::timer_delay(delay),
                            move || {
                                if let Some(h) = weak.upgrade()
                                    && h.visible.get()
                                    && let Some(w) = h.window.upgrade()
                                {
                                    w.window()
                                        .with_winit_window(|native| native.request_redraw());
                                }
                            },
                        );
                    }
                    return EventResult::PreventDefault;
                }
                h.pacing_timer.stop();
            }
            if matches!(
                event,
                winit::event::WindowEvent::Resized(_) | winit::event::WindowEvent::Focused(_)
            ) {
                h.update_icon();
            }
            if let winit::event::WindowEvent::Resized(_) = event {
                h.window_state();
                window.with_winit_window(|native| {
                    let visible = !native.is_minimized().unwrap_or(false)
                        && native.is_visible().unwrap_or(true);
                    if h.visible.replace(visible) != visible {
                        let _ = h.tx.send(ui::Request::Visibility(visible));
                    }
                });
            }
            EventResult::Propagate
        }
    });
    if (!start_in_tray || test || !host.tray.registered())
        && let Err(e) = window.show()
    {
        let _ = host.tx.send(ui::Request::Stop);
        let _ = host.control.send(ui::ControlRequest::Stop);
        let _ = host.preset.send(preset_ui::Request::Stop);
        let _ = host.aux.send(Aux::Stop);
        drop(notifications);
        unsafe {
            host.tray.remove(hwnd);
            DestroyWindow(hwnd);
            UnregisterClassW(class.as_ptr(), module);
        }
        return Err(error(e));
    }
    let weak = window.as_weak();
    let weak_host = Rc::downgrade(&host);
    slint::Timer::single_shot(std::time::Duration::ZERO, move || {
        if let (Some(w), Some(h)) = (weak.upgrade(), weak_host.upgrade()) {
            w.window().set_size(if h.test {
                slint::LogicalSize::new(860.0, 540.0)
            } else {
                slint::LogicalSize::new(1000.0, 720.0)
            });
            let visible = h.visible_hwnd();
            unsafe {
                SendMessageW(
                    visible,
                    WM_SETICON,
                    ICON_BIG as usize,
                    crate::native_ui::app_icon(SM_CXICON) as isize,
                );
                SendMessageW(
                    visible,
                    WM_SETICON,
                    ICON_SMALL as usize,
                    crate::native_ui::app_icon(SM_CXSMICON) as isize,
                );
            }
        }
    });
    let result = slint::run_event_loop_until_quit().map_err(error);
    host.apply.stop();
    let _ = host.tx.send(ui::Request::Stop);
    let _ = host.control.send(ui::ControlRequest::Stop);
    let _ = host.preset.send(preset_ui::Request::Stop);
    let _ = host.aux.send(Aux::Stop);
    drop(notifications);
    unsafe {
        host.tray.remove(hwnd);
        DestroyWindow(hwnd);
        UnregisterClassW(class.as_ptr(), module);
    }
    for worker in workers {
        if worker.is_finished() {
            let _ = worker.join();
        }
    }
    result
}

/// All tool-entry commands activate the same Slint shell, retaining drafts.
pub fn entry(page: i32, selected: Option<String>) -> Result<(), String> {
    if let Some(id) = &selected {
        omen_core::profiles::validate_id(id)?;
    }
    if crate::single_instance::running(ui::KIND) {
        crate::single_instance::activate(ui::CLASS, true);
        let class = crate::native_ui::wide(ui::CLASS);
        unsafe {
            let hwnd = FindWindowW(class.as_ptr(), ptr::null());
            if !hwnd.is_null() {
                PostMessageW(hwnd, navigate_message(), page.clamp(0, 4) as usize, 0);
            }
        }
        return Ok(());
    }
    if !crate::single_instance::elevated() {
        let command = match page {
            1 => "presets-ui",
            2 => "fan-ui",
            3 => "keyboard-ui",
            4 => "settings-ui",
            _ => "--show",
        };
        let args = format!(
            "--lang {} {} {}",
            crate::i18n::language_code(),
            command,
            selected.as_deref().unwrap_or("")
        );
        if crate::single_instance::relaunch_elevated(args.trim()) {
            return Ok(());
        }
    }
    let Some(_instance) = crate::single_instance::claim(ui::KIND) else {
        crate::single_instance::activate(ui::CLASS, true);
        return Ok(());
    };
    run(false, page, false, selected)
}

fn test_view(id: Option<&str>, profiles: &[PerformanceProfile]) -> preset_ui::View {
    let applied_delta = if std::env::var_os("OMEN_UI_TEST_CURVE_APPLIED").is_some() {
        60000
    } else {
        0
    };
    preset_ui::View {
        profiles: profiles.to_vec(),
        selected: id.and_then(|id| profiles.iter().find(|p| p.id == id).cloned()),
        binding: DeviceBinding {
            manufacturer: "UI TEST".into(),
            board: "TEST".into(),
            bios: "TEST".into(),
            cpu_names: vec![],
            nvidia_uuid: Some("TEST-GPU".into()),
            nvidia_driver: Some("TEST-DRIVER".into()),
            nvidia_vbios: Some("TEST-VBIOS".into()),
        },
        defaults: BTreeMap::new(),
        curve: Ok(omen_core::vf::VfState {
            nodes: (0..80)
                .map(|i| omen_core::vf::VfNode {
                    index: i,
                    voltage_uv: 450000 + u32::from(i) * 10000,
                    frequency_khz: 400000 + u32::from(i) * 30000 + applied_delta as u32,
                    delta_khz: applied_delta,
                })
                .collect(),
            minimum_delta_khz: -1000000,
            maximum_delta_khz: 1000000,
            first_index: 0,
            last_index: 79,
        }),
        removed_options: 0,
        pending: std::env::var_os("OMEN_UI_TEST_PENDING").is_some(),
        applicable: false,
        blockers: vec![],
        startup_enabled: false,
        startup_valid: false,
        battery: None,
    }
}

fn bind(w: &AppWindow, h: &Rc<Host>) {
    let d = w.global::<Workbench>();
    {
        let editor = w.global::<ColorEditor>();
        let host = h.clone();
        editor.on_hsv(move |h, s, v| host.picker_hsv(h, s, v));
        let host = h.clone();
        editor.on_hex_edited(move |text| host.picker_hex(text.as_str()));
        let host = h.clone();
        editor.on_swatch(move |i| host.picker_swatch(i));
        let host = h.clone();
        editor.on_accept(move || host.close_color_picker(true));
        let host = h.clone();
        editor.on_cancel(move || host.close_color_picker(false));
    }
    {
        let h = h.clone();
        d.on_show_about(move || h.show_about());
    }
    macro_rules! callback {($method:ident,|$host:ident $(,$arg:ident)*|$body:block)=>{{let $host=h.clone();d.$method(move|$($arg),*|$body);}};}
    callback!(on_navigate, |h, page| {
        h.navigate(page);
    });
    callback!(on_window_action, |h, action| {
        h.window_action(action);
    });
    callback!(on_refresh, |h| {
        let _ = h.tx.send(ui::Request::Refresh);
    });
    callback!(on_open_color_picker, |h, target| {
        h.open_color_picker(target);
    });
    callback!(on_palette_changed, |h, index| {
        h.select_palette(index);
    });
    callback!(on_colors_changed, |h| {
        h.edit_colors();
    });
    callback!(on_reset_appearance, |h| {
        if !h.blocked() {
            h.show_appearance(Default::default());
        }
    });
    callback!(on_name_edited, |h, _name| {
        h.dirty();
    });
    callback!(on_select_preset, |h, index| {
        h.select(index);
    });
    callback!(on_new_preset, |h| {
        h.select(0);
    });
    callback!(on_include_field, |h, index, value| {
        if let Some(row) = h.fields.borrow_mut().get_mut(index as usize) {
            row.included = value;
        }
        h.dirty();
        // Dynamic Boost changes the CPU watt slider's range.
        if preset_fields::fields()
            .get(index as usize)
            .is_some_and(|f| f.feature == Feature::GpuUnleashed)
        {
            h.update_fields();
        } else {
            h.update_field(index as usize);
        }
    });
    callback!(on_slider_field, |h, index, watts| {
        if let Some(row) = h.fields.borrow_mut().get_mut(index as usize) {
            row.text = format!("{}", watts.round() as i64);
        }
        h.dirty();
        h.update_field(index as usize);
    });
    callback!(on_edit_field, |h, index, text| {
        if let Some(row) = h.fields.borrow_mut().get_mut(index as usize) {
            row.text = text.to_string();
        }
        h.dirty();
        h.update_field(index as usize);
    });
    callback!(on_choice_field, |h, index, choice| {
        if let Some(field) = preset_fields::fields().get(index as usize)
            && let Some(value) = field.choices().get(choice as usize)
            && let Some(row) = h.fields.borrow_mut().get_mut(index as usize)
        {
            row.value = Value::Choice(*value);
        }
        h.dirty();
        h.update_field(index as usize);
    });
    callback!(on_edit_core, |h, index, text| {
        if let Some(w) = h.window.upgrade() {
            let rows = w.global::<Workbench>().get_cores();
            if let Some(model) = rows.as_any().downcast_ref::<VecModel<CoreRow>>()
                && let Some(mut row) = model.row_data(index as usize)
            {
                row.value = text;
                model.set_row_data(index as usize, row);
            }
        }
        h.dirty();
    });
    callback!(on_toggle_group, |h, index| {
        if let Some(mut group) = h.groups.row_data(index as usize) {
            group.expanded = !group.expanded;
            h.groups.set_row_data(index as usize, group);
        }
    });
    callback!(on_startup_edited, |h, value| {
        if let Some(w) = h.window.upgrade() {
            w.global::<Workbench>().set_preset_startup(value);
        }
        h.dirty();
    });
    callback!(on_save_preset, |h| {
        h.save_preset();
    });
    callback!(on_apply_preset, |h| {
        let id = h
            .view
            .borrow()
            .as_ref()
            .and_then(|v| v.selected.as_ref().map(|p| p.id.clone()));
        if let Some(id) = id {
            h.queue_apply(apply::Target::Preset(id));
        }
    });
    callback!(on_delete_preset, |h| {
        if let Some(w) = h.window.upgrade() {
            w.global::<Workbench>().set_confirm_kind(2);
        }
    });
    callback!(on_reset_preset, |h| {
        let view = h.view.borrow();
        for (field, row) in preset_fields::fields()
            .iter()
            .zip(h.fields.borrow_mut().iter_mut())
        {
            row.value = view
                .as_ref()
                .and_then(|v| v.defaults.get(&field.feature))
                .cloned()
                .unwrap_or_else(|| field.initial());
            row.included = matches!(row.value, Value::Boolean(true));
            row.text = match row.value {
                Value::Integer(v) => {
                    crate::i18n::number(&omen_core::preset::format_number(v, field.scale()))
                }
                _ => String::new(),
            };
        }
        drop(view);
        *h.gpu_draft.borrow_mut() = None;
        if let Some(w) = h.window.upgrade() {
            w.global::<Workbench>().set_preset_startup(false);
        }
        h.dirty();
        h.update_fields();
    });
    callback!(on_cancel_preset, |h| {
        h.cancel_preset_edit();
    });
    callback!(on_profile_fan_selected, |h, index| {
        h.choose_profile_fan(index);
    });
    callback!(on_fan_library_selected, |h, index| {
        h.choose_fan_library(index);
    });
    callback!(on_fan_library_new, |h| {
        h.choose_fan_library(0);
    });
    callback!(on_fan_library_name_edited, |h, name| {
        h.edit_fan_library_name(name.as_str());
    });
    callback!(on_fan_library_refresh, |h| {
        if !h.fan_library_dirty() && !h.blocked() {
            h.library_attempted.set(false);
            h.load_fan_library();
        }
    });
    callback!(on_fan_library_reset, |h| {
        h.reset_fan_library();
    });
    callback!(on_fan_library_save, |h| {
        h.save_fan_library();
    });
    callback!(on_fan_library_delete, |h| {
        h.delete_fan_library();
    });
    callback!(on_fan_library_request, |h| {
        h.request_fan_library();
    });
    callback!(on_battery_selected, |h, _index| {
        h.dirty();
    });
    callback!(on_fan_change, |h, index| {
        if let Some(mode) = FanMode::from_index(index as usize) {
            h.send_control(ui::ControlRequest::FanMode(mode));
        }
    });
    callback!(on_graphics_change, |h, index| {
        if let Some(mode) = GraphicsMode::from_index(index as usize) {
            h.send_control(ui::ControlRequest::Graphics(mode));
        }
    });
    callback!(on_open_curve, |h, gpu| {
        h.open_curve(gpu);
    });
    callback!(on_curve_action, |h, action| {
        h.curve_action(action.as_str());
    });
    callback!(on_select_point, |h, index| {
        if let Some(e) = h.curve.borrow_mut().as_mut()
            && (index as usize) < e.curve.points.len()
        {
            e.curve.select(index as usize, false);
        }
        h.draw_curve();
    });
    callback!(on_chart_event, |h, kind, x, y, w, ht, shift, ctrl, alt| {
        h.chart_event(
            kind,
            x,
            y,
            (w, ht),
            omen_core::curve_input::Modifiers { shift, ctrl, alt },
        );
    });
    callback!(on_chart_wheel, |h, dx, dy, x, y, shift, ctrl| {
        h.chart_wheel(dx, dy, x, y, shift, ctrl);
    });
    callback!(on_chart_hover, |h, x, y, w, ht| {
        h.chart_hover(x, y, (w, ht));
    });
    callback!(on_chart_resized, |h| {
        h.draw_curve();
    });
    callback!(on_chart_key, |h, key, shift, ctrl, alt| {
        h.chart_key(key.as_str(), shift, ctrl, alt)
    });
    callback!(on_save_settings, |h| {
        h.save_settings();
    });
    callback!(on_confirm_save, |h| {
        h.save_before_switch();
    });
    callback!(on_dismiss_notice, |h| {
        h.notice_timer.stop();
        if let Some(w) = h.window.upgrade() {
            w.global::<Workbench>().set_notice_visible(false);
        }
    });
    callback!(on_notice_details, |h| {
        h.notice_timer.stop();
        if let Some(w) = h.window.upgrade() {
            w.global::<Workbench>().set_confirm_kind(5);
        }
    });
    callback!(on_close_notice_details, |h| {
        if let Some(w) = h.window.upgrade() {
            w.global::<Workbench>().set_confirm_kind(0);
        }
        h.arm_notice();
    });
    callback!(on_settings_changed, |h| {
        if let Some(w) = h.window.upgrade() {
            w.global::<Workbench>()
                .set_settings_dirty(h.settings_dirty());
        }
    });
    callback!(on_lighting_power, |h| {
        if !h.blocked() {
            if let Some(d) = h.lighting_draft.borrow_mut().as_mut() {
                d.enabled = !d.enabled;
            }
            h.draw_lighting(false);
        }
    });
    callback!(on_select_lighting_group, |h, index| {
        h.select_lighting_group(index);
    });
    callback!(on_keyboard_layout, |h, german| {
        h.keyboard_layout(german);
    });
    callback!(on_edit_lighting_color, |h, value| {
        h.edit_lighting_color(value.as_str());
    });
    callback!(on_edit_lighting_brightness, |h, value| {
        h.edit_lighting_brightness(value.as_str());
    });
    callback!(on_lighting_level_edited, |h, value| {
        let value = value.round().clamp(0.0, 100.0).to_string();
        if let Some(w) = h.window.upgrade() {
            w.global::<Workbench>()
                .set_lighting_brightness(value.as_str().into());
        }
        h.edit_lighting_brightness(&value);
    });
    callback!(on_pick_lighting_color, |h| {
        h.pick_lighting_color();
    });
    callback!(on_apply_lighting, |h| {
        h.apply_lighting();
    });
    callback!(on_reset_lighting, |h| {
        if !h.blocked() {
            if let Some(d) = h.lighting_draft.borrow_mut().as_mut() {
                d.reset();
            }
            if let Some(w) = h.window.upgrade() {
                w.global::<Workbench>().set_lighting_error("".into());
            }
            h.draw_lighting(true);
        }
    });
    callback!(on_refresh_lighting, |h| {
        if !h.lighting_dirty() && !h.test {
            h.send_aux(Aux::Lighting(None));
        }
    });
    callback!(on_hp_service, |h| {
        h.status(t("ui.hp_explicit"), false);
        if let Some(w) = h.window.upgrade() {
            h.send_aux(Aux::HpService(!w.global::<Workbench>().get_hp_disabled()));
        }
    });
    callback!(on_confirm, |h, accept| {
        let Some(w) = h.window.upgrade() else { return };
        let d = w.global::<Workbench>();
        let kind = d.get_confirm_kind();
        d.set_confirm_kind(0);
        if !accept {
            if kind == 1 {
                let old = h
                    .view
                    .borrow()
                    .as_ref()
                    .and_then(|v| v.selected.as_ref().map(|p| p.id.clone()));
                let index = old
                    .and_then(|id| h.profiles.borrow().iter().position(|p| p.id == id))
                    .map_or(0, |i| i as i32 + 1);
                d.set_profile_index(index);
            }
            h.pending_select.set(None);
            return;
        }
        match kind {
            1 => {
                d.set_dirty(false);
                if let Some(index) = h.pending_select.take() {
                    h.load_preset(index);
                }
            }
            2 => {
                let id = h
                    .view
                    .borrow()
                    .as_ref()
                    .and_then(|v| v.selected.as_ref().map(|p| p.id.clone()));
                if let Some(id) = id {
                    if h.test {
                        h.status(t("ui.test_no_write"), true);
                    } else {
                        h.send_preset(preset_ui::Request::Delete(id));
                    }
                }
            }
            3 => {
                d.set_dirty(false);
                let _ = slint::quit_event_loop();
            }
            _ => (),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn battery_choices_are_other_presets_the_template_and_the_stored_choice() {
        let preset = |id: &str| PerformanceProfile {
            schema: omen_core::profiles::PROFILE_SCHEMA,
            id: id.into(),
            name: id.into(),
            binding: test_view(None, &[]).binding,
            settings: vec![],
        };
        let template = Selection::Preset(crate::battery_preset::ID.into());
        // Nothing saved yet: only the template, labelled as not yet created.
        let fresh = battery_entries(&[], None, None);
        assert_eq!(fresh, vec![template.clone()]);
        assert_eq!(selection_label(&fresh[0], &[]), t("battery.optimized_new"));
        // The edited preset never names itself; other presets are offered.
        let profiles = [preset("preset-gaming"), preset("preset-office")];
        assert_eq!(
            battery_entries(&profiles, Some("preset-gaming"), None),
            vec![Selection::Preset("preset-office".into()), template.clone()]
        );
        // A deleted preset and an older HP mode stay distinct selections.
        let missing = Selection::Preset("removed-preset".into());
        assert_eq!(
            battery_entries(&profiles, Some("preset-gaming"), Some(&missing)).last(),
            Some(&missing)
        );
        let mode = Selection::Mode(HpMode::Quiet);
        assert_eq!(
            battery_entries(&[], None, Some(&mode)),
            vec![template.clone(), mode]
        );
        // The existing template is listed once, and not for itself.
        let with_template = [preset("preset-gaming"), preset(crate::battery_preset::ID)];
        assert_eq!(
            battery_entries(&with_template, Some("preset-gaming"), None),
            vec![template]
        );
        assert_eq!(
            battery_entries(&with_template, Some(crate::battery_preset::ID), None),
            vec![Selection::Preset("preset-gaming".into())]
        );
    }
}
