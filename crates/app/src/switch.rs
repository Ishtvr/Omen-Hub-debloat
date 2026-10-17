//! Owner-painted position switch for the dashboard (`●─○─○` with the option
//! names underneath). Clicking a stop or using the arrow keys selects it and
//! sends `WM_COMMAND` / `BN_CLICKED` to the parent. Shapes are drawn at four
//! times the size and scaled down so the circles stay smooth.
use crate::{
    native_ui::wide,
    segments::{self, Key},
    theme::{self, Theme},
};
use std::{cell::Cell, ptr};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::WM_MOUSELEAVE,
        Input::KeyboardAndMouse::{
            SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_DOWN, VK_END, VK_HOME,
            VK_LEFT, VK_RIGHT, VK_UP,
        },
        WindowsAndMessaging::*,
    },
};

const CLASS: &str = "OmenHubPPSwitch";
/// Sets the selection without notifying the parent (wparam: index).
pub const SET_SELECTION: u32 = WM_USER + 1;
/// Returns the selected index.
pub const GET_SELECTION: u32 = WM_USER + 2;
const SCALE: i32 = 4;

struct Colors {
    background: COLORREF,
    track: COLORREF,
    border: COLORREF,
    accent: COLORREF,
    text: COLORREF,
    muted: COLORREF,
}
struct Switch {
    caption: String,
    options: Vec<String>,
    selected: Cell<usize>,
    hover: Cell<Option<usize>>,
    font: HFONT,
    colors: Colors,
}

/// Creates a switch child. `caption` names it for assistive technology.
pub unsafe fn create(
    parent: HWND,
    id: i32,
    caption: &str,
    options: &[&str],
    theme: &Theme,
) -> HWND {
    unsafe {
        let class = wide(CLASS);
        let mut registration: WNDCLASSW = std::mem::zeroed();
        registration.lpfnWndProc = Some(procedure);
        registration.hInstance = GetModuleHandleW(ptr::null());
        registration.lpszClassName = class.as_ptr();
        registration.hCursor = LoadCursorW(ptr::null_mut(), IDC_HAND);
        // Already registered by an earlier dashboard build: nothing to do.
        RegisterClassW(&registration);
        let colors = if theme.high_contrast {
            Colors {
                background: theme.bg(),
                track: theme.bg(),
                border: theme.text(),
                accent: GetSysColor(COLOR_HIGHLIGHT),
                text: theme.text(),
                muted: theme.text(),
            }
        } else {
            Colors {
                background: theme::BACKGROUND,
                track: theme::FIELD,
                border: theme::BORDER,
                accent: theme::ACCENT,
                text: theme::FOREGROUND,
                muted: theme::MUTED,
            }
        };
        let state = Box::new(Switch {
            caption: caption.into(),
            options: options.iter().map(|o| (*o).to_owned()).collect(),
            selected: Cell::new(0),
            hover: Cell::new(None),
            font: theme.small,
            colors,
        });
        let handle = CreateWindowExW(
            0,
            class.as_ptr(),
            wide(caption).as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            0,
            0,
            10,
            10,
            parent,
            id as HMENU,
            GetModuleHandleW(ptr::null()),
            Box::into_raw(state).cast(),
        );
        name(handle);
        handle
    }
}

pub unsafe fn select(handle: HWND, index: usize) {
    unsafe {
        SendMessageW(handle, SET_SELECTION, index, 0);
    }
}
pub unsafe fn selection(handle: HWND) -> usize {
    unsafe { SendMessageW(handle, GET_SELECTION, 0, 0) as usize }
}

/// Window text "Caption: Option" for screen readers.
unsafe fn name(handle: HWND) {
    unsafe {
        if let Some(state) = (GetWindowLongPtrW(handle, GWLP_USERDATA) as *const Switch).as_ref() {
            let option = state
                .options
                .get(state.selected.get())
                .map_or("", String::as_str);
            SetWindowTextW(
                handle,
                wide(&format!("{}: {option}", state.caption)).as_ptr(),
            );
        }
    }
}

/// Posted, not sent: the parent reads the selection after this window
/// procedure has returned.
unsafe fn choose(handle: HWND, state: &Switch, index: usize) {
    unsafe {
        if index == state.selected.get() || index >= state.options.len() {
            return;
        }
        state.selected.set(index);
        name(handle);
        InvalidateRect(handle, ptr::null(), 0);
        let id = GetDlgCtrlID(handle) as usize;
        PostMessageW(
            GetParent(handle),
            WM_COMMAND,
            (BN_CLICKED as usize) << 16 | id,
            handle as isize,
        );
    }
}

unsafe fn circle(dc: HDC, x: i32, y: i32, diameter: i32, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        let pen = CreatePen(PS_SOLID, 1, color);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        let r = diameter / 2;
        Ellipse(dc, x - r, y - r, x - r + diameter, y - r + diameter);
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(brush);
        DeleteObject(pen);
    }
}

unsafe fn paint(handle: HWND, state: &Switch, target: HDC) {
    unsafe {
        let mut client: RECT = std::mem::zeroed();
        GetClientRect(handle, &mut client);
        let (width, height) = (client.right.max(1), client.bottom.max(1));
        let enabled = windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(handle) != 0;
        let focused = windows_sys::Win32::UI::Input::KeyboardAndMouse::GetFocus() == handle;
        let c = &state.colors;
        let count = state.options.len();
        // Shapes at SCALE x, then halftone-scaled into the 1x buffer.
        let big = CreateCompatibleDC(target);
        let big_bitmap = CreateCompatibleBitmap(target, width * SCALE, segments::TRACK * SCALE);
        let old_big = SelectObject(big, big_bitmap);
        let background = CreateSolidBrush(c.background);
        let all = RECT {
            left: 0,
            top: 0,
            right: width * SCALE,
            bottom: segments::TRACK * SCALE,
        };
        FillRect(big, &all, background);
        let (left, right) = segments::track(width, count);
        let pen = CreatePen(
            PS_SOLID,
            if focused { 2 * SCALE } else { SCALE },
            if focused && enabled {
                c.accent
            } else {
                c.border
            },
        );
        let fill = CreateSolidBrush(c.track);
        let old_pen = SelectObject(big, pen);
        let old_brush = SelectObject(big, fill);
        let inset = SCALE / 2;
        RoundRect(
            big,
            left * SCALE + inset,
            inset,
            right * SCALE - inset,
            segments::TRACK * SCALE - inset,
            segments::TRACK * SCALE,
            segments::TRACK * SCALE,
        );
        SelectObject(big, old_pen);
        SelectObject(big, old_brush);
        DeleteObject(pen);
        DeleteObject(fill);
        let stops = segments::stops(width, count);
        let middle = segments::TRACK * SCALE / 2;
        // The connecting line gives the `○─○─○` rail between the stops.
        if let (Some(first), Some(last)) = (stops.first(), stops.last()) {
            let rail = CreatePen(PS_SOLID, 2 * SCALE, c.border);
            let old = SelectObject(big, rail);
            MoveToEx(big, first * SCALE, middle, ptr::null_mut());
            LineTo(big, last * SCALE, middle);
            SelectObject(big, old);
            DeleteObject(rail);
        }
        for (index, x) in stops.iter().enumerate() {
            if index == state.selected.get() {
                continue;
            }
            let hot = state.hover.get() == Some(index) && enabled;
            circle(
                big,
                x * SCALE,
                middle,
                if hot { 12 } else { 9 } * SCALE,
                if hot { c.text } else { c.muted },
            );
        }
        if let Some(x) = stops.get(state.selected.get()) {
            let knob = (segments::TRACK - 6) * SCALE;
            circle(
                big,
                x * SCALE,
                middle,
                knob,
                if enabled { c.accent } else { c.muted },
            );
        }
        // Compose the 1x frame off screen to avoid flicker.
        let frame = CreateCompatibleDC(target);
        let frame_bitmap = CreateCompatibleBitmap(target, width, height);
        let old_frame = SelectObject(frame, frame_bitmap);
        FillRect(frame, &client, background);
        SetStretchBltMode(frame, HALFTONE);
        SetBrushOrgEx(frame, 0, 0, ptr::null_mut());
        StretchBlt(
            frame,
            0,
            0,
            width,
            segments::TRACK,
            big,
            0,
            0,
            width * SCALE,
            segments::TRACK * SCALE,
            SRCCOPY,
        );
        let old_font = SelectObject(frame, state.font);
        SetBkMode(frame, TRANSPARENT as i32);
        let column = width / count.max(1) as i32;
        for (index, label) in state.options.iter().enumerate() {
            let chosen = index == state.selected.get();
            SetTextColor(frame, if chosen && enabled { c.text } else { c.muted });
            let mut r = RECT {
                left: index as i32 * column,
                top: segments::TRACK + segments::LABEL_GAP,
                right: (index as i32 + 1) * column,
                bottom: height,
            };
            DrawTextW(
                frame,
                wide(label).as_ptr(),
                -1,
                &mut r,
                DT_CENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
            );
        }
        SelectObject(frame, old_font);
        BitBlt(target, 0, 0, width, height, frame, 0, 0, SRCCOPY);
        SelectObject(frame, old_frame);
        DeleteObject(frame_bitmap);
        DeleteDC(frame);
        SelectObject(big, old_big);
        DeleteObject(big_bitmap);
        DeleteDC(big);
        DeleteObject(background);
    }
}

unsafe extern "system" fn procedure(
    handle: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam as *const CREATESTRUCTW);
            SetWindowLongPtrW(handle, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(handle, GWLP_USERDATA) as *mut Switch;
        if pointer.is_null() {
            return DefWindowProcW(handle, message, wparam, lparam);
        }
        // Messages arrive on the UI thread only; the Box lives until WM_NCDESTROY.
        // Shared references plus Cell stay sound when Windows re-enters this
        // procedure (focus changes, painting) during a call.
        let state = &*pointer;
        let width = || {
            let mut r: RECT = std::mem::zeroed();
            GetClientRect(handle, &mut r);
            r.right
        };
        match message {
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(handle, &mut ps);
                paint(handle, state, dc);
                EndPaint(handle, &ps);
                0
            }
            WM_PRINTCLIENT => {
                paint(handle, state, wparam as HDC);
                0
            }
            WM_ERASEBKGND => 1,
            SET_SELECTION => {
                if wparam < state.options.len() && wparam != state.selected.get() {
                    state.selected.set(wparam);
                    name(handle);
                    InvalidateRect(handle, ptr::null(), 0);
                }
                0
            }
            GET_SELECTION => state.selected.get() as isize,
            WM_LBUTTONDOWN => {
                SetFocus(handle);
                let x = (lparam & 0xffff) as i16 as i32;
                choose(
                    handle,
                    state,
                    segments::hit(x, width(), state.options.len()),
                );
                0
            }
            WM_MOUSEMOVE => {
                let x = (lparam & 0xffff) as i16 as i32;
                let hover = Some(segments::hit(x, width(), state.options.len()));
                if state.hover.get() != hover {
                    if state.hover.get().is_none() {
                        let mut track = TRACKMOUSEEVENT {
                            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: handle,
                            dwHoverTime: 0,
                        };
                        TrackMouseEvent(&mut track);
                    }
                    state.hover.set(hover);
                    InvalidateRect(handle, ptr::null(), 0);
                }
                0
            }
            WM_MOUSELEAVE => {
                state.hover.set(None);
                InvalidateRect(handle, ptr::null(), 0);
                0
            }
            WM_GETDLGCODE => DLGC_WANTARROWS as isize,
            WM_KEYDOWN => {
                let key = match wparam as u16 {
                    VK_LEFT | VK_UP => Some(Key::Previous),
                    VK_RIGHT | VK_DOWN => Some(Key::Next),
                    VK_HOME => Some(Key::First),
                    VK_END => Some(Key::Last),
                    _ => None,
                };
                if let Some(key) = key {
                    let next = segments::navigate(state.selected.get(), state.options.len(), key);
                    choose(handle, state, next);
                    return 0;
                }
                DefWindowProcW(handle, message, wparam, lparam)
            }
            WM_SETFOCUS | WM_KILLFOCUS | WM_ENABLE | WM_SIZE => {
                InvalidateRect(handle, ptr::null(), 0);
                DefWindowProcW(handle, message, wparam, lparam)
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(handle, GWLP_USERDATA, 0);
                drop(Box::from_raw(pointer));
                DefWindowProcW(handle, message, wparam, lparam)
            }
            _ => DefWindowProcW(handle, message, wparam, lparam),
        }
    }
}
