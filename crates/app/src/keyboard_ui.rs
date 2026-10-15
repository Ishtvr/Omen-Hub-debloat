//! Direct four-zone lighting. One elevated editor; the dashboard stays normal.
use crate::{
    i18n::t,
    native_ui::{self as ui, text, wide},
    theme,
};
use omen_core::{
    features::Feature,
    keyboard::KeyboardState,
    profiles::{PerformanceProfile, Value},
};
use omen_platform::{DirectLighting, profile_store::ProfileStore};
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
    System::SystemServices::SS_CENTER,
    UI::{
        Controls::Dialogs::*,
        Controls::{DRAWITEMSTRUCT, ODS_FOCUS, ODS_SELECTED},
        Input::KeyboardAndMouse::{EnableWindow, GetFocus},
        Shell::{IsUserAnAdmin, ShellExecuteW},
        WindowsAndMessaging::*,
    },
};

const UPDATE: u32 = WM_APP + 21;
const POWER: usize = 600;
const ZONE: usize = 610;
const STATUS: i32 = 624;
const CLASS: &str = "OmenHubPPKeyboardEditor";
/// Language keys of the four keyboard zones, left to right.
const LABELS: [&str; 4] = [
    "lighting.zone_left",
    "lighting.zone_middle",
    "lighting.zone_right",
    "lighting.zone_numpad",
];
enum Request {
    Read,
    Apply {
        expected: KeyboardState,
        colors: [[u8; 3]; 4],
        enabled: bool,
    },
    Stop,
}
struct Update {
    state: Option<KeyboardState>,
    error: Option<String>,
}
struct State {
    theme: theme::Theme,
    tx: mpsc::Sender<Request>,
    rx: mpsc::Receiver<Update>,
    live: RefCell<Option<KeyboardState>>,
    colors: Cell<[[u8; 3]; 4]>,
    enabled: Cell<bool>,
    busy: Cell<bool>,
    closing: Cell<bool>,
    zones: Cell<[HWND; 4]>,
    bounds: Cell<[RECT; 4]>,
    power: Cell<HWND>,
    preview: bool,
    custom_colors: RefCell<[u32; 16]>,
}
fn worker(hwnd: usize, requests: mpsc::Receiver<Request>, updates: mpsc::Sender<Update>) {
    let mut control = DirectLighting::new();
    while let Ok(request) = requests.recv() {
        if matches!(request, Request::Stop) {
            break;
        }
        let result = match control.as_mut() {
            Ok(control) => match request {
                Request::Read => control.read(),
                Request::Apply {
                    expected,
                    colors,
                    enabled,
                } => control.apply(&expected, &colors, enabled),
                Request::Stop => unreachable!(),
            },
            Err(error) => Err(error.clone()),
        };
        let update = match result {
            Ok(state) => Update {
                state: Some(state),
                error: None,
            },
            Err(error) => {
                let state = control.as_mut().ok().and_then(|c| c.read().ok());
                if let Ok(user) = ProfileStore::user_data() {
                    if let Ok(store) = ProfileStore::at(user.root().join("keyboard-control")) {
                        let _=store.save_control_result(&serde_json::json!({"schema":1,"operation":"lighting_direct","detail":error}));
                    }
                }
                Update {
                    state,
                    error: Some(error),
                }
            }
        };
        if updates.send(update).is_err() {
            break;
        }
        unsafe {
            PostMessageW(hwnd as HWND, UPDATE, 0, 0);
        }
    }
}
fn status(state: &State, message: &str, failed: bool) {
    unsafe {
        let hwnd = GetParent(state.power.get());
        let handle = GetDlgItem(hwnd, STATUS);
        text(handle, message);
        theme::error(handle, failed);
    }
}
fn refresh(state: &State) {
    unsafe {
        let ready = state.live.borrow().is_some() || state.preview;
        EnableWindow(state.power.get(), (!state.busy.get()) as i32);
        text(
            state.power.get(),
            if state.busy.get() {
                t("lighting.applying")
            } else if !ready {
                t("lighting.reconnect")
            } else if state.enabled.get() {
                t("common.on")
            } else {
                t("common.off")
            },
        );
        for handle in state.zones.get() {
            EnableWindow(
                handle,
                (ready && !state.busy.get() && state.enabled.get()) as i32,
            );
            InvalidateRect(handle, ptr::null(), 0);
        }
        InvalidateRect(state.power.get(), ptr::null(), 0);
    }
}
fn dispatch(state: &State, request: Request) {
    if state.busy.replace(true) {
        return;
    }
    status(state, t("lighting.applying"), false);
    refresh(state);
    if state.tx.send(request).is_err() {
        state.busy.set(false);
        status(state, t("preset.connection_lost"), true);
        refresh(state);
    }
}
fn apply(state: &State, colors: [[u8; 3]; 4], enabled: bool) {
    if state.preview {
        state.colors.set(colors);
        state.enabled.set(enabled);
        refresh(state);
        return;
    }
    let expected = state.live.borrow().clone();
    if let Some(expected) = expected {
        dispatch(
            state,
            Request::Apply {
                expected,
                colors,
                enabled,
            },
        );
    }
}
fn pick(state: &State, zone: usize) {
    if state.busy.get() || !state.enabled.get() {
        return;
    }
    let mut colors = state.colors.get();
    let [r, g, b] = colors[zone];
    let mut custom = state.custom_colors.borrow_mut();
    let mut choose: CHOOSECOLORW = unsafe { std::mem::zeroed() };
    choose.lStructSize = size_of::<CHOOSECOLORW>() as u32;
    choose.hwndOwner = unsafe { GetParent(state.power.get()) };
    choose.rgbResult = theme::rgb(r, g, b);
    choose.lpCustColors = custom.as_mut_ptr();
    choose.Flags = CC_RGBINIT | CC_FULLOPEN;
    if unsafe { ChooseColorW(&mut choose) } != 0 {
        colors[zone] = [
            (choose.rgbResult & 255) as u8,
            ((choose.rgbResult >> 8) & 255) as u8,
            ((choose.rgbResult >> 16) & 255) as u8,
        ];
        drop(custom);
        apply(state, colors, true);
    }
}
unsafe fn layout(hwnd: HWND, state: &State) {
    unsafe {
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut client);
        let w = client.right;
        let h = client.bottom;
        ui::place(GetDlgItem(hwnd, 620), 32, 26, w - 270, 40);
        ui::place(state.power.get(), w - 180, 25, 148, 42);
        let keyboard_w = w - 64;
        let keyboard_h = (keyboard_w * 31 / 100).clamp(200, 330);
        let top = 104 + ((h - 180 - keyboard_h).max(0) / 2);
        let mut bounds = [RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        }; 4];
        for (i, bound) in bounds.iter_mut().enumerate() {
            let left = 32 + keyboard_w * i as i32 / 4;
            let right = 32 + keyboard_w * (i + 1) as i32 / 4;
            *bound = RECT {
                left,
                top,
                right,
                bottom: top + keyboard_h,
            };
            ui::place(state.zones.get()[i], left, top, right - left, keyboard_h);
            ui::place(
                GetDlgItem(hwnd, 630 + i as i32),
                left,
                top + keyboard_h + 12,
                right - left,
                24,
            );
        }
        state.bounds.set(bounds);
        ui::place(GetDlgItem(hwnd, STATUS), 32, h - 49, w - 64, 28);
    }
}
unsafe fn key(dc: HDC, rect: RECT, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        let pen = CreatePen(PS_SOLID, 1, theme::BORDER);
        let old_b = SelectObject(dc, brush);
        let old_p = SelectObject(dc, pen);
        RoundRect(dc, rect.left, rect.top, rect.right, rect.bottom, 5, 5);
        SelectObject(dc, old_b);
        SelectObject(dc, old_p);
        DeleteObject(brush);
        DeleteObject(pen);
    }
}
unsafe fn draw_zone(state: &State, item: &DRAWITEMSTRUCT) {
    unsafe {
        let zone = (item.CtlID as usize) - ZONE;
        let bounds = state.bounds.get();
        let full_w = bounds[3].right - bounds[0].left;
        let h = item.rcItem.bottom;
        let local_left = bounds[zone].left - bounds[0].left;
        ui::fill(item.hDC, &item.rcItem, state.theme.panel_color());
        let save = SaveDC(item.hDC);
        let mut origin = POINT { x: 0, y: 0 };
        GetViewportOrgEx(item.hDC, &mut origin);
        SetViewportOrgEx(item.hDC, origin.x - local_left, origin.y, ptr::null_mut());
        let unit = (full_w - 32) as f32 / 20.0;
        let row_h = (h - 30) as f32 / 6.0;
        let color = |center: f32| {
            if !state.enabled.get() || (state.live.borrow().is_none() && !state.preview) {
                return theme::rgb(52, 58, 65);
            }
            let index = ((center / 5.0) as usize).min(3);
            let [r, g, b] = state.colors.get()[index];
            theme::rgb(r, g, b)
        };
        for row in 0..6 {
            let mut columns: Vec<(f32, f32)> = if row == 5 {
                vec![
                    (0., 1.2),
                    (1.2, 1.2),
                    (2.4, 1.2),
                    (3.6, 5.2),
                    (8.8, 1.2),
                    (10., 1.2),
                    (11.2, 1.2),
                    (12.4, 1.2),
                    (13.6, 1.4),
                ]
            } else if row == 2 {
                std::iter::once((0., 1.5))
                    .chain((0..12).map(|i| (1.5 + i as f32, 1.)))
                    .chain(std::iter::once((13.5, 1.5)))
                    .collect()
            } else if row == 3 {
                std::iter::once((0., 1.8))
                    .chain((0..11).map(|i| (1.8 + i as f32, 1.)))
                    .chain(std::iter::once((12.8, 2.2)))
                    .collect()
            } else if row == 4 {
                std::iter::once((0., 2.2))
                    .chain((0..10).map(|i| (2.2 + i as f32, 1.)))
                    .chain(std::iter::once((12.2, 2.8)))
                    .collect()
            } else {
                (0..15).map(|i| (i as f32, 1.)).collect()
            };
            columns.extend((0..4).map(|i| (16. + i as f32, 1.)));
            for (x, width) in columns {
                let rect = RECT {
                    left: 16 + (x * unit) as i32 + 2,
                    top: 15 + (row as f32 * row_h) as i32 + 2,
                    right: 16 + ((x + width) * unit) as i32 - 3,
                    bottom: 15 + ((row + 1) as f32 * row_h) as i32 - 3,
                };
                key(item.hDC, rect, color(x + width / 2.));
            }
        }
        RestoreDC(item.hDC, save);
        if item.itemState & (ODS_FOCUS | ODS_SELECTED) != 0 || GetFocus() == item.hwndItem {
            let pen = CreatePen(PS_SOLID, 2, theme::ACCENT);
            let old_p = SelectObject(item.hDC, pen);
            let old_b = SelectObject(item.hDC, GetStockObject(NULL_BRUSH));
            Rectangle(
                item.hDC,
                2,
                2,
                item.rcItem.right - 2,
                item.rcItem.bottom - 2,
            );
            SelectObject(item.hDC, old_p);
            SelectObject(item.hDC, old_b);
            DeleteObject(pen);
        }
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let raw = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const State;
        if raw.is_null() {
            return DefWindowProcW(hwnd, message, w, l);
        }
        let state = &*raw;
        if let Some(color) = state.theme.color(message, w, l) {
            return color;
        }
        match message {
            WM_CREATE => {
                state.theme.frame(hwnd);
                ui::control(
                    hwnd,
                    state.theme.title,
                    620,
                    "STATIC",
                    t("dash.lighting"),
                    0,
                );
                state.power.set(ui::control(
                    hwnd,
                    state.theme.body,
                    POWER,
                    "BUTTON",
                    t("curve.reading_short"),
                    WS_TABSTOP,
                ));
                let zones = std::array::from_fn(|i| {
                    ui::control(
                        hwnd,
                        state.theme.body,
                        ZONE + i,
                        "BUTTON",
                        t(LABELS[i]),
                        WS_TABSTOP | BS_OWNERDRAW as u32,
                    )
                });
                state.zones.set(zones);
                for (i, label) in LABELS.iter().enumerate() {
                    let c = ui::control(
                        hwnd,
                        state.theme.small,
                        630 + i,
                        "STATIC",
                        t(label),
                        SS_CENTER,
                    );
                    theme::muted(c);
                }
                ui::control(
                    hwnd,
                    state.theme.small,
                    STATUS as usize,
                    "STATIC",
                    t("lighting.reading"),
                    0,
                );
                layout(hwnd, state);
                refresh(state);
                0
            }
            WM_SIZE => {
                layout(hwnd, state);
                0
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(l as *mut MINMAXINFO);
                info.ptMinTrackSize = POINT { x: 760, y: 480 };
                0
            }
            WM_ERASEBKGND | WM_PRINTCLIENT => {
                let mut rect = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                ui::fill(w as HDC, &rect, state.theme.bg());
                1
            }
            WM_DRAWITEM => {
                let item = &*(l as *const DRAWITEMSTRUCT);
                if (ZONE..ZONE + 4).contains(&(item.CtlID as usize)) {
                    draw_zone(state, item);
                } else {
                    state.theme.button(
                        item,
                        state.theme.body,
                        state.enabled.get() && item.CtlID == POWER as u32,
                    );
                }
                1
            }
            WM_COMMAND => {
                let id = w & 0xffff;
                let notification = (w >> 16) & 0xffff;
                if notification != BN_CLICKED as usize || state.busy.get() {
                    return 0;
                }
                if id == POWER {
                    if state.live.borrow().is_none() && !state.preview {
                        dispatch(state, Request::Read);
                    } else {
                        apply(state, state.colors.get(), !state.enabled.get());
                    }
                } else if (ZONE..ZONE + 4).contains(&id) {
                    pick(state, id - ZONE);
                }
                0
            }
            UPDATE => {
                while let Ok(update) = state.rx.try_recv() {
                    state.busy.set(false);
                    if let Some(live) = update.state {
                        if let Ok(colors) = live
                            .colors()
                            .and_then(|v| v.try_into().map_err(|_| t("lighting.four_zones").into()))
                        {
                            state.colors.set(colors);
                        }
                        state
                            .enabled
                            .set(live.custom_colors_enabled().unwrap_or(false));
                        *state.live.borrow_mut() = Some(live);
                    } else {
                        *state.live.borrow_mut() = None;
                    }
                    if let Some(error) = update.error {
                        status(
                            state,
                            if error.contains("outside this window") {
                                t("lighting.changed_elsewhere")
                            } else {
                                t("lighting.failed")
                            },
                            true,
                        );
                    } else {
                        status(
                            state,
                            if state.enabled.get() {
                                t("lighting.hint")
                            } else {
                                t("lighting.off")
                            },
                            false,
                        );
                    }
                    refresh(state);
                }
                if state.closing.get() && !state.busy.get() {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_CLOSE => {
                if state.busy.get() {
                    state.closing.set(true);
                    status(state, t("lighting.finishing"), false);
                } else {
                    DestroyWindow(hwnd);
                }
                0
            }
            WM_DESTROY => {
                let _ = state.tx.send(Request::Stop);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, w, l),
        }
    }
}
pub fn run(selected: Option<String>) -> Result<(), String> {
    if selected.is_some() {
        return Err(t("lighting.no_profile").into());
    }
    if unsafe { IsUserAnAdmin() } == 0 {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                wide("runas").as_ptr(),
                wide(&exe.to_string_lossy()).as_ptr(),
                wide("keyboard-ui").as_ptr(),
                ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        if result as usize <= 32 {
            return Err(t("lighting.uac_cancelled").into());
        }
        return Ok(());
    }
    run_window(None, None, None)
}
pub fn preview(selected: String, path: &Path, size: Option<[i32; 2]>) -> Result<(), String> {
    run_window(
        Some(ProfileStore::user_data()?.load(&selected)?),
        Some(path),
        size,
    )
}
fn run_window(
    preview_profile: Option<PerformanceProfile>,
    preview: Option<&Path>,
    size: Option<[i32; 2]>,
) -> Result<(), String> {
    let [width, height] = size.unwrap_or([1000, 590]);
    if !(760..=2560).contains(&width) || !(480..=1600).contains(&height) {
        return Err(t("error.preview_size").into());
    }
    let (tx, requests) = mpsc::channel();
    let (updates, rx) = mpsc::channel();
    let colors = preview_profile
        .as_ref()
        .and_then(|p| {
            p.settings.iter().find_map(|s| {
                if let Value::Colors(v) = &s.value {
                    Some(v.clone())
                } else {
                    None
                }
            })
        })
        .and_then(|v| v.try_into().ok())
        .unwrap_or([[185, 213, 248]; 4]);
    let enabled = preview_profile
        .as_ref()
        .and_then(|p| {
            p.settings
                .iter()
                .find(|s| s.feature == Feature::KeyboardEffect)
        })
        .is_none_or(|s| s.value == Value::Choice(omen_core::profiles::Choice::Static));
    let state = Box::new(State {
        theme: theme::Theme::new(),
        tx,
        rx,
        live: RefCell::new(None),
        colors: Cell::new(colors),
        enabled: Cell::new(enabled),
        busy: Cell::new(false),
        closing: Cell::new(false),
        zones: Cell::new([ptr::null_mut(); 4]),
        bounds: Cell::new(
            [RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            }; 4],
        ),
        power: Cell::new(ptr::null_mut()),
        preview: preview.is_some(),
        custom_colors: RefCell::new([0; 16]),
    });
    unsafe {
        ui::register(CLASS, Some(procedure))?;
        let hwnd = ui::create(
            CLASS,
            t("lighting.window_title"),
            width,
            height,
            (&*state as *const State).cast_mut().cast(),
            preview.is_some(),
        )?;
        ShowWindow(hwnd, SW_SHOWNORMAL);
        if let Some(path) = preview {
            status(
                &state,
                if state.enabled.get() {
                    t("lighting.hint")
                } else {
                    t("lighting.off")
                },
                false,
            );
            let result = crate::curve_widget::render_window(hwnd, path);
            DestroyWindow(hwnd);
            return result;
        }
        let handle = hwnd as usize;
        let thread = thread::spawn(move || worker(handle, requests, updates));
        dispatch(&state, Request::Read);
        ui::messages(hwnd);
        let _ = state.tx.send(Request::Stop);
        if thread.is_finished() {
            let _ = thread.join();
        }
    }
    Ok(())
}
