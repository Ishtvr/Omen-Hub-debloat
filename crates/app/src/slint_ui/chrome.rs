//! Window chrome actions share the native close policy and Windows window state.
use super::*;
impl Host {
    pub(super) fn close_window(&self) {
        let close_tray = self.baseline_settings.borrow().as_ref().map_or_else(
            || ProfileStore::user_data().map_or(true, |s| s.close_to_tray()),
            |s| s.options.close_to_tray,
        );
        if close_tray {
            self.hide();
        } else {
            self.exit();
        }
    }
    pub(super) fn window_state(&self) {
        if let Some(w) = self.window.upgrade() {
            let maximized = w
                .window()
                .with_winit_window(|n| n.is_maximized())
                .unwrap_or(false);
            w.global::<Workbench>().set_window_maximized(maximized);
        }
    }
    pub(super) fn window_action(&self, action: i32) {
        if action == 2 {
            self.close_window();
            return;
        }
        let Some(w) = self.window.upgrade() else {
            return;
        };
        match action {
            0 => {
                self.was_minimized.set(true);
                w.window().with_winit_window(|n| n.set_minimized(true));
            }
            1 => {
                w.window()
                    .with_winit_window(|n| n.set_maximized(!n.is_maximized()));
            }
            3 => unsafe {
                let hwnd = self.visible_hwnd();
                let menu = GetSystemMenu(hwnd, 0);
                if !hwnd.is_null() && !menu.is_null() {
                    let mut point = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut point);
                    let command = TrackPopupMenu(
                        menu,
                        TPM_RETURNCMD | TPM_RIGHTBUTTON,
                        point.x,
                        point.y,
                        0,
                        hwnd,
                        ptr::null(),
                    );
                    if command != 0 {
                        PostMessageW(hwnd, WM_SYSCOMMAND, command as usize, 0);
                    }
                }
            },
            _ => (),
        }
    }
}
