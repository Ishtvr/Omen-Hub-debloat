//! Native controls with synthetic data only: no driver, profile store or helper.
use super::*;
use omen_core::{
    curve_editor::DragMode,
    curve_view::Viewport,
    vf::{VfNode, VfState},
};
use std::time::Instant;

struct Editor {
    hwnd: HWND,
    state: Box<State>,
}

fn gpu() -> Curve {
    Curve::gpu(
        VfState {
            nodes: (0..127)
                .map(|i| VfNode {
                    index: i,
                    voltage_uv: 600_000 + u32::from(i) * 5_000,
                    frequency_khz: 900_000 + u32::from(i) * 10_000,
                    delta_khz: 0,
                })
                .collect(),
            minimum_delta_khz: -400_000,
            maximum_delta_khz: 400_000,
            first_index: 0,
            last_index: 126,
        },
        None,
    )
    .unwrap()
}

impl Editor {
    fn new(curve: Curve) -> Self {
        let (tx, _) = mpsc::channel();
        let (_, rx) = mpsc::channel();
        let index = if curve.kind == Kind::Gpu { 0 } else { 1 };
        let state = Box::new(State {
            tx,
            rx,
            view: RefCell::new(None),
            controls: Cell::new(None),
            font: Cell::new(ptr::null_mut()),
            mode: Cell::new(index),
            allowed: [true; 3],
            busy: Cell::new(false),
            rendering: Cell::new(false),
            point_list: RefCell::new(None),
            dirty: Cell::new(false),
            graph_only: Cell::new(true),
            background: Cell::new(unsafe { CreateSolidBrush(plot::BACKGROUND) }),
            surface: Cell::new(unsafe { CreateSolidBrush(plot::SURFACE) }),
            theme: crate::theme::Theme::new(),
            draft: true,
            draft_result: RefCell::new(None),
            fans_merged: false,
        });
        let class = "OmenHubPPCurveTest";
        let hwnd = unsafe {
            crate::native_ui::register(class, Some(procedure)).unwrap();
            CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide("Curve fixture").as_ptr(),
                WS_OVERLAPPEDWINDOW,
                -30000,
                -30000,
                1240,
                780,
                ptr::null_mut(),
                ptr::null_mut(),
                GetModuleHandleW(ptr::null()),
                (&*state as *const State).cast(),
            )
        };
        assert!(!hwnd.is_null());
        show(
            &state,
            View {
                profiles: vec![],
                selected: None,
                binding: DeviceBinding {
                    manufacturer: "fixture".into(),
                    board: "fixture".into(),
                    bios: "fixture".into(),
                    cpu_names: vec![],
                    nvidia_uuid: None,
                    nvidia_driver: None,
                    nvidia_vbios: None,
                },
                rows: std::array::from_fn(|i| Row {
                    curve: if i == index {
                        Some(curve.clone())
                    } else {
                        None
                    },
                    enabled: i == index,
                    source: "fixture".into(),
                }),
            },
            "fixture",
        );
        Self { hwnd, state }
    }

    fn controls(&self) -> Controls {
        self.state.controls.get().unwrap()
    }

    fn send(&self, message: u32, w: usize, l: isize) {
        unsafe {
            SendMessageW(self.controls().canvas, message, w, l);
        }
    }

    fn mouse(&self, message: u32, p: POINT, keys: usize) {
        self.send(
            message,
            keys,
            (u32::from(p.x as u16) | (u32::from(p.y as u16) << 16)) as isize,
        );
    }

    fn pump(&self) {
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(
                &mut message,
                self.hwnd,
                plot::CHANGED,
                plot::CHANGED,
                PM_REMOVE,
            ) != 0
            {
                DispatchMessageW(&message);
            }
        }
    }

    fn screen(&self, point: Point) -> POINT {
        let model = plot::curve(self.controls().canvas).unwrap();
        let v = Viewport::initial(&model);
        let mut r: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            GetClientRect(self.controls().canvas, &mut r);
        }
        POINT {
            x: 72 + (point.x - v.lo.x) * (r.right - 100) / (v.hi.x - v.lo.x),
            y: r.bottom - 54 - (point.y - v.lo.y) * (r.bottom - 92) / (v.hi.y - v.lo.y),
        }
    }

    fn model(&self) -> Curve {
        plot::curve(self.controls().canvas).unwrap()
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd);
            DeleteObject(self.state.font.get());
            DeleteObject(self.state.background.get());
            DeleteObject(self.state.surface.get());
            let mut msg: MSG = std::mem::zeroed();
            PeekMessageW(&mut msg, ptr::null_mut(), WM_QUIT, WM_QUIT, PM_REMOVE);
        }
    }
}

#[test]
fn mouse_release_uses_final_position_and_is_one_undo_step() {
    let editor = Editor::new(Curve::fan(Kind::CpuFan, None).unwrap());
    let before = editor.model().points;
    let down = editor.screen(before[1]);
    let last = POINT {
        x: down.x,
        y: down.y - 12,
    };
    let up = POINT {
        x: down.x,
        y: down.y - 36,
    };
    editor.mouse(WM_LBUTTONDOWN, down, 1);
    editor.mouse(WM_MOUSEMOVE, last, 1);
    let intermediate = editor.model().points;
    editor.mouse(WM_LBUTTONUP, up, 0);
    editor.pump();
    let after = editor.model().points;
    assert!(
        after[1].y > intermediate[1].y,
        "release position was ignored"
    );
    assert!(editor.state.dirty.get());
    editor.send(plot::UNDO, 0, 0);
    editor.pump();
    assert_eq!(editor.model().points, before);
    assert!(!editor.model().can_undo());
    editor.send(plot::REDO, 0, 0);
    editor.pump();
    assert_eq!(editor.model().points, after);
}

#[test]
fn cancelled_drag_cannot_be_reapplied_by_mouse_release() {
    let editor = Editor::new(Curve::fan(Kind::CpuFan, None).unwrap());
    let before = editor.model().points;
    let down = editor.screen(before[1]);
    let up = POINT {
        x: down.x,
        y: down.y - 36,
    };
    editor.mouse(WM_LBUTTONDOWN, down, 1);
    editor.mouse(WM_MOUSEMOVE, up, 1);
    editor.send(WM_KEYDOWN, 0x1b, 0);
    editor.mouse(WM_LBUTTONUP, up, 0);
    editor.pump();
    assert_eq!(editor.model().points, before);
    assert!(!editor.model().can_undo());
}

fn assert_list(editor: &Editor) {
    let controls = editor.controls();
    let model = editor.model();
    let (x, y) = model.kind.units();
    unsafe {
        assert_eq!(
            SendMessageW(controls.points, CB_GETCOUNT, 0, 0),
            model.points.len() as isize
        );
        assert_eq!(
            SendMessageW(controls.points, CB_GETCURSEL, 0, 0),
            model.selected as isize
        );
        for (i, point) in model.points.iter().enumerate() {
            let length = SendMessageW(controls.points, CB_GETLBTEXTLEN, i, 0);
            assert!(length >= 0);
            let mut buffer = vec![0u16; length as usize + 1];
            SendMessageW(
                controls.points,
                CB_GETLBTEXT,
                i,
                buffer.as_mut_ptr() as isize,
            );
            assert_eq!(
                String::from_utf16(&buffer[..length as usize]).unwrap(),
                format!("{} {} · {} {}", point.x, x, point.y, y)
            );
        }
    }
    assert_eq!(
        crate::native_ui::value(controls.x),
        model.points[model.selected].x.to_string()
    );
    assert_eq!(
        crate::native_ui::value(controls.y),
        model.points[model.selected].y.to_string()
    );
}

#[test]
fn point_selection_keeps_native_rows_and_edits_refresh_exact_values() {
    let editor = Editor::new(gpu());
    let c = editor.controls();
    unsafe {
        for i in 0..127 {
            SendMessageW(c.points, CB_SETITEMDATA, i, (i + 100) as isize);
        }
    }
    plot::mutate(c.canvas, |curve| {
        curve.select(60, false);
        Ok(false)
    })
    .unwrap();
    sync(&editor.state);
    assert_list(&editor);
    unsafe {
        for i in 0..127 {
            assert_eq!(
                SendMessageW(c.points, CB_GETITEMDATA, i, 0),
                (i + 100) as isize
            );
        }
    }
    plot::mutate(c.canvas, |curve| {
        let point = curve.points[60];
        curve.set(
            60,
            Point {
                y: point.y + 1,
                ..point
            },
            false,
        )
    })
    .unwrap();
    sync(&editor.state);
    assert_list(&editor);
    unsafe {
        assert_eq!(SendMessageW(c.points, CB_GETITEMDATA, 59, 0), 159);
        assert_eq!(SendMessageW(c.points, CB_GETITEMDATA, 61, 0), 161);
    }
    let points = editor.model().points;
    plot::mutate(c.canvas, |curve| {
        curve.drag_gpu(&points, DragMode::CurrentOffset, 20)
    })
    .unwrap();
    sync(&editor.state);
    assert_list(&editor);
    // Batching redraw must not unhide controls in the chart-only layout.
    assert_eq!(
        unsafe { GetWindowLongPtrW(c.points, GWL_STYLE) } as u32 & WS_VISIBLE,
        0
    );
    editor.state.graph_only.set(false);
    layout(editor.hwnd, &editor.state);
    unsafe {
        ShowWindow(editor.hwnd, SW_SHOWNOACTIVATE);
    }
    assert_ne!(unsafe { IsWindowVisible(c.points) }, 0);
    let points = editor.model().points;
    plot::mutate(c.canvas, |curve| {
        curve.drag_gpu(&points, DragMode::CurrentOffset, -10)
    })
    .unwrap();
    sync(&editor.state);
    assert_list(&editor);
    assert_ne!(unsafe { IsWindowVisible(c.points) }, 0);
    assert!(unsafe { GetPropW(c.points, wide("SysSetRedraw").as_ptr()) }.is_null());
}

#[test]
fn curve_switch_keeps_latest_points_and_undo_history() {
    let editor = Editor::new(gpu());
    let before = editor.model().points;
    edit(&editor.state, |curve| {
        curve.drag_gpu(&before, DragMode::CurrentOffset, 15)
    });
    let edited = editor.model().points;
    editor.state.view.borrow_mut().as_mut().unwrap().rows[1].curve =
        Some(Curve::fan(Kind::CpuFan, None).unwrap());
    mode(&editor.state, 1, true);
    assert_list(&editor);
    edit(&editor.state, |curve| {
        curve.insert()?;
        Ok(true)
    });
    assert_list(&editor);
    let fan = editor.model().points;
    mode(&editor.state, 0, true);
    assert_eq!(editor.model().points, edited);
    assert_list(&editor);
    editor.send(plot::UNDO, 0, 0);
    editor.pump();
    assert_eq!(editor.model().points, before);
    mode(&editor.state, 1, true);
    assert_eq!(editor.model().points, fan);
    edit(&editor.state, |curve| {
        curve.remove()?;
        Ok(true)
    });
    assert_list(&editor);
}

#[test]
fn draft_save_returns_live_canvas_even_without_a_presentation_update() {
    let editor = Editor::new(gpu());
    let before = editor.model().points;
    plot::mutate(editor.controls().canvas, |curve| {
        curve.drag_gpu(&before, DragMode::CurrentOffset, 15)
    })
    .unwrap();
    let expected = editor.model().value().unwrap();
    unsafe {
        SendMessageW(editor.hwnd, WM_COMMAND, SAVE, 0);
    }
    assert_eq!(*editor.state.draft_result.borrow(), Some(expected));
}

#[test]
fn gpu_group_release_without_mousemove_preserves_anchors_and_history() {
    let editor = Editor::new(gpu());
    let before = editor.model().points;
    let down = editor.screen(before[70]);
    let up = POINT {
        x: down.x + 80,
        y: down.y - 36,
    };
    editor.mouse(WM_LBUTTONDOWN, down, 5);
    editor.mouse(WM_LBUTTONUP, up, 4);
    editor.pump();
    let after = editor.model().points;
    let delta = after[0].y - before[0].y;
    assert!(delta > 0);
    for (a, b) in after.iter().zip(&before) {
        assert_eq!(a.x, b.x);
        assert_eq!(a.y - b.y, delta);
    }
    assert_list(&editor);
    editor.send(plot::UNDO, 0, 0);
    editor.pump();
    assert_eq!(editor.model().points, before);
    assert!(!editor.model().can_undo());
    editor.send(plot::REDO, 0, 0);
    editor.pump();
    assert_eq!(editor.model().points, after);
}

#[test]
fn lost_capture_ends_gesture_before_a_late_release() {
    let editor = Editor::new(Curve::fan(Kind::CpuFan, None).unwrap());
    let down = editor.screen(editor.model().points[1]);
    let moved = POINT {
        x: down.x,
        y: down.y - 12,
    };
    editor.mouse(WM_LBUTTONDOWN, down, 1);
    editor.mouse(WM_MOUSEMOVE, moved, 1);
    let expected = editor.model().points;
    editor.send(WM_CAPTURECHANGED, 0, 0);
    editor.mouse(
        WM_LBUTTONUP,
        POINT {
            x: down.x,
            y: down.y - 36,
        },
        0,
    );
    editor.pump();
    assert_eq!(editor.model().points, expected);
}

#[test]
#[ignore = "manual native UI timing, no hardware or saved state"]
fn interaction_benchmark() {
    let mut model = gpu();
    for i in 0..64 {
        let points = model.points.clone();
        model
            .drag_gpu(
                &points,
                DragMode::CurrentOffset,
                if i % 2 == 0 { 1 } else { -1 },
            )
            .unwrap();
    }
    let editor = Editor::new(model);
    let canvas = editor.controls().canvas;
    let start = Instant::now();
    for i in 0..1000 {
        plot::mutate(canvas, |c| {
            c.select(i % 127, false);
            Ok(false)
        })
        .unwrap();
        sync(&editor.state);
    }
    println!(
        "curve selection + native sync: 1000 updates, 127 points, 64 undo entries: {:?}",
        start.elapsed()
    );
    let points = editor.model().points;
    plot::mutate(canvas, |c| {
        c.begin_gesture();
        Ok(false)
    })
    .unwrap();
    let start = Instant::now();
    for i in 0..1000 {
        plot::mutate(canvas, |c| {
            c.drag_gpu(&points, DragMode::CurrentOffset, i % 30)
        })
        .unwrap();
        sync(&editor.state);
    }
    println!(
        "curve group drag + native sync: 1000 updates, 127 points, 64 undo entries: {:?}",
        start.elapsed()
    );
}
