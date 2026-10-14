//! Reusable native GDI curve canvas. Every edit changes a draft only.
use omen_core::{
    curve_editor::{Curve, DragMode, Kind, Point as DataPoint},
    curve_input::{self, Key, Modifiers},
    curve_view::{Viewport, tick_step},
};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    ptr,
    rc::Rc,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::WM_MOUSELEAVE,
        Input::KeyboardAndMouse::{
            GetFocus, GetKeyState, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE,
            TRACKMOUSEEVENT, TrackMouseEvent,
        },
        WindowsAndMessaging::*,
    },
};

pub const CHANGED: u32 = WM_APP + 31;
pub const EDIT_VALUE: u32 = WM_APP + 33;
pub const ZOOM_IN: u32 = WM_APP + 34;
pub const ZOOM_OUT: u32 = WM_APP + 35;
pub const FIT: u32 = WM_APP + 36;
pub const UNDO: u32 = WM_APP + 37;
pub const REDO: u32 = WM_APP + 38;
enum Drag {
    Point {
        start: Vec<DataPoint>,
        down: DataPoint,
        geometry: Geometry,
        mode: DragMode,
    },
    Band {
        x: i32,
        geometry: Geometry,
    },
}
struct State {
    model: RefCell<Option<Curve>>,
    font: HFONT,
    drag: RefCell<Option<Drag>>,
    viewport: Cell<Option<Viewport>>,
    modifiers: Cell<Modifiers>,
    hover: Cell<Option<usize>>,
    error: RefCell<String>,
    ownership: Rc<Cell<u8>>,
}
impl Drop for State {
    fn drop(&mut self) {
        if !self.font.is_null() {
            unsafe {
                DeleteObject(self.font);
            }
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
unsafe fn state<'a>(hwnd: HWND) -> Option<&'a State> {
    // SAFETY: Set only to our Box<State>, removed before WM_NCDESTROY releases it.
    let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const State;
    unsafe { p.as_ref() }
}
pub fn curve(hwnd: HWND) -> Option<Curve> {
    unsafe { state(hwnd) }?.model.borrow().clone()
}
pub fn error(hwnd: HWND) -> String {
    unsafe { state(hwnd) }.map_or(String::new(), |s| s.error.borrow().clone())
}
pub fn set(hwnd: HWND, model: Option<Curve>) {
    if let Some(s) = unsafe { state(hwnd) } {
        s.viewport.set(model.as_ref().map(Viewport::fit));
        *s.drag.borrow_mut() = None;
        s.hover.set(None);
        *s.model.borrow_mut() = model;
        refresh(hwnd);
    }
}
pub fn mutate(
    hwnd: HWND,
    action: impl FnOnce(&mut Curve) -> Result<bool, String>,
) -> Result<bool, String> {
    let s = unsafe { state(hwnd) }.ok_or("Kurvenansicht fehlt.")?;
    let result = {
        let mut model = s.model.borrow_mut();
        action(model.as_mut().ok_or("Kurve noch nicht verfügbar.")?)
    };
    if result.is_ok() {
        refresh(hwnd);
    }
    result
}
fn refresh(hwnd: HWND) {
    let caption = curve(hwnd).map_or("Kurve nicht verfügbar".into(), |c| {
        let p = c.points[c.selected];
        let (x, y) = c.kind.units();
        let view=unsafe {state(hwnd)}.and_then(|s|s.viewport.get()).map_or(String::new(),|v|format!(" Ansicht: {}–{} {}, {}–{} {}.",v.lo.x,v.hi.x,x,v.lo.y,v.hi.y,y));
        let selection = c.selection().map_or(String::new(), |(a,b)| format!(" Auswahl: {}–{} {} ({} Punkte).",c.points[a].x,c.points[b].x,x,b-a+1));
        format!(
            "{}: Punkt {} von {}, {} {}, {} {}.{}{} Shift + Ziehen verschiebt Kurve/Auswahl. Strg + Z: Rückgängig.",
            c.kind.title(),
            c.selected + 1,
            c.points.len(),
            p.x,
            x,
            p.y,
            y,selection,view
        )
    });
    // SAFETY: Our native control copies the bounded terminated caption.
    unsafe {
        SetWindowTextW(hwnd, wide(&caption).as_ptr());
        InvalidateRect(hwnd, ptr::null(), 0);
    }
}
fn notify(hwnd: HWND, changed: usize) {
    // Post instead of Send: parent reads the model after mutable borrows end.
    unsafe {
        PostMessageW(GetParent(hwnd), CHANGED, changed, hwnd as isize);
    }
}
#[derive(Clone, Copy)]
struct Geometry {
    area: RECT,
    lo: DataPoint,
    hi: DataPoint,
}
impl Geometry {
    fn new(rect: RECT, view: Viewport) -> Self {
        let Viewport { lo, hi } = view;
        Self {
            area: RECT {
                left: rect.left + 72,
                top: rect.top + 38,
                right: rect.right - 28,
                bottom: rect.bottom - 54,
            },
            lo,
            hi,
        }
    }
    fn screen(self, p: DataPoint) -> POINT {
        POINT {
            x: self.area.left
                + ((i64::from(p.x - self.lo.x) * i64::from(self.area.right - self.area.left))
                    / (i64::from(self.hi.x - self.lo.x).max(1))) as i32,
            y: self.area.bottom
                - ((i64::from(p.y - self.lo.y) * i64::from(self.area.bottom - self.area.top))
                    / (i64::from(self.hi.y - self.lo.y).max(1))) as i32,
        }
    }
    fn data(self, x: i32, y: i32) -> DataPoint {
        DataPoint {
            x: self.lo.x
                + ((i64::from(x - self.area.left) * i64::from(self.hi.x - self.lo.x))
                    / (i64::from(self.area.right - self.area.left).max(1)))
                    as i32,
            y: self.lo.y
                + ((i64::from(self.area.bottom - y) * i64::from(self.hi.y - self.lo.y))
                    / (i64::from(self.area.bottom - self.area.top).max(1)))
                    as i32,
        }
    }
}
fn coordinate(value: LPARAM) -> (i32, i32) {
    (
        (value as u32 as u16 as i16) as i32,
        ((value as u32 >> 16) as u16 as i16) as i32,
    )
}
fn geometry(hwnd: HWND, c: &Curve) -> Geometry {
    let mut r: RECT = unsafe { std::mem::zeroed() };
    unsafe {
        GetClientRect(hwnd, &mut r);
    }
    let view = unsafe { state(hwnd) }
        .and_then(|s| s.viewport.get())
        .unwrap_or_else(|| Viewport::fit(c));
    Geometry::new(r, view)
}
fn deliver_result(hwnd: HWND, result: Result<bool, String>) {
    match result {
        Ok(true) => notify(hwnd, 1),
        Ok(false) => notify(hwnd, 0),
        Err(e) => {
            if let Some(s) = unsafe { state(hwnd) } {
                *s.error.borrow_mut() = e;
            }
            notify(hwnd, 2);
        }
    }
}
fn modifiers(s: &State, mouse: usize) -> Modifiers {
    let keys = s.modifiers.get();
    Modifiers {
        shift: keys.shift || mouse & 4 != 0 || unsafe { GetKeyState(0x10) < 0 },
        ctrl: keys.ctrl || mouse & 8 != 0 || unsafe { GetKeyState(0x11) < 0 },
        alt: keys.alt || unsafe { GetKeyState(0x12) < 0 },
    }
}
fn nearest(g: Geometry, c: &Curve, x: i32, y: i32) -> Option<usize> {
    c.points
        .iter()
        .enumerate()
        .filter(|(_, p)| (g.lo.x..=g.hi.x).contains(&p.x) && (g.lo.y..=g.hi.y).contains(&p.y))
        .map(|(i, p)| {
            let p = g.screen(*p);
            (i, i64::from(p.x - x).pow(2) + i64::from(p.y - y).pow(2))
        })
        .min_by_key(|(_, d)| *d)
        .filter(|(_, d)| *d <= 225)
        .map(|(i, _)| i)
}
fn finish_drag(hwnd: HWND, cancel: bool) {
    if let Some(s) = unsafe { state(hwnd) } {
        if s.drag.borrow_mut().take().is_some() {
            let changed = if let Some(c) = s.model.borrow_mut().as_mut() {
                if cancel {
                    c.cancel_gesture()
                } else {
                    c.end_gesture();
                    false
                }
            } else {
                false
            };
            unsafe {
                ReleaseCapture();
            }
            refresh(hwnd);
            notify(hwnd, usize::from(changed));
        }
    }
}
fn interact(hwnd: HWND, x: i32, y: i32) {
    let Some(s) = (unsafe { state(hwnd) }) else {
        return;
    };
    let result = mutate(hwnd, |c| match s.drag.borrow().as_ref() {
        Some(Drag::Band {
            x: start,
            geometry: g,
        }) => {
            c.select_voltage_range(*start, g.data(x, y).x);
            Ok(false)
        }
        Some(Drag::Point {
            start,
            down,
            geometry: g,
            mode,
        }) => {
            let p = g.data(x, y);
            if c.kind == Kind::Gpu {
                c.drag_gpu(start, *mode, p.y - down.y)
            } else {
                c.set(
                    c.selected,
                    DataPoint {
                        x: start[c.selected].x + p.x - down.x,
                        y: start[c.selected].y + p.y - down.y,
                    },
                    true,
                )
            }
        }
        None => Ok(false),
    });
    deliver_result(hwnd, result);
}
fn zoom(hwnd: HWND, vertical: bool, inward: bool, at: Option<DataPoint>) {
    let Some(s) = (unsafe { state(hwnd) }) else {
        return;
    };
    if let Some(c) = s.model.borrow().as_ref() {
        let mut v = s.viewport.get().unwrap_or_else(|| Viewport::fit(c));
        v.zoom(c, at.unwrap_or(c.points[c.selected]), vertical, inward);
        s.viewport.set(Some(v));
        refresh(hwnd);
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: Pointer belongs to this canvas; messages run on its creating UI thread.
    unsafe {
        if message == WM_NCCREATE {
            let c = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, c.lpCreateParams as isize);
            (*(c.lpCreateParams as *const State)).ownership.set(1);
        }
        let Some(s) = state(hwnd) else {
            return DefWindowProcW(hwnd, message, w, l);
        };
        match message {
            WM_PRINTCLIENT => {
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                draw(
                    w as HDC,
                    rect,
                    s.model.borrow().as_ref(),
                    s.font,
                    false,
                    s.viewport.get(),
                    None,
                );
                0
            }
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let dc = BeginPaint(hwnd, &mut ps);
                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                let memory = CreateCompatibleDC(dc);
                let bitmap = CreateCompatibleBitmap(dc, rect.right.max(1), rect.bottom.max(1));
                if !memory.is_null() && !bitmap.is_null() {
                    let old = SelectObject(memory, bitmap);
                    draw(
                        memory,
                        rect,
                        s.model.borrow().as_ref(),
                        s.font,
                        GetFocus() == hwnd,
                        s.viewport.get(),
                        s.hover.get(),
                    );
                    BitBlt(dc, 0, 0, rect.right, rect.bottom, memory, 0, 0, SRCCOPY);
                    SelectObject(memory, old);
                } else {
                    draw(
                        dc,
                        rect,
                        s.model.borrow().as_ref(),
                        s.font,
                        GetFocus() == hwnd,
                        s.viewport.get(),
                        s.hover.get(),
                    );
                }
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                if !memory.is_null() {
                    DeleteDC(memory);
                }
                EndPaint(hwnd, &ps);
                0
            }
            WM_ERASEBKGND => 1,
            WM_KILLFOCUS => {
                finish_drag(hwnd, false);
                s.modifiers.set(Modifiers::default());
                s.hover.set(None);
                InvalidateRect(hwnd, ptr::null(), 0);
                0
            }
            WM_SETFOCUS | WM_SIZE | WM_ENABLE => {
                InvalidateRect(hwnd, ptr::null(), 0);
                0
            }
            WM_GETDLGCODE => {
                if w == 0x09 {
                    0
                } else {
                    (DLGC_WANTARROWS | DLGC_WANTCHARS | DLGC_WANTALLKEYS) as isize
                }
            }
            WM_LBUTTONDOWN => {
                SetFocus(hwnd);
                let (x, y) = coordinate(l);
                let keys = modifiers(s, w);
                let drag = {
                    let mut model = s.model.borrow_mut();
                    if let Some(c) = model.as_mut() {
                        let g = geometry(hwnd, c);
                        if let Some(i) = nearest(g, c, x, y) {
                            if c.selection().is_some_and(|(a, b)| (a..=b).contains(&i)) {
                                c.selected = i;
                            } else {
                                c.select(i, false);
                            }
                            c.begin_gesture();
                            Some(Drag::Point {
                                start: c.points.clone(),
                                down: g.data(x, y),
                                geometry: g,
                                mode: if keys.ctrl {
                                    DragMode::Tilt
                                } else if keys.shift {
                                    DragMode::ReferenceOffset
                                } else if keys.alt {
                                    DragMode::CurrentOffset
                                } else {
                                    DragMode::Point
                                },
                            })
                        } else if keys.shift
                            && c.kind == Kind::Gpu
                            && x >= g.area.left
                            && x <= g.area.right
                            && y >= g.area.top
                            && y <= g.area.bottom
                        {
                            c.begin_gesture();
                            Some(Drag::Band {
                                x: g.data(x, y).x,
                                geometry: g,
                            })
                        } else {
                            c.clear_selection();
                            None
                        }
                    } else {
                        None
                    }
                };
                if drag.is_some() {
                    SetCapture(hwnd);
                }
                *s.drag.borrow_mut() = drag;
                refresh(hwnd);
                notify(hwnd, 0);
                0
            }
            WM_MOUSEMOVE => {
                let (x, y) = coordinate(l);
                if s.drag.borrow().is_some() {
                    interact(hwnd, x, y);
                } else {
                    let near = s
                        .model
                        .borrow()
                        .as_ref()
                        .and_then(|c| nearest(geometry(hwnd, c), c, x, y));
                    if s.hover.replace(near) != near {
                        InvalidateRect(hwnd, ptr::null(), 0);
                    }
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    TrackMouseEvent(&mut track);
                }
                0
            }
            WM_MOUSELEAVE => {
                s.hover.set(None);
                InvalidateRect(hwnd, ptr::null(), 0);
                0
            }
            WM_LBUTTONUP => {
                finish_drag(hwnd, false);
                0
            }
            WM_CAPTURECHANGED => {
                finish_drag(hwnd, false);
                0
            }
            WM_LBUTTONDBLCLK => {
                finish_drag(hwnd, true);
                let _ = mutate(hwnd, |c| {
                    c.clear_selection();
                    Ok(false)
                });
                notify(hwnd, 0);
                0
            }
            WM_MOUSEWHEEL => {
                if s.drag.borrow().is_some() {
                    return 0;
                }
                let keys = modifiers(s, w & 0xffff);
                let inward = (w >> 16) as u16 as i16 > 0;
                let (x, y) = coordinate(l);
                let mut pos = POINT { x, y };
                ScreenToClient(hwnd, &mut pos);
                if keys.shift {
                    if let Some(c) = s.model.borrow().as_ref() {
                        let mut v = s.viewport.get().unwrap_or_else(|| Viewport::fit(c));
                        v.pan(c, if inward { -1 } else { 1 });
                        s.viewport.set(Some(v));
                    }
                    refresh(hwnd);
                } else {
                    let at = s
                        .model
                        .borrow()
                        .as_ref()
                        .map(|c| geometry(hwnd, c).data(pos.x, pos.y));
                    zoom(hwnd, keys.ctrl, inward, at);
                }
                0
            }
            ZOOM_IN | ZOOM_OUT => {
                zoom(hwnd, false, message == ZOOM_IN, None);
                0
            }
            FIT => {
                s.viewport.set(s.model.borrow().as_ref().map(Viewport::fit));
                refresh(hwnd);
                0
            }
            UNDO | REDO => {
                finish_drag(hwnd, false);
                let result = mutate(hwnd, |c| {
                    Ok(if message == UNDO { c.undo() } else { c.redo() })
                });
                deliver_result(hwnd, result);
                0
            }
            WM_KEYUP | WM_SYSKEYUP => {
                let mut m = s.modifiers.get();
                match w {
                    0x10 => m.shift = false,
                    0x11 => m.ctrl = false,
                    0x12 => m.alt = false,
                    _ => {}
                }
                s.modifiers.set(m);
                if message == WM_SYSKEYUP {
                    DefWindowProcW(hwnd, message, w, l)
                } else {
                    0
                }
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let mut m = s.modifiers.get();
                match w {
                    0x10 => m.shift = true,
                    0x11 => m.ctrl = true,
                    0x12 => m.alt = true,
                    _ => {}
                }
                s.modifiers.set(m);
                let keys = modifiers(s, 0);
                if [0xbb, 0x6b, 0xbd, 0x6d].contains(&w) {
                    zoom(hwnd, keys.ctrl, w == 0xbb || w == 0x6b, None);
                    return 0;
                }
                if w == 0x30 && keys.ctrl {
                    SendMessageW(hwnd, FIT, 0, 0);
                    return 0;
                }
                if w == 0x0d && !keys.shift {
                    PostMessageW(GetParent(hwnd), EDIT_VALUE, 0, hwnd as isize);
                    return 0;
                }
                let key = match w {
                    0x25 => Some(Key::Left),
                    0x27 => Some(Key::Right),
                    0x26 => Some(Key::Up),
                    0x28 => Some(Key::Down),
                    0x24 => Some(Key::Home),
                    0x23 => Some(Key::End),
                    0x1b => Some(Key::Escape),
                    0x2d => Some(Key::Insert),
                    0x2e => Some(Key::Delete),
                    0x41 if keys.ctrl => Some(Key::All),
                    0x5a if keys.ctrl => Some(if keys.shift { Key::Redo } else { Key::Undo }),
                    0x59 if keys.ctrl || keys.shift => Some(Key::Redo),
                    0x0d if keys.shift => Some(Key::Flatten),
                    0x44 if keys.ctrl => Some(Key::Reset),
                    _ => None,
                };
                if let Some(key) = key {
                    if s.drag.borrow().is_some() {
                        if w == 0x1b {
                            finish_drag(hwnd, true);
                        }
                        return 0;
                    }
                    let result = mutate(hwnd, |c| curve_input::edit(c, key, keys));
                    deliver_result(hwnd, result);
                } else if message == WM_SYSKEYDOWN {
                    return DefWindowProcW(hwnd, message, w, l);
                }
                0
            }
            WM_NCDESTROY => {
                s.ownership.set(2);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let p = s as *const State as *mut State;
                drop(Box::from_raw(p));
                DefWindowProcW(hwnd, message, w, l)
            }
            _ => DefWindowProcW(hwnd, message, w, l),
        }
    }
}
pub fn create(parent: HWND, id: usize, _font: HFONT) -> Result<HWND, String> {
    // SAFETY: Class text is copied by Windows; Box remains owned by the canvas.
    unsafe {
        let instance = GetModuleHandleW(ptr::null());
        let name = wide("OmenHubPPCurveCanvas");
        let mut class: WNDCLASSW = std::mem::zeroed();
        class.lpfnWndProc = Some(procedure);
        class.hInstance = instance;
        class.lpszClassName = name.as_ptr();
        class.hCursor = LoadCursorW(ptr::null_mut(), IDC_CROSS);
        class.style = CS_DBLCLKS;
        if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let ownership = Rc::new(Cell::new(0));
        let mut font_info: LOGFONTW = std::mem::zeroed();
        font_info.lfHeight = -14;
        for (a, b) in font_info.lfFaceName.iter_mut().zip(wide("Segoe UI")) {
            *a = b;
        }
        let chart_font = CreateFontIndirectW(&font_info);
        let boxed = Box::into_raw(Box::new(State {
            model: RefCell::new(None),
            font: chart_font,
            drag: RefCell::new(None),
            viewport: Cell::new(None),
            modifiers: Cell::new(Modifiers::default()),
            hover: Cell::new(None),
            error: RefCell::new(String::new()),
            ownership: ownership.clone(),
        }));
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            wide("Kurvenansicht").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            0,
            0,
            400,
            250,
            parent,
            id as HMENU,
            instance,
            boxed.cast(),
        );
        if hwnd.is_null() {
            if ownership.get() == 0 {
                drop(Box::from_raw(boxed));
            }
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(hwnd)
    }
}
// COLORREFs, shared with the window chrome. Neutral data; accent means selection.
pub const BACKGROUND: u32 = 0x00221f1c;
pub const FOREGROUND: u32 = 0x00e5e2de;
pub const SURFACE: u32 = 0x002d2a27;
pub const ACCENT: u32 = 0x00f4cd85;
const PLOT: u32 = 0x001f1d1c;
const GRID: u32 = 0x003c3834;
const GRID_MINOR: u32 = 0x002c2926;
const REFERENCE: u32 = 0x00978d84;
const DRAFT: u32 = 0x00d3cdc7;
unsafe fn label(dc: HDC, x: i32, y: i32, value: &str) {
    let s = wide(value);
    unsafe {
        TextOutW(dc, x, y, s.as_ptr(), (s.len() - 1) as i32);
    }
}
unsafe fn width(dc: HDC, value: &str) -> i32 {
    let s = wide(value);
    let mut size: SIZE = unsafe { std::mem::zeroed() };
    unsafe {
        GetTextExtentPoint32W(dc, s.as_ptr(), (s.len() - 1) as i32, &mut size);
    }
    size.cx
}
unsafe fn draw(
    dc: HDC,
    rect: RECT,
    curve: Option<&Curve>,
    font: HFONT,
    focus: bool,
    view: Option<Viewport>,
    hover: Option<usize>,
) {
    // SAFETY: A living window or owned memory DC; every selected object is restored.
    unsafe {
        let saved = SaveDC(dc);
        let brush = CreateSolidBrush(PLOT);
        FillRect(dc, &rect, brush);
        DeleteObject(brush);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, FOREGROUND);
        SelectObject(
            dc,
            if font.is_null() {
                GetStockObject(DEFAULT_GUI_FONT)
            } else {
                font
            },
        );
        if let Some(curve) = curve {
            let g = Geometry::new(rect, view.unwrap_or_else(|| Viewport::fit(curve)));
            let xstep = tick_step(g.hi.x - g.lo.x, g.area.right - g.area.left, 55);
            let ystep = tick_step(g.hi.y - g.lo.y, g.area.bottom - g.area.top, 26);
            let clipped = SaveDC(dc);
            IntersectClipRect(
                dc,
                g.area.left,
                g.area.top,
                g.area.right + 1,
                g.area.bottom + 1,
            );
            if let Some((a, b)) = curve.selection() {
                let left = g.screen(curve.points[a]).x;
                let right = g.screen(curve.points[b]).x;
                let band = CreateSolidBrush(0x00403127);
                FillRect(
                    dc,
                    &RECT {
                        left: left - 4,
                        top: g.area.top,
                        right: right + 5,
                        bottom: g.area.bottom,
                    },
                    band,
                );
                DeleteObject(band);
            }
            for (color, xs, ys) in [
                (GRID_MINOR, (xstep / 5).max(1), (ystep / 2).max(1)),
                (GRID, xstep, ystep),
            ] {
                let pen = CreatePen(PS_SOLID, 1, color);
                let old = SelectObject(dc, pen);
                let first_x = ((g.lo.x + xs as i32 - 1) / xs as i32) * xs as i32;
                for x in (first_x..=g.hi.x).step_by(xs) {
                    let p = g.screen(DataPoint { x, y: g.lo.y });
                    MoveToEx(dc, p.x, g.area.top, ptr::null_mut());
                    LineTo(dc, p.x, g.area.bottom);
                }
                let first_y = ((g.lo.y + ys as i32 - 1) / ys as i32) * ys as i32;
                for y in (first_y..=g.hi.y).step_by(ys) {
                    let p = g.screen(DataPoint { x: g.lo.x, y });
                    MoveToEx(dc, g.area.left, p.y, ptr::null_mut());
                    LineTo(dc, g.area.right, p.y);
                }
                SelectObject(dc, old);
                DeleteObject(pen);
            }
            for (nodes, color, style) in [
                (curve.reference(), REFERENCE, PS_DOT),
                (&curve.points[..], DRAFT, PS_SOLID),
            ] {
                let points = nodes.iter().map(|p| g.screen(*p)).collect::<Vec<_>>();
                let pen = CreatePen(style, 1, color);
                let old = SelectObject(dc, pen);
                Polyline(dc, points.as_ptr(), points.len() as i32);
                if style == PS_SOLID {
                    let fill = CreateSolidBrush(color);
                    let old_fill = SelectObject(dc, fill);
                    for p in &points {
                        Rectangle(dc, p.x - 2, p.y - 2, p.x + 3, p.y + 3);
                    }
                    SelectObject(dc, old_fill);
                    DeleteObject(fill);
                }
                SelectObject(dc, old);
                DeleteObject(pen);
            }
            let node = curve.points[curve.selected];
            let p = g.screen(node);
            if (g.lo.x..=g.hi.x).contains(&node.x) && (g.lo.y..=g.hi.y).contains(&node.y) {
                let pen = CreatePen(PS_DOT, 1, REFERENCE);
                let old = SelectObject(dc, pen);
                MoveToEx(dc, g.area.left, p.y, ptr::null_mut());
                LineTo(dc, g.area.right, p.y);
                MoveToEx(dc, p.x, g.area.top, ptr::null_mut());
                LineTo(dc, p.x, g.area.bottom);
                SelectObject(dc, old);
                DeleteObject(pen);
                let pen = CreatePen(PS_SOLID, 1, ACCENT);
                let old = SelectObject(dc, pen);
                let fill = CreateSolidBrush(ACCENT);
                let old_fill = SelectObject(dc, fill);
                Rectangle(dc, p.x - 5, p.y - 5, p.x + 6, p.y + 6);
                SelectObject(dc, old_fill);
                DeleteObject(fill);
                SelectObject(dc, old);
                DeleteObject(pen);
            }
            if let Some(i) = hover.filter(|i| *i < curve.points.len()) {
                let point = curve.points[i];
                let pos = g.screen(point);
                let (x, y) = curve.kind.units();
                let value = format!("{} {} · {} {}", point.x, x, point.y, y);
                let size = width(dc, &value) + 20;
                let left = (pos.x + 14).clamp(g.area.left, (g.area.right - size).max(g.area.left));
                let top = (pos.y - 30).clamp(g.area.top, (g.area.bottom - 24).max(g.area.top));
                let fill = CreateSolidBrush(SURFACE);
                FillRect(
                    dc,
                    &RECT {
                        left,
                        top,
                        right: left + size,
                        bottom: top + 24,
                    },
                    fill,
                );
                DeleteObject(fill);
                label(dc, left + 10, top + 4, &value);
            }
            RestoreDC(dc, clipped);
            let first_y = ((g.lo.y + ystep as i32 - 1) / ystep as i32) * ystep as i32;
            for y in (first_y..=g.hi.y).step_by(ystep) {
                let point = g.screen(DataPoint { x: g.lo.x, y });
                let value = y.to_string();
                label(
                    dc,
                    g.area.left - 10 - width(dc, &value),
                    point.y - 7,
                    &value,
                );
            }
            let first_x = ((g.lo.x + xstep as i32 - 1) / xstep as i32) * xstep as i32;
            for x in (first_x..=g.hi.x).step_by(xstep) {
                let point = g.screen(DataPoint { x, y: g.lo.y });
                let value = x.to_string();
                label(
                    dc,
                    point.x - width(dc, &value) / 2,
                    g.area.bottom + 8,
                    &value,
                );
            }
            let (x, y) = curve.kind.units();
            let ylabel = if curve.kind == Kind::Gpu {
                "Taktrate, MHz"
            } else {
                "Lüfterwert, %"
            };
            let xlabel = if curve.kind == Kind::Gpu {
                "Spannung, mV"
            } else {
                "Temperatur, °C"
            };
            label(dc, g.area.left, 10, ylabel);
            label(
                dc,
                g.area.right - width(dc, xlabel),
                rect.bottom - 21,
                xlabel,
            );
            let selection = curve.selection().map_or(String::new(), |(a, b)| {
                format!(" · Auswahl: {} Punkte", b - a + 1)
            });
            let point = format!(
                "Ausgewählt: {} {} · {} {}{}",
                node.x, x, node.y, y, selection
            );
            label(dc, g.area.right - width(dc, &point), 10, &point);
            label(
                dc,
                g.area.left,
                rect.bottom - 21,
                "Quadrate: Entwurf   ·   Gepunktet: beim Laden",
            );
        } else {
            label(
                dc,
                24,
                30,
                "Kurve nicht verfügbar. Lüfter-Entwürfe können weiterhin bearbeitet werden.",
            );
        }
        if focus {
            let mut r = rect;
            r.left += 3;
            r.top += 3;
            r.right -= 3;
            r.bottom -= 3;
            let pen = CreatePen(PS_SOLID, 1, ACCENT);
            let old = SelectObject(dc, pen);
            let old_fill = SelectObject(dc, GetStockObject(HOLLOW_BRUSH));
            Rectangle(dc, r.left, r.top, r.right, r.bottom);
            SelectObject(dc, old_fill);
            SelectObject(dc, old);
            DeleteObject(pen);
        }
        RestoreDC(dc, saved);
    }
}
/// Render our widget code to an owned DIB, never capture a desktop or other app.
pub fn render(path: &Path, curve: &Curve) -> Result<(), String> {
    unsafe {
        let mut info: LOGFONTW = std::mem::zeroed();
        info.lfHeight = -14;
        for (a, b) in info.lfFaceName.iter_mut().zip(wide("Segoe UI")) {
            *a = b;
        }
        let font = CreateFontIndirectW(&info);
        let result = bitmap(path, 1320, 680, |dc, rect| {
            draw(dc, rect, Some(curve), font, false, None, None)
        });
        if !font.is_null() {
            DeleteObject(font);
        }
        result
    }
}
/// Print only this application's owned window hierarchy into a memory DIB.
/// This uses WM_PRINT, not screen capture, and never reads another application's pixels.
pub fn render_window(hwnd: HWND, path: &Path) -> Result<(), String> {
    unsafe {
        // Classic single-line EDIT printing depends on visible control state.
        // Activate the owned preview off-screen without keyboard activation;
        // capture production controls, then the preview caller destroys it.
        if IsWindowVisible(hwnd) == 0 {
            SetWindowPos(
                hwnd,
                ptr::null_mut(),
                -30000,
                -30000,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            RedrawWindow(
                hwnd,
                ptr::null(),
                ptr::null_mut(),
                RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
            );
        }
        let mut rect: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rect);
        bitmap(path, rect.right, rect.bottom, |dc, _| {
            // Child WM_PRINT includes its own nonclient border. Position each
            // visible child in client space; a top-level print adds a titlebar.
            let fill = GetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND) as HBRUSH;
            FillRect(dc, &rect, fill);
            // Ask the production parent to paint its client surfaces before
            // printing children; a class brush alone misses dashboard panels.
            SendMessageW(hwnd, WM_PRINTCLIENT, dc as usize, PRF_CLIENT as isize);
            let mut child = GetWindow(hwnd, GW_CHILD);
            while !child.is_null() {
                if GetWindowLongPtrW(child, GWL_STYLE) as u32 & WS_VISIBLE != 0 {
                    let mut r: RECT = std::mem::zeroed();
                    GetWindowRect(child, &mut r);
                    let mut p = POINT {
                        x: r.left,
                        y: r.top,
                    };
                    ScreenToClient(hwnd, &mut p);
                    let saved = SaveDC(dc);
                    IntersectClipRect(dc, p.x, p.y, p.x + r.right - r.left, p.y + r.bottom - r.top);
                    SetViewportOrgEx(dc, p.x, p.y, ptr::null_mut());
                    SendMessageW(
                        child,
                        WM_PRINT,
                        dc as usize,
                        (PRF_CLIENT | PRF_NONCLIENT | PRF_CHILDREN | PRF_ERASEBKGND) as isize,
                    );
                    RestoreDC(dc, saved);
                }
                child = GetWindow(child, GW_HWNDNEXT);
            }
        })
    }
}
fn bitmap(path: &Path, w: i32, h: i32, paint: impl FnOnce(HDC, RECT)) -> Result<(), String> {
    if !(1..=4096).contains(&w) || !(1..=4096).contains(&h) {
        return Err("Ungültige Render-Größe.".into());
    }
    unsafe {
        let dc = CreateCompatibleDC(ptr::null_mut());
        if dc.is_null() {
            return Err("Kein Render-Kontext verfügbar.".into());
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut bits = ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0);
        if bitmap.is_null() || bits.is_null() {
            DeleteDC(dc);
            return Err("Render-Bitmap fehlt.".into());
        }
        let old = SelectObject(dc, bitmap);
        paint(
            dc,
            RECT {
                left: 0,
                top: 0,
                right: w,
                bottom: h,
            },
        );
        GdiFlush();
        let count = (w * h * 4) as usize;
        let mut data = vec![];
        data.extend(b"BM");
        data.extend(((54 + count) as u32).to_le_bytes());
        data.extend(0u32.to_le_bytes());
        data.extend(54u32.to_le_bytes());
        data.extend(40u32.to_le_bytes());
        data.extend(w.to_le_bytes());
        data.extend((-h).to_le_bytes());
        data.extend(1u16.to_le_bytes());
        data.extend(32u16.to_le_bytes());
        data.extend(0u32.to_le_bytes());
        data.extend((count as u32).to_le_bytes());
        data.extend([0u8; 16]);
        data.extend(std::slice::from_raw_parts(bits as *const u8, count));
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        std::fs::write(path, data).map_err(|e| e.to_string())
    }
}
