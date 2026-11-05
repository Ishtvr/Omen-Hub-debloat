//! Central native application settings. No preset approval. The only hardware
//! action is the explicit "Disable HP OMEN Cap" switch, run as its own
//! elevated command.
use crate::{
    i18n::{t, tf},
    native_ui as ui, settings, theme,
};
use omen_core::{app_settings::LanguagePreference, battery::Selection, hp_mode::HpMode};
use std::{
    cell::RefCell,
    ptr,
    sync::{Arc, Mutex},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{
        Controls::{BST_CHECKED, BST_UNCHECKED, DRAWITEMSTRUCT},
        Input::KeyboardAndMouse::EnableWindow,
        WindowsAndMessaging::*,
    },
};

const LANGUAGE: usize = 801;
const AUTOSTART: usize = 802;
const TRAY: usize = 803;
const CLOSE: usize = 804;
const SAVE: usize = 805;
const CANCEL: usize = 806;
const STATUS: i32 = 807;
/// Preset applied automatically without the charger.
const BATTERY: usize = 808;
/// Stops and disables the HP OMEN Cap service (HPOmenCap) at once.
const HP_SERVICE: usize = 809;
/// Rewrites the applied preset's values when firmware changes them.
const HOLD: usize = 818;
/// Round "i" buttons: language, autostart, tray, close, battery, HP service,
/// keep preset. Explanations live only here; the window shows label,
/// control and "i".
const INFO: [usize; 7] = [820, 821, 822, 823, 824, 825, 826];
const INFO_TEXT: [&[&str]; 7] = [
    &["settings.restart_note"],
    &["info.autostart", "settings.startup_note"],
    &["info.start_in_tray"],
    &["settings.close_note"],
    &["info.battery"],
    &["info.hp_service"],
    &["info.hold_preset"],
];
/// The HP service change finished; its status line waits in `hp_result`.
const HP_DONE: u32 = WM_APP + 71;
const MODES: [(&str, HpMode); 4] = [
    ("mode.quiet", HpMode::Quiet),
    ("mode.balanced", HpMode::Balanced),
    ("mode.performance", HpMode::Performance),
    ("mode.unleashed", HpMode::Unleashed),
];
struct State {
    theme: theme::Theme,
    baseline: RefCell<settings::Snapshot>,
    /// Battery selector entries after "Off", in list order.
    battery: RefCell<Vec<Selection>>,
    hp_result: Arc<Mutex<Option<(String, bool)>>>,
}

unsafe fn checked(hwnd: HWND, id: usize) -> bool {
    unsafe { SendMessageW(GetDlgItem(hwnd, id as i32), BM_GETCHECK, 0, 0) == BST_CHECKED as isize }
}
unsafe fn populate(hwnd: HWND, state: &State) {
    let snapshot = state.baseline.borrow();
    unsafe {
        for (id, keys, selected) in [
            (
                LANGUAGE,
                &[
                    "settings.language_system",
                    "settings.language_de",
                    "settings.language_en",
                ][..],
                match snapshot.options.language {
                    LanguagePreference::System => 0,
                    LanguagePreference::German => 1,
                    LanguagePreference::English => 2,
                },
            ),
            (
                CLOSE,
                &["settings.close_tray", "settings.close_exit"][..],
                usize::from(!snapshot.options.close_to_tray),
            ),
        ] {
            let combo = GetDlgItem(hwnd, id as i32);
            for key in keys {
                SendMessageW(combo, CB_ADDSTRING, 0, ui::wide(t(key)).as_ptr() as isize);
            }
            ui::dropdown_width(
                combo,
                state.theme.body,
                &keys.iter().map(|key| t(key)).collect::<Vec<_>>(),
            );
            SendMessageW(combo, CB_SETCURSEL, selected, 0);
        }
        // Battery: "Off", the built-in modes, then the saved presets.
        let mut entries: Vec<(Selection, String)> = MODES
            .iter()
            .map(|(key, mode)| (Selection::Mode(*mode), tf("mode.default_entry", &[&t(key)])))
            .collect();
        if let Ok(profiles) =
            omen_platform::profile_store::ProfileStore::user_data().and_then(|s| s.list())
        {
            entries.extend(
                profiles
                    .into_iter()
                    .map(|p| (Selection::Preset(p.id), p.name)),
            );
        }
        let saved = snapshot
            .options
            .battery_preset
            .as_deref()
            .and_then(Selection::parse);
        let combo = GetDlgItem(hwnd, BATTERY as i32);
        SendMessageW(combo, CB_RESETCONTENT, 0, 0);
        let mut labels = vec![t("battery.off").to_owned()];
        labels.extend(entries.iter().map(|(_, label)| label.clone()));
        for label in &labels {
            SendMessageW(combo, CB_ADDSTRING, 0, ui::wide(label).as_ptr() as isize);
        }
        ui::dropdown_width(
            combo,
            state.theme.body,
            &labels.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        let selected = entries
            .iter()
            .position(|(selection, _)| saved.as_ref() == Some(selection))
            .map_or(0, |i| i + 1);
        SendMessageW(combo, CB_SETCURSEL, selected, 0);
        *state.battery.borrow_mut() = entries.into_iter().map(|(s, _)| s).collect();
        sync_hp_service(hwnd);
        for (id, value) in [
            (AUTOSTART, snapshot.run_entry.is_some()),
            (TRAY, snapshot.options.start_in_tray),
            (HOLD, snapshot.options.hold_preset),
        ] {
            SendMessageW(
                GetDlgItem(hwnd, id as i32),
                BM_SETCHECK,
                if value { BST_CHECKED } else { BST_UNCHECKED } as usize,
                0,
            );
        }
    }
}
/// Ticked while HPOmenCap is disabled; unavailable when it is absent.
unsafe fn sync_hp_service(hwnd: HWND) {
    unsafe {
        let status = omen_platform::hp_control::oem_service();
        let check = GetDlgItem(hwnd, HP_SERVICE as i32);
        let disabled = status.as_ref().is_ok_and(|s| s.is_some_and(|s| s.disabled));
        SendMessageW(
            check,
            BM_SETCHECK,
            if disabled { BST_CHECKED } else { BST_UNCHECKED } as usize,
            0,
        );
        EnableWindow(check, status.is_ok_and(|s| s.is_some()) as i32);
    }
}
/// Stops and disables HPOmenCap, or restores it, on a worker thread; the
/// window reports the result when it arrives.
unsafe fn change_hp_service(hwnd: HWND, state: &State) {
    let disable = unsafe { checked(hwnd, HP_SERVICE) };
    unsafe {
        EnableWindow(GetDlgItem(hwnd, HP_SERVICE as i32), 0);
        ui::text(
            GetDlgItem(hwnd, STATUS),
            t(if disable {
                "hp_service.disabling"
            } else {
                "hp_service.restoring"
            }),
        );
        theme::error(GetDlgItem(hwnd, STATUS), false);
    }
    let result = state.hp_result.clone();
    let target = hwnd as usize;
    std::thread::spawn(move || {
        let outcome =
            match crate::elevation::run(None, crate::elevation::Action::HpService(disable), || {
                false
            }) {
                Ok(_) if disable => (t("hp_service.disabled").to_owned(), false),
                Ok(_) => (t("hp_service.restored").to_owned(), false),
                Err(error) => (
                    tf("hp_service.failed", &[&crate::i18n::error_message(&error)]),
                    true,
                ),
            };
        *result.lock().unwrap_or_else(|e| e.into_inner()) = Some(outcome);
        // SAFETY: PostMessage does not dereference a closed window.
        unsafe {
            PostMessageW(target as HWND, HP_DONE, 0, 0);
        }
    });
}
fn save(hwnd: HWND, state: &State) -> Result<(), String> {
    let baseline = state.baseline.borrow().clone();
    let mut options = baseline.options.clone();
    unsafe {
        options.language = match SendMessageW(GetDlgItem(hwnd, LANGUAGE as i32), CB_GETCURSEL, 0, 0)
        {
            0 => LanguagePreference::System,
            1 => LanguagePreference::German,
            2 => LanguagePreference::English,
            _ => return Err(t("preset.choice_invalid").into()),
        };
        options.start_in_tray = checked(hwnd, TRAY);
        options.hold_preset = checked(hwnd, HOLD);
        let index = SendMessageW(GetDlgItem(hwnd, BATTERY as i32), CB_GETCURSEL, 0, 0);
        options.battery_preset = usize::try_from(index - 1)
            .ok()
            .and_then(|i| state.battery.borrow().get(i).map(Selection::key));
        options.close_to_tray =
            match SendMessageW(GetDlgItem(hwnd, CLOSE as i32), CB_GETCURSEL, 0, 0) {
                0 => true,
                1 => false,
                _ => return Err(t("preset.choice_invalid").into()),
            };
        let snapshot = settings::save(&baseline, &options, checked(hwnd, AUTOSTART))?;
        *state.baseline.borrow_mut() = snapshot;
        let restart =
            crate::i18n::preference_code(options.language) != crate::i18n::language_code();
        ui::text(
            GetDlgItem(hwnd, STATUS),
            t(if restart {
                "settings.saved_restart"
            } else {
                "settings.saved"
            }),
        );
        theme::error(GetDlgItem(hwnd, STATUS), false);
    }
    Ok(())
}
unsafe fn layout(hwnd: HWND) {
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut r);
        if r.right < 200 || r.bottom < 200 {
            return;
        }
        let w = r.right;
        let mut layout = ui::Layout::new(hwnd);
        for (id, x, y, width, height) in [
            (810, 28, 24, w - 56, 36),
            (811, 28, 82, 220, 28),
            (LANGUAGE as i32, 270, 76, w - 298, 260),
            (AUTOSTART as i32, 28, 128, w - 56, 30),
            (TRAY as i32, 28, 168, w - 56, 30),
            (813, 28, 216, 220, 28),
            (CLOSE as i32, 270, 210, w - 298, 220),
            (817, 28, 264, 220, 28),
            (BATTERY as i32, 270, 258, w - 298, 300),
            (HP_SERVICE as i32, 28, 308, w - 56, 30),
            (HOLD as i32, 28, 348, w - 56, 30),
            (STATUS, 28, r.bottom - 128, w - 56, 42),
            (816, 28, r.bottom - 64, w - 330, 28),
            (SAVE as i32, w - 296, r.bottom - 66, 126, 40),
            (CANCEL as i32, w - 154, r.bottom - 66, 126, 40),
        ] {
            layout.place(GetDlgItem(hwnd, id), x, y, width, height);
        }
        // Each info button follows the text of its setting.
        for (info, (control, y)) in INFO.iter().zip([
            (811, 82),
            (AUTOSTART as i32, 131),
            (TRAY as i32, 171),
            (813, 216),
            (817, 264),
            (HP_SERVICE as i32, 311),
            (HOLD as i32, 351),
        ]) {
            let handle = GetDlgItem(hwnd, control);
            let font = SendMessageW(handle, WM_GETFONT, 0, 0) as HFONT;
            let mut label = [0u16; 256];
            let n = GetWindowTextW(handle, label.as_mut_ptr(), 256).max(0) as usize;
            let text = ui::text_width(handle, font, &String::from_utf16_lossy(&label[..n]));
            // Check boxes draw their box before the text.
            let offset = if control == 811 || control == 813 || control == 817 {
                8
            } else {
                30
            };
            layout.place(
                GetDlgItem(hwnd, *info as i32),
                28 + text + offset,
                y,
                24,
                24,
            );
        }
        layout.apply();
    }
}
unsafe fn close(hwnd: HWND) {
    unsafe {
        let owner = GetWindow(hwnd, GW_OWNER);
        if !owner.is_null() {
            EnableWindow(owner, 1);
        }
        DestroyWindow(hwnd);
    }
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
        let Some(state) = raw.as_ref() else {
            return DefWindowProcW(hwnd, m, w, l);
        };
        if let Some(color) = state.theme.color(m, w, l) {
            return color;
        }
        match m {
            WM_CREATE => {
                state.theme.frame(hwnd);
                ui::control(
                    hwnd,
                    state.theme.heading,
                    810,
                    "STATIC",
                    t("settings.title"),
                    0,
                );
                for (id, key) in [
                    (811, "settings.language"),
                    (813, "settings.close_behavior"),
                    (817, "settings.battery"),
                ] {
                    ui::control(hwnd, state.theme.body, id, "STATIC", t(key), 0);
                }
                ui::control(
                    hwnd,
                    state.theme.body,
                    LANGUAGE,
                    "COMBOBOX",
                    "",
                    WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                );
                ui::control(
                    hwnd,
                    state.theme.body,
                    BATTERY,
                    "COMBOBOX",
                    "",
                    WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                );
                for id in INFO {
                    ui::control(
                        hwnd,
                        state.theme.small,
                        id,
                        "BUTTON",
                        "i",
                        WS_TABSTOP | BS_OWNERDRAW as u32,
                    );
                }
                for (id, key) in [
                    (AUTOSTART, "settings.autostart"),
                    (TRAY, "settings.start_in_tray"),
                    (HP_SERVICE, "settings.hp_service"),
                    (HOLD, "settings.hold_preset"),
                ] {
                    ui::control(
                        hwnd,
                        state.theme.body,
                        id,
                        "BUTTON",
                        t(key),
                        WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                    );
                }
                ui::control(
                    hwnd,
                    state.theme.body,
                    CLOSE,
                    "COMBOBOX",
                    "",
                    WS_TABSTOP | CBS_DROPDOWNLIST as u32,
                );
                ui::control(hwnd, state.theme.small, STATUS as usize, "STATIC", "", 0);
                ui::control(
                    hwnd,
                    state.theme.small,
                    816,
                    "STATIC",
                    &tf("settings.version", &[&env!("CARGO_PKG_VERSION")]),
                    0,
                );
                for (id, key) in [(SAVE, "preset.save"), (CANCEL, "settings.close")] {
                    ui::control(hwnd, state.theme.body, id, "BUTTON", t(key), WS_TABSTOP);
                }
                populate(hwnd, state);
                layout(hwnd);
                0
            }
            WM_SIZE => {
                layout(hwnd);
                0
            }
            WM_GETMINMAXINFO => {
                (*(l as *mut MINMAXINFO)).ptMinTrackSize = POINT { x: 740, y: 540 };
                0
            }
            WM_ERASEBKGND | WM_PRINTCLIENT => {
                state.theme.fill(hwnd, w as HDC);
                1
            }
            WM_DRAWITEM => {
                let item = &*(l as *const DRAWITEMSTRUCT);
                if INFO.contains(&(item.CtlID as usize)) {
                    state.theme.info(item);
                } else {
                    state.theme.button(item, state.theme.body, w == SAVE);
                }
                1
            }
            HP_DONE => {
                if let Some((message, error)) = state
                    .hp_result
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take()
                {
                    ui::text(GetDlgItem(hwnd, STATUS), &message);
                    theme::error(GetDlgItem(hwnd, STATUS), error);
                }
                sync_hp_service(hwnd);
                0
            }
            WM_COMMAND => {
                if (w >> 16) as u32 != BN_CLICKED {
                    return 0;
                }
                let id = w & 0xffff;
                if let Some(index) = INFO.iter().position(|info| *info == id) {
                    let body = INFO_TEXT[index]
                        .iter()
                        .map(|key| t(key))
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    MessageBoxW(
                        hwnd,
                        ui::wide(&body).as_ptr(),
                        ui::wide(t("settings.title")).as_ptr(),
                        MB_OK | MB_ICONINFORMATION,
                    );
                    return 0;
                }
                match id {
                    HP_SERVICE => change_hp_service(hwnd, state),
                    SAVE => {
                        if let Err(error) = save(hwnd, state) {
                            ui::text(
                                GetDlgItem(hwnd, STATUS),
                                &crate::i18n::error_message(&error),
                            );
                            theme::error(GetDlgItem(hwnd, STATUS), true);
                        }
                    }
                    CANCEL => {
                        close(hwnd);
                    }
                    _ => {}
                }
                0
            }
            WM_CLOSE => {
                close(hwnd);
                0
            }
            _ => DefWindowProcW(hwnd, m, w, l),
        }
    }
}
pub fn run(owner: HWND) -> Result<(), String> {
    let state = Box::new(State {
        theme: theme::Theme::new(),
        baseline: RefCell::new(settings::load()?),
        battery: RefCell::new(vec![]),
        hp_result: Arc::new(Mutex::new(None)),
    });
    unsafe {
        ui::register("OmenHubPPSettings", Some(procedure))?;
        let hwnd = ui::create(
            "OmenHubPPSettings",
            t("settings.window_title"),
            760,
            560,
            (&*state as *const State).cast_mut().cast(),
            false,
        )?;
        if !owner.is_null() {
            SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, owner as isize);
            EnableWindow(owner, 0);
        }
        ShowWindow(hwnd, SW_SHOW);
        let mut message: MSG = std::mem::zeroed();
        while IsWindow(hwnd) != 0 {
            let result = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam as i32);
                }
                if IsWindow(hwnd) != 0 {
                    DestroyWindow(hwnd);
                }
                break;
            }
            if IsDialogMessageW(hwnd, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if !owner.is_null() && IsWindow(owner) != 0 {
            EnableWindow(owner, 1);
            SetForegroundWindow(owner);
        }
    }
    Ok(())
}
