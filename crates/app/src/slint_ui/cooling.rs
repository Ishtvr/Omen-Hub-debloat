//! Named fan curves are metadata until the explicit request action. Performance
//! profiles store point snapshots, so library changes cannot alter them silently.
use super::*;
use omen_core::{fan_profile::FanPreset, profiles::FanPoint};

pub(super) struct Snapshot {
    pub presets: Vec<FanPreset>,
    pub active: Result<Option<Vec<FanPoint>>, String>,
    pub board: String,
    pub bios: String,
}
pub(super) struct Library {
    pub entries: Vec<FanPreset>,
    pub initial: FanPreset,
    pub baseline: Option<FanPreset>,
    pub draft: FanPreset,
}
#[derive(Clone)]
pub(super) enum Choice {
    KeepInvalid,
    Unchanged,
    Curve(Vec<FanPoint>),
}
fn initial(board: &str, bios: &str, points: Option<Vec<FanPoint>>) -> FanPreset {
    let points = points.unwrap_or_else(|| {
        let Value::FanCurve(p) = Curve::fan(Kind::CpuFan, None).unwrap().value().unwrap() else {
            unreachable!()
        };
        p
    });
    FanPreset {
        schema: 1,
        id: format!("fan-{}", crate::native_ui::token()),
        name: String::new(),
        board: board.into(),
        bios: bios.into(),
        points,
    }
}
impl Host {
    pub(super) fn load_fan_library(&self) {
        if self.blocked() || self.library_attempted.get() {
            return;
        }
        let binding = self
            .view
            .borrow()
            .as_ref()
            .map(|v| (v.binding.board.clone(), v.binding.bios.clone()));
        let (board, bios) = binding
            .or_else(|| {
                self.shared
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .as_ref()
                    .map(|s| (s.inventory.board.clone(), s.inventory.bios.clone()))
            })
            .unwrap_or_default();
        if board.is_empty() || bios.is_empty() {
            return;
        }
        self.library_attempted.set(true);
        if self.test {
            self.show_fan_library(
                Snapshot {
                    presets: vec![],
                    active: Ok(None),
                    board,
                    bios,
                },
                None,
            );
        } else {
            self.send_aux(Aux::FanLibrary(board, bios));
        }
    }
    pub(super) fn show_fan_library(&self, snapshot: Snapshot, select: Option<String>) {
        let entries: Vec<_> = snapshot
            .presets
            .into_iter()
            .filter(|p| p.board == snapshot.board && p.bios == snapshot.bios)
            .collect();
        let active = match snapshot.active {
            Ok(v) => v,
            Err(e) => {
                self.status(&crate::i18n::error_message(&e), true);
                None
            }
        };
        let selected = select
            .as_ref()
            .and_then(|id| entries.iter().find(|p| &p.id == id))
            .cloned()
            .or_else(|| {
                entries
                    .iter()
                    .find(|p| Some(&p.points) == active.as_ref())
                    .cloned()
            })
            .or_else(|| entries.first().cloned());
        let draft = selected
            .clone()
            .unwrap_or_else(|| initial(&snapshot.board, &snapshot.bios, active));
        *self.fan_library.borrow_mut() = Some(Library {
            entries,
            initial: draft.clone(),
            baseline: selected,
            draft,
        });
        *self.fan_draft.borrow_mut() = None;
        self.sync_fan_library();
        self.sync_profile_fan();
    }
    pub(super) fn sync_fan_library(&self) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let lib = self.fan_library.borrow();
        let Some(lib) = lib.as_ref() else {
            return;
        };
        let d = w.global::<Workbench>();
        d.set_fan_library_ready(true);
        d.set_fan_library_names(strings(
            std::iter::once(t("fan_profile.new").to_owned())
                .chain(lib.entries.iter().map(|p| p.name.clone())),
        ));
        d.set_fan_library_index(
            lib.baseline
                .as_ref()
                .and_then(|p| lib.entries.iter().position(|f| f.id == p.id))
                .map_or(0, |i| i as i32 + 1),
        );
        d.set_fan_library_name(lib.draft.name.as_str().into());
        d.set_fan_library_saved(lib.baseline.is_some());
        d.set_fan_library_dirty(self.fan_library_dirty());
    }
    pub(super) fn fan_library_dirty(&self) -> bool {
        let lib = self.fan_library.borrow();
        let Some(lib) = lib.as_ref() else {
            return false;
        };
        let pending = |c: &Curve| {
            c.kind != Kind::Gpu && c.value().ok() != Some(Value::FanCurve(lib.draft.points.clone()))
        };
        lib.draft != lib.initial
            || self.fan_draft.borrow().as_ref().is_some_and(pending)
            || self
                .curve
                .borrow()
                .as_ref()
                .is_some_and(|e| pending(&e.curve))
    }
    pub(super) fn choose_fan_library(&self, index: i32) {
        if self.blocked() || self.fan_library_dirty() {
            return;
        }
        {
            let mut state = self.fan_library.borrow_mut();
            let Some(lib) = state.as_mut() else {
                return;
            };
            let selected = usize::try_from(index - 1)
                .ok()
                .and_then(|i| lib.entries.get(i))
                .cloned();
            let draft = selected
                .clone()
                .unwrap_or_else(|| initial(&lib.draft.board, &lib.draft.bios, None));
            lib.baseline = selected;
            lib.initial = draft.clone();
            lib.draft = draft;
        }
        *self.fan_draft.borrow_mut() = None;
        self.sync_fan_library();
    }
    pub(super) fn edit_fan_library_name(&self, name: &str) {
        if let Some(lib) = self.fan_library.borrow_mut().as_mut() {
            lib.draft.name = name.into();
        }
        if let Some(w) = self.window.upgrade() {
            w.global::<Workbench>()
                .set_fan_library_dirty(self.fan_library_dirty());
        }
    }
    pub(super) fn open_fan_library_curve(&self) {
        if self.blocked() {
            return;
        }
        let result = self.fan_draft.borrow().clone().map(Ok).or_else(|| {
            self.fan_library
                .borrow()
                .as_ref()
                .map(|l| Curve::fan(Kind::CpuFan, Some(&Value::FanCurve(l.draft.points.clone()))))
        });
        match result {
            Some(Ok(c)) => self.open_loaded_curve(c),
            Some(Err(e)) => self.status(&crate::i18n::error_message(&e), true),
            None => self.load_fan_library(),
        }
    }
    pub(super) fn accept_fan_library_curve(&self, points: Vec<FanPoint>) {
        if let Some(lib) = self.fan_library.borrow_mut().as_mut() {
            lib.draft.points = points;
        }
        self.sync_fan_library();
        self.status(t("fan_profile.curve_in_draft"), false);
    }
    pub(super) fn reset_fan_library(&self) {
        if self.blocked() {
            return;
        }
        if let Some(lib) = self.fan_library.borrow_mut().as_mut() {
            lib.draft = lib.initial.clone();
        }
        *self.fan_draft.borrow_mut() = None;
        self.sync_fan_library();
    }
    pub(super) fn save_fan_library(&self) {
        if self.blocked() {
            return;
        }
        let target = self
            .fan_library
            .borrow()
            .as_ref()
            .map(|l| (l.draft.clone(), l.baseline.clone()));
        let Some((draft, baseline)) = target else {
            return;
        };
        if let Err(e) = draft.validate() {
            self.status(&crate::i18n::error_message(&e), true);
            return;
        }
        if self
            .fan_draft
            .borrow()
            .as_ref()
            .is_some_and(|c| c.value().ok() != Some(Value::FanCurve(draft.points.clone())))
        {
            self.status(t("fan_profile.accept_first"), true);
            return;
        }
        if self.test {
            let mut entries = self.fan_library.borrow().as_ref().unwrap().entries.clone();
            entries.retain(|p| p.id != draft.id);
            entries.push(draft.clone());
            self.show_fan_library(
                Snapshot {
                    presets: entries,
                    active: Ok(None),
                    board: draft.board.clone(),
                    bios: draft.bios.clone(),
                },
                Some(draft.id),
            );
            self.saved(t("ui.test_saved"));
        } else {
            self.send_aux(Aux::SaveFanPreset(draft, baseline));
        }
    }
    pub(super) fn delete_fan_library(&self) {
        if self.blocked() || self.fan_library_dirty() {
            return;
        }
        let selected = self
            .fan_library
            .borrow()
            .as_ref()
            .and_then(|l| l.baseline.clone());
        let Some(selected) = selected else {
            return;
        };
        if self.test {
            let mut entries = self.fan_library.borrow().as_ref().unwrap().entries.clone();
            entries.retain(|p| p.id != selected.id);
            self.show_fan_library(
                Snapshot {
                    presets: entries,
                    active: Ok(None),
                    board: selected.board,
                    bios: selected.bios,
                },
                None,
            );
            self.completed(t("fan_profile.deleted"));
        } else {
            self.send_aux(Aux::DeleteFanPreset(selected));
        }
    }
    pub(super) fn request_fan_library(&self) {
        if self.blocked() || self.fan_library_dirty() {
            return;
        }
        let selected = self
            .fan_library
            .borrow()
            .as_ref()
            .and_then(|l| l.baseline.clone());
        if let Some(p) = selected {
            if self.test {
                self.status(t("ui.test_no_write"), true);
            } else {
                self.send_aux(Aux::RequestFanPreset(p));
            }
        }
    }
    pub(super) fn reset_profile_fan(&self) {
        let value = self.view.borrow().as_ref().and_then(|v| {
            v.selected
                .as_ref()
                .map(omen_core::fan_profile::shared_curve)
        });
        let (points, invalid) = match value {
            Some(Ok(p)) => (p, false),
            Some(Err(_)) => (None, true),
            None => (None, false),
        };
        *self.profile_fan_points.borrow_mut() = points;
        self.profile_fan_invalid.set(invalid);
        self.profile_fan_changed.set(false);
        self.sync_profile_fan();
    }
    pub(super) fn sync_profile_fan(&self) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let mut labels = vec![t("fan_profile.unchanged").to_owned()];
        let mut choices = vec![Choice::Unchanged];
        if let Some(lib) = self.fan_library.borrow().as_ref() {
            for p in &lib.entries {
                labels.push(p.name.clone());
                choices.push(Choice::Curve(p.points.clone()));
            }
        }
        let selected = self.profile_fan_points.borrow();
        let index = if self.profile_fan_invalid.get() {
            labels.push(t("fan_profile.unsupported_saved").into());
            choices.push(Choice::KeepInvalid);
            choices.len() - 1
        } else if let Some(points) = selected.as_ref() {
            if let Some(i) = choices
                .iter()
                .position(|c| matches!(c,Choice::Curve(p)if p==points))
            {
                i
            } else {
                labels.push(t("fan_profile.custom_snapshot").into());
                choices.push(Choice::Curve(points.clone()));
                choices.len() - 1
            }
        } else {
            0
        };
        let d = w.global::<Workbench>();
        d.set_profile_fan_choices(strings(labels));
        d.set_profile_fan_index(index as i32);
        d.set_profile_fan_linked(selected.is_some() || self.profile_fan_invalid.get());
        *self.profile_fan_choices.borrow_mut() = choices;
    }
    pub(super) fn choose_profile_fan(&self, index: i32) {
        if self.blocked() {
            return;
        }
        let choice = self
            .profile_fan_choices
            .borrow()
            .get(index as usize)
            .cloned();
        match choice {
            Some(Choice::Unchanged) => *self.profile_fan_points.borrow_mut() = None,
            Some(Choice::Curve(p)) => *self.profile_fan_points.borrow_mut() = Some(p),
            _ => {
                self.sync_profile_fan();
                return;
            }
        }
        self.profile_fan_invalid.set(false);
        self.profile_fan_changed.set(true);
        self.dirty();
        self.sync_profile_fan();
    }
    pub(super) fn append_profile_fan(
        &self,
        edits: &mut Vec<(Feature, Option<Value>)>,
        startup: bool,
    ) -> Result<(), String> {
        if startup && (self.profile_fan_points.borrow().is_some() || self.profile_fan_invalid.get())
        {
            return Err(omen_core::message::msg("fan_profile.err.startup", &[]));
        }
        if self.profile_fan_changed.get() {
            for f in [Feature::CpuFanCurve, Feature::GpuFanCurve] {
                edits.push((
                    f,
                    self.profile_fan_points
                        .borrow()
                        .clone()
                        .map(Value::FanCurve),
                ));
            }
        }
        Ok(())
    }
}
