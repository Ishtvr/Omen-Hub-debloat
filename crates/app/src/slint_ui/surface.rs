//! Refresh discarded Windows software buffers without rebuilding UI state.
//! Slint 1.18's public snapshot path resets its partial-render caches. The
//! temporary image is dropped immediately, followed by an actual presentation.
use super::*;
impl Host {
    pub(super) fn refresh_surface(&self) {
        if !self.pacing.borrow().enabled {
            return;
        }
        let weak = self.window.clone();
        self.surface_timer.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(1),
            move || {
                if let Some(w) = weak.upgrade() {
                    let drawable = w
                        .window()
                        .with_winit_window(|n| {
                            n.is_visible().unwrap_or(true) && !n.is_minimized().unwrap_or(false)
                        })
                        .unwrap_or(false);
                    if drawable {
                        drop(w.window().take_snapshot());
                        w.window().request_redraw();
                    }
                }
            },
        );
    }
}
