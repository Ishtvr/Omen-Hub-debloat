//! UI result notifications. Loading/telemetry never become success popups.
use super::*;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PresetActivity {
    Load,
    Save,
    Other,
}
pub(super) struct AfterSave {
    target: Option<String>,
}
impl Host {
    pub(super) fn abort_saved_selection(&self) {
        if self.after_save.borrow_mut().take().is_some() {
            self.restore_editor_selection();
        }
    }
    pub(super) fn restore_editor_selection(&self) {
        let selected = self
            .view
            .borrow()
            .as_ref()
            .and_then(|v| v.selected.as_ref().map(|p| p.id.clone()));
        let index = selected
            .and_then(|id| self.profiles.borrow().iter().position(|p| p.id == id))
            .map_or(0, |i| i as i32 + 1);
        if let Some(w) = self.window.upgrade() {
            w.global::<Workbench>().set_profile_index(index);
        }
    }
    pub(super) fn notice(&self, title: &str, text: &str, failed: bool) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        if failed && d.get_notice_visible() && d.get_notice_failed() && d.get_notice_text() == text
        {
            return;
        }
        self.notice_timer.stop();
        d.set_notice_title(title.into());
        d.set_notice_text(text.into());
        d.set_notice_failed(failed);
        d.set_notice_visible(true);
        self.arm_notice();
    }
    pub(super) fn arm_notice(&self) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        if d.get_notice_visible()
            && !d.get_notice_failed()
            && self.visible.get()
            && !self.notice_timer.running()
        {
            let weak = self.window.clone();
            self.notice_timer.start(
                slint::TimerMode::SingleShot,
                std::time::Duration::from_secs(6),
                move || {
                    if let Some(w) = weak.upgrade() {
                        w.global::<Workbench>().set_notice_visible(false);
                    }
                },
            );
        }
    }
    pub(super) fn completed(&self, text: &str) {
        self.notice(t("notice.done"), text, false);
    }
    pub(super) fn saved(&self, text: &str) {
        self.notice(t("notice.saved"), text, false);
    }
    pub(super) fn save_before_switch(&self) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        if w.global::<Workbench>().get_confirm_kind() != 1 || self.is_busy() {
            return;
        }
        let Some(index) = self.pending_select.take() else {
            return;
        };
        let target = if index == 0 {
            None
        } else {
            self.profiles
                .borrow()
                .get((index - 1) as usize)
                .map(|p| p.id.clone())
        };
        if index > 0 && target.is_none() {
            return;
        }
        *self.after_save.borrow_mut() = Some(AfterSave { target });
        w.global::<Workbench>().set_confirm_kind(0);
        self.save_preset();
    }
    pub(super) fn follow_saved_selection(&self) {
        if self.blocked() {
            return;
        }
        let Some(next) = self.after_save.borrow_mut().take() else {
            return;
        };
        let index = match next.target {
            None => Some(0),
            Some(id) => self
                .profiles
                .borrow()
                .iter()
                .position(|p| p.id == id)
                .map(|i| i as i32 + 1),
        };
        if let Some(index) = index {
            self.load_preset(index);
        }
    }
}
