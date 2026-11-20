//! Presentation adapter for the existing platform-independent curve editor.
use super::*;
use omen_core::{
    curve_editor::DragMode,
    curve_input::{self, Key, Modifiers},
};

/// An actual path, rather than rotated rectangles (rotation is unavailable in
/// Slint's software renderer). One fixed viewbox scales with the input plot.
fn path(points: &[Point], view: Viewport) -> String {
    use std::fmt::Write;
    let sx = (view.hi.x - view.lo.x).max(1) as f64;
    let sy = (view.hi.y - view.lo.y).max(1) as f64;
    let mut commands = String::with_capacity(points.len() * 26);
    for (i, point) in points.iter().enumerate() {
        let x = f64::from(point.x - view.lo.x) * 1000.0 / sx;
        let y = 1000.0 - f64::from(point.y - view.lo.y) * 1000.0 / sy;
        let _ = write!(
            commands,
            "{} {x:.3} {y:.3} ",
            if i == 0 { 'M' } else { 'L' }
        );
    }
    commands
}

/// Checking a new curve field does not populate its initial empty UI value.
/// Start that placeholder at the driver's factory curve; existing nonempty
/// targets still undergo the editor's complete grid/range validation.
fn gpu_editor(state: omen_core::vf::VfState, value: Option<&Value>) -> Result<Curve, String> {
    let value = value.filter(|v| !matches!(v, Value::VfCurve(points) if points.is_empty()));
    Curve::gpu(state, value)
}

pub(super) struct Editor {
    pub curve: Curve,
    view: Viewport,
    gesture: Option<Gesture>,
    selection_mode: bool,
    /// Unused wheel delta below one zoom/pan step.
    wheel: f32,
    /// GPU driver default curve; offsets and the reference line use it.
    factory: Option<Vec<Point>>,
    /// Point under the pointer while no button is pressed.
    hover: Option<usize>,
}
impl Host {
    pub(super) fn gpu_curve_dirty(&self) -> bool {
        let selected = self
            .fields
            .borrow()
            .iter()
            .zip(preset_fields::fields())
            .find(|(_, f)| f.feature == Feature::GpuVfCurve)
            .map(|(d, _)| d.value.clone());
        let changed =
            |curve: &Curve| curve.points != curve.reference() && curve.value().ok() != selected;
        self.gpu_draft.borrow().as_ref().is_some_and(changed)
            || self
                .curve
                .borrow()
                .as_ref()
                .is_some_and(|e| e.curve.kind == Kind::Gpu && changed(&e.curve))
    }
    pub(super) fn curves_dirty(&self) -> bool {
        self.gpu_curve_dirty() || self.fan_library_dirty()
    }
    pub(super) fn open_new_gpu_curve(&self) {
        // New profile means a fresh driver snapshot's factory curve, even if
        // another profile or a cached editor previously held an offset/plateau.
        *self.gpu_draft.borrow_mut() = None;
        let result = self
            .view
            .borrow()
            .as_ref()
            .map(|v| v.curve.clone().and_then(|state| gpu_editor(state, None)));
        match result {
            Some(Ok(curve)) => self.open_loaded_curve(curve),
            Some(Err(error)) => self.status(&crate::i18n::error_message(&error), true),
            None => self.status(t("preset.still_loading"), true),
        }
    }
    pub(super) fn open_curve(&self, gpu: bool) {
        if !gpu {
            self.open_fan_library_curve();
            return;
        }
        let cached = if gpu {
            self.gpu_draft.borrow().clone()
        } else {
            self.fan_draft.borrow().clone()
        };
        let result = if let Some(curve) = cached {
            Ok(curve)
        } else if gpu {
            let view = self.view.borrow();
            let Some(view) = view.as_ref() else { return };
            let value = self
                .fields
                .borrow()
                .iter()
                .zip(preset_fields::fields())
                .find(|(_, f)| f.feature == Feature::GpuVfCurve)
                .and_then(|(d, _)| d.included.then(|| d.value.clone()));
            view.curve
                .clone()
                .and_then(|state| gpu_editor(state, value.as_ref()))
        } else {
            unreachable!("Fan editor is handled by its library")
        };
        match result {
            Ok(curve) => self.open_loaded_curve(curve),
            Err(e) => self.status(&crate::i18n::error_message(&e), true),
        }
    }
    pub(super) fn open_loaded_curve(&self, curve: Curve) {
        let gpu = curve.kind == Kind::Gpu;
        *self.curve.borrow_mut() = Some(Editor::new(curve));
        if let Some(w) = self.window.upgrade() {
            let data = w.global::<Workbench>();
            data.set_curve_open(true);
            data.set_curve_gpu(gpu);
            if let Some(editor) = self.curve.borrow().as_ref() {
                data.set_range_from(
                    editor.curve.points[editor.curve.selected]
                        .x
                        .to_string()
                        .into(),
                );
                data.set_range_to(editor.curve.points.last().unwrap().x.to_string().into());
            }
            data.set_curve_title(
                t(if gpu {
                    "ui.gpu_curve"
                } else {
                    "ui.shared_curve"
                })
                .into(),
            );
        }
        self.draw_curve();
        self.status(t("ui.ready"), false);
        // The selector may restore its own focus after its selected callback.
        // Transfer focus once that callback/popup closure has completed.
        let weak = self.window.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = weak.upgrade()
                && w.global::<Workbench>().get_curve_open()
                && w.global::<Workbench>().get_confirm_kind() == 0
            {
                w.invoke_focus_curve();
            }
        });
    }
    pub(super) fn draw_curve(&self) {
        let borrowed = self.curve.borrow();
        let Some(editor) = borrowed.as_ref() else {
            return;
        };
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        let curve = &editor.curve;
        let v = editor.view;
        let baseline = editor.baseline();
        d.set_curve_path(path(&curve.points, v).into());
        d.set_reference_path(path(baseline, v).into());
        d.set_curve_driver_reference(editor.factory.is_some());
        d.set_curve_delta(driver_delta(editor).into());
        d.set_curve_selection_mode(editor.selection_mode);
        let selection = curve.selection();
        d.set_has_selection(selection.is_some());
        d.set_selection_label(
            selection
                .map_or_else(
                    || t("curve.no_selection").to_owned(),
                    |(a, b)| {
                        tf(
                            "curve.selection_count",
                            &[&(b - a + 1), &curve.points[a].x, &curve.points[b].x],
                        )
                    },
                )
                .into(),
        );
        let sx = (v.hi.x - v.lo.x).max(1) as f32;
        let sy = (v.hi.y - v.lo.y).max(1) as f32;
        d.set_selection_low(
            selection.map_or(0.0, |(a, _)| (curve.points[a].x - v.lo.x) as f32 / sx),
        );
        d.set_selection_high(
            selection.map_or(0.0, |(_, b)| (curve.points[b].x - v.lo.x) as f32 / sx),
        );
        let nodes = curve
            .points
            .iter()
            .enumerate()
            .map(|(i, p)| CurveNode {
                x: (p.x - v.lo.x) as f32 / sx,
                y: (p.y - v.lo.y) as f32 / sy,
                reference_y: baseline
                    .get(i)
                    .map_or((p.y - v.lo.y) as f32 / sy, |p| (p.y - v.lo.y) as f32 / sy),
                selected: curve
                    .selection()
                    .map_or(i == curve.selected, |(a, b)| (a..=b).contains(&i)),
            })
            .collect::<Vec<_>>();
        d.set_nodes(Rc::new(VecModel::from(nodes)).into());
        // Grid lines and labels sit at round data values, mapped exactly like
        // the points and the path; spacing follows the actual plot size.
        let (width, height) = plot_size(d.get_plot_width(), d.get_plot_height());
        d.set_x_ticks(Rc::new(VecModel::from(ticks(v.lo.x, v.hi.x, width, X_TICK_SPACING))).into());
        d.set_y_ticks(
            Rc::new(VecModel::from(ticks(
                v.lo.y,
                v.hi.y,
                height,
                Y_TICK_SPACING,
            )))
            .into(),
        );
        show_readout(&d, editor);
        d.set_can_undo(curve.can_undo());
        d.set_can_redo(curve.can_redo());
        let (xunit, yunit) = curve.kind.units();
        d.set_curve_x_label(xunit.into());
        d.set_curve_y_label(yunit.into());

        d.set_point_labels(strings(
            curve
                .points
                .iter()
                .enumerate()
                .map(|(i, p)| format!("{} · {} {xunit}", i + 1, p.x)),
        ));
        d.set_point_index(curve.selected as i32);
        if let Some(p) = curve.points.get(curve.selected) {
            d.set_point_x(p.x.to_string().into());
            d.set_point_y(p.y.to_string().into());
        }
    }
    pub(super) fn curve_action(&self, action: &str) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let d = w.global::<Workbench>();
        if action == "close" {
            if let Some(mut editor) = self.curve.borrow_mut().take() {
                editor.curve.cancel_gesture();
                if editor.curve.kind == Kind::Gpu {
                    *self.gpu_draft.borrow_mut() = Some(editor.curve);
                } else {
                    *self.fan_draft.borrow_mut() = Some(editor.curve);
                }
            }
            d.set_curve_open(false);
            if !d.get_curve_gpu() {
                self.sync_fan_library();
            }
            return;
        }
        if action == "save" {
            let value = self.curve.borrow().as_ref().map(|e| e.curve.value());
            match value {
                Some(Ok(Value::VfCurve(points))) => {
                    if let Some((index, _)) = preset_fields::fields()
                        .iter()
                        .enumerate()
                        .find(|(_, f)| f.feature == Feature::GpuVfCurve)
                    {
                        let mut rows = self.fields.borrow_mut();
                        rows[index].value = Value::VfCurve(points);
                        rows[index].included = true;
                    }
                    self.dirty();
                    self.update_fields();
                    self.curve_action("close");
                    self.status(t("ui.curve_in_draft"), false);
                }
                Some(Ok(Value::FanCurve(points))) => {
                    self.accept_fan_library_curve(points);
                    self.curve_action("close");
                }
                Some(Err(e)) => self.status(&crate::i18n::error_message(&e), true),
                _ => (),
            }
            return;
        }
        let result = (|| -> Result<(), String> {
            let mut current = self.curve.borrow_mut();
            let Some(e) = current.as_mut() else {
                return Ok(());
            };
            // A button during a drag completes the drag first (old UI).
            e.finish_gesture(false);
            match action {
                "undo" => {
                    e.curve.undo();
                }
                "redo" => {
                    e.curve.redo();
                }
                "reset" => {
                    e.curve.reset()?;
                }
                "flatten" => {
                    let mhz = d
                        .get_point_y()
                        .parse()
                        .map_err(|_| t("preset.co_steps_invalid"))?;
                    if e.curve.selection().is_some() {
                        e.curve.flatten_selection_to(mhz)?;
                    } else if e.selection_mode {
                        return Err(omen_core::message::msg("curve.err.selection_required", &[]));
                    } else {
                        e.curve.plateau(mhz)?;
                    }
                }
                "selection-mode" => {
                    e.selection_mode = !e.selection_mode;
                }
                "select-all" => {
                    e.curve.select_all();
                }
                "clear-selection" => {
                    e.curve.clear_selection();
                }
                "select-right" => {
                    let from = e.curve.points[e.curve.selected].x;
                    let to = e.curve.points.last().unwrap().x;
                    e.curve.select_voltage_range(from, to);
                    d.set_range_from(from.to_string().into());
                    d.set_range_to(to.to_string().into());
                }
                "select-range" => {
                    let from = d
                        .get_range_from()
                        .parse()
                        .map_err(|_| t("curve.invalid_range"))?;
                    let to = d
                        .get_range_to()
                        .parse()
                        .map_err(|_| t("curve.invalid_range"))?;
                    e.selection_mode = true;
                    if !e.curve.select_voltage_range(from, to) {
                        return Err(omen_core::message::msg("curve.err.empty_selection", &[]));
                    }
                }
                "insert" => e.curve.insert()?,
                "remove" => e.curve.remove()?,
                "fit" => e.view = Viewport::fit(&e.curve),
                "zoom-in" | "zoom-out" => {
                    let p = e.curve.points[e.curve.selected];
                    e.view.zoom(&e.curve, p, false, action == "zoom-in");
                }
                "set" => {
                    let p = Point {
                        x: d.get_point_x()
                            .parse()
                            .map_err(|_| t("preset.co_steps_invalid"))?,
                        y: d.get_point_y()
                            .parse()
                            .map_err(|_| t("preset.co_steps_invalid"))?,
                    };
                    e.curve.set(e.curve.selected, p, false)?;
                }
                _ => (),
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.status(&crate::i18n::error_message(&e), true);
        }
        self.draw_curve();
    }
    pub(super) fn chart_event(
        &self,
        kind: i32,
        x: f32,
        y: f32,
        plot: (f32, f32),
        modifiers: Modifiers,
    ) {
        let mut range = None;
        let result = (|| -> Result<(), String> {
            let mut current = self.curve.borrow_mut();
            let Some(e) = current.as_mut() else {
                return Ok(());
            };
            let point = e.view.data(x, y);
            match kind {
                0 => {
                    e.finish_gesture(true);
                    e.gesture = press(e, x, y, plot, modifiers);
                }
                1 | 2 => {
                    if let Some(gesture) = &e.gesture {
                        match gesture {
                            Gesture::Band { x: start } => {
                                e.curve.select_voltage_range(*start, point.x);
                                if kind == 2 {
                                    range = Some((*start.min(&point.x), *start.max(&point.x)));
                                }
                            }
                            Gesture::Point { start, down, mode } => {
                                if e.curve.kind == Kind::Gpu {
                                    e.curve.drag_gpu(start, *mode, point.y - down.y)?;
                                } else {
                                    let selected = e.curve.selected;
                                    let from = start[selected];
                                    e.curve.set(
                                        selected,
                                        Point {
                                            x: from.x + point.x - down.x,
                                            y: from.y + point.y - down.y,
                                        },
                                        true,
                                    )?;
                                }
                            }
                        }
                    }
                    if kind == 2 {
                        e.finish_gesture(false);
                        e.hover = hit(e, x, y, plot);
                    }
                }
                3 => e.finish_gesture(true),
                5 => {
                    // Afterburner/old UI: double-click clears the range; on a
                    // fan curve, free chart space gains a point there.
                    e.finish_gesture(true);
                    if e.curve.kind != Kind::Gpu && hit(e, x, y, plot).is_none() {
                        e.curve.insert_at(point)?;
                    } else {
                        e.curve.clear_selection();
                    }
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.status(&crate::i18n::error_message(&e), true);
        }
        if let (Some((from, to)), Some(w)) = (range, self.window.upgrade()) {
            let d = w.global::<Workbench>();
            d.set_range_from(from.to_string().into());
            d.set_range_to(to.to_string().into());
        }
        self.draw_curve();
    }
    /// Wheel: zoom X around the pointer, Ctrl zooms Y, Shift (or a sideways
    /// touchpad swipe) pans. One step per notch, not per wheel message.
    pub(super) fn chart_wheel(&self, dx: f32, dy: f32, x: f32, y: f32, shift: bool, ctrl: bool) {
        {
            let mut current = self.curve.borrow_mut();
            let Some(e) = current.as_mut() else {
                return;
            };
            if e.gesture.is_some() {
                return;
            }
            let sideways = dx.abs() > dy.abs();
            e.wheel += if sideways { dx } else { dy };
            let steps = (e.wheel / WHEEL_STEP).trunc();
            e.wheel -= steps * WHEEL_STEP;
            let at = e.view.data(x, y);
            for _ in 0..steps.abs() as u32 {
                let inward = steps > 0.0;
                if shift || sideways {
                    e.view.pan(&e.curve, if inward { -1 } else { 1 });
                } else {
                    e.view.zoom(&e.curve, at, ctrl, inward);
                }
            }
            if steps == 0.0 {
                return;
            }
        }
        self.draw_curve();
    }
    /// Pointer movement without a pressed button shows the values of the
    /// point within the pick radius; x < 0 means the pointer left the chart.
    pub(super) fn chart_hover(&self, x: f32, y: f32, plot: (f32, f32)) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let mut current = self.curve.borrow_mut();
        let Some(e) = current.as_mut() else {
            return;
        };
        if e.gesture.is_some() {
            return;
        }
        let hover = if x < 0.0 { None } else { hit(e, x, y, plot) };
        if hover != e.hover {
            e.hover = hover;
            show_readout(&w.global::<Workbench>(), e);
        }
    }
    pub(super) fn chart_key(&self, text: &str, shift: bool, ctrl: bool, alt: bool) -> bool {
        let Some(command) = decode_key(text, shift, ctrl) else {
            return false;
        };
        let modifiers = Modifiers { shift, ctrl, alt };
        let mut follow = None;
        let mut error = None;
        if let Some(e) = self.curve.borrow_mut().as_mut() {
            // During a drag only Escape counts: it cancels the drag.
            if e.gesture.is_some() {
                if matches!(command, Command::Edit(Key::Escape)) {
                    e.finish_gesture(true);
                }
            } else if let Err(err) = match command {
                Command::Edit(Key::Flatten)
                    if e.selection_mode && e.curve.selection().is_none() =>
                {
                    Err(omen_core::message::msg("curve.err.selection_required", &[]))
                }
                Command::Edit(key) => curve_input::edit(&mut e.curve, key, modifiers).map(|_| ()),
                Command::Revert => e.curve.revert().map(|_| ()),
                Command::Zoom { inward } => {
                    let at = e.curve.points[e.curve.selected];
                    e.view.zoom(&e.curve, at, ctrl, inward);
                    Ok(())
                }
                Command::Fit => {
                    e.view = Viewport::fit(&e.curve);
                    Ok(())
                }
                Command::EditValue | Command::Save | Command::Help => {
                    follow = Some(command);
                    Ok(())
                }
            } {
                error = Some(err);
            }
        }
        if let Some(err) = error {
            self.status(&crate::i18n::error_message(&err), true);
        }
        match follow {
            Some(Command::EditValue) => {
                if let Some(w) = self.window.upgrade() {
                    w.invoke_edit_curve_value();
                }
            }
            Some(Command::Save) => self.curve_action("save"),
            Some(Command::Help) => self.curve_help(),
            _ => self.draw_curve(),
        }
        true
    }
    fn curve_help(&self) {
        let Some(w) = self.window.upgrade() else {
            return;
        };
        let gpu = self
            .curve
            .borrow()
            .as_ref()
            .is_some_and(|e| e.curve.kind == Kind::Gpu);
        let d = w.global::<Workbench>();
        self.notice_timer.stop();
        d.set_notice_visible(false);
        d.set_notice_failed(false);
        d.set_notice_title(t("curve.help_title").into());
        d.set_notice_text(
            t(if gpu {
                "curve.help_draft"
            } else {
                "curve.help"
            })
            .replace("\r\n", "\n")
            .into(),
        );
        d.set_confirm_kind(5);
    }
}

/// The point whose values are shown beside it: the dragged point, otherwise
/// the point under the pointer, provided it is inside the visible range.
fn readout_point(e: &Editor) -> Option<usize> {
    let index = match &e.gesture {
        Some(Gesture::Point { .. }) => Some(e.curve.selected),
        Some(Gesture::Band { .. }) => None,
        None => e.hover,
    }?;
    let p = e.curve.points.get(index)?;
    let v = e.view;
    ((v.lo.x..=v.hi.x).contains(&p.x) && (v.lo.y..=v.hi.y).contains(&p.y)).then_some(index)
}
/// Anchor and value; GPU points add their offset from the driver default
/// curve, fan points the change since the drag started.
fn readout(e: &Editor, index: usize) -> String {
    let Some(p) = e.curve.points.get(index) else {
        return String::new();
    };
    let (xunit, yunit) = e.curve.kind.units();
    if let Some(base) = e.factory.as_ref().and_then(|f| f.get(index)) {
        return tf("curve.readout_driver", &[&p.x, &p.y, &signed(p.y - base.y)]);
    }
    match &e.gesture {
        Some(Gesture::Point { start, .. }) => match start.get(index) {
            Some(from) => format!(
                "{} {xunit} · {} {yunit} ({})",
                p.x,
                p.y,
                signed(p.y - from.y)
            ),
            None => format!("{} {xunit} · {} {yunit}", p.x, p.y),
        },
        _ => format!("{} {xunit} · {} {yunit}", p.x, p.y),
    }
}
fn show_readout(d: &Workbench<'_>, e: &Editor) {
    let Some(index) = readout_point(e) else {
        d.set_curve_readout("".into());
        return;
    };
    let p = e.curve.points[index];
    let v = e.view;
    d.set_readout_x((p.x - v.lo.x) as f32 / (v.hi.x - v.lo.x).max(1) as f32);
    d.set_readout_y((p.y - v.lo.y) as f32 / (v.hi.y - v.lo.y).max(1) as f32);
    d.set_readout_hover(e.gesture.is_none());
    d.set_curve_readout(readout(e, index).into());
}
/// Offset of the whole draft from the driver default curve, as one value or
/// as its range across all anchors.
fn driver_delta(e: &Editor) -> String {
    let Some(factory) = &e.factory else {
        return String::new();
    };
    let offsets = e.curve.points.iter().zip(factory).map(|(p, f)| p.y - f.y);
    let (Some(lo), Some(hi)) = (offsets.clone().min(), offsets.max()) else {
        return String::new();
    };
    if lo == 0 && hi == 0 {
        t("curve.delta_none").to_owned()
    } else if lo == hi {
        tf("curve.delta_uniform", &[&signed(lo)])
    } else {
        tf("curve.delta_range", &[&signed(lo), &signed(hi)])
    }
}
/// Signed MHz offset with a typographic minus, as elsewhere in the UI.
fn signed(value: i32) -> String {
    if value < 0 {
        format!("−{}", value.unsigned_abs())
    } else {
        format!("+{value}")
    }
}
/// Minimum on-screen distance between labelled grid lines, in logical px.
const X_TICK_SPACING: i32 = 64;
const Y_TICK_SPACING: i32 = 24;
/// Plot size reported by the canvas; a nominal size until it is laid out.
fn plot_size(width: f32, height: f32) -> (f32, f32) {
    if width >= 1.0 && height >= 1.0 {
        (width, height)
    } else {
        (600.0, 300.0)
    }
}
/// Grid ticks at multiples of a readable step inside `lo..=hi`, positioned
/// with the same `(value - lo) / span` mapping as points and path.
fn ticks(lo: i32, hi: i32, pixels: f32, spacing: i32) -> Vec<AxisTick> {
    let span = (hi - lo).max(1);
    let step = omen_core::curve_view::tick_step(span, pixels.round() as i32, spacing).max(1) as i32;
    let first = lo.div_euclid(step) * step + if lo.rem_euclid(step) == 0 { 0 } else { step };
    (0..)
        .map(|i| first + i * step)
        .take_while(|value| *value <= hi)
        .map(|value| AxisTick {
            position: (value - lo) as f32 / span as f32,
            label: value.to_string().into(),
        })
        .collect()
}
/// Slint reports a classic wheel notch as 120 logical pixels on Windows;
/// touchpads deliver smaller deltas that accumulate to the same step.
const WHEEL_STEP: f32 = 120.0;
/// Pick radius around a visible point, in logical pixels (old native UI).
const HIT_RADIUS: f32 = 15.0;

pub(super) enum Gesture {
    /// Dragging a point; a selected band moves with it as one group.
    Point {
        start: Vec<Point>,
        down: Point,
        mode: DragMode,
    },
    /// Shift + drag on free GPU chart space (or range mode) selects anchors.
    Band { x: i32 },
}
impl Editor {
    pub(super) fn new(curve: Curve) -> Self {
        Self {
            view: Viewport::initial(&curve),
            factory: curve.factory().and_then(Result::ok),
            curve,
            gesture: None,
            selection_mode: false,
            wheel: 0.0,
            hover: None,
        }
    }
    /// Offsets and the reference line compare against the driver default
    /// curve; without one (fan curves) against the curve loaded at opening.
    fn baseline(&self) -> &[Point] {
        self.factory.as_deref().unwrap_or(self.curve.reference())
    }
    /// Ends a drag: one undo step, or none when cancelled.
    fn finish_gesture(&mut self, cancel: bool) {
        if self.gesture.take().is_some() {
            if cancel {
                self.curve.cancel_gesture();
            } else {
                self.curve.end_gesture();
            }
        }
    }
}
trait ViewData {
    fn data(&self, x: f32, y: f32) -> Point;
}
impl ViewData for Viewport {
    fn data(&self, x: f32, y: f32) -> Point {
        Point {
            x: (self.lo.x as f32 + x.clamp(0., 1.) * (self.hi.x - self.lo.x) as f32).round() as i32,
            y: (self.hi.y as f32 - y.clamp(0., 1.) * (self.hi.y - self.lo.y) as f32).round() as i32,
        }
    }
}
/// The visible point within the pick radius of the pointer (x/y in 0..1).
fn hit(e: &Editor, x: f32, y: f32, (w, h): (f32, f32)) -> Option<usize> {
    let v = e.view;
    let sx = (v.hi.x - v.lo.x).max(1) as f32;
    let sy = (v.hi.y - v.lo.y).max(1) as f32;
    e.curve
        .points
        .iter()
        .enumerate()
        .filter(|(_, p)| (v.lo.x..=v.hi.x).contains(&p.x) && (v.lo.y..=v.hi.y).contains(&p.y))
        .map(|(i, p)| {
            let px = (p.x - v.lo.x) as f32 / sx * w;
            let py = (1.0 - (p.y - v.lo.y) as f32 / sy) * h;
            (i, (px - x * w).powi(2) + (py - y * h).powi(2))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .filter(|(_, d)| *d <= HIT_RADIUS * HIT_RADIUS)
        .map(|(i, _)| i)
}
/// Mouse down, as in the old native editor: a point starts a drag (Ctrl tilt,
/// Shift reference offset, Alt current offset); a point inside the selected
/// band keeps the band so it moves as a group; Shift on free GPU space or
/// range mode starts a band; a plain click on free space clears the band.
fn press(e: &mut Editor, x: f32, y: f32, plot: (f32, f32), m: Modifiers) -> Option<Gesture> {
    let gpu = e.curve.kind == Kind::Gpu;
    let point = e.view.data(x, y);
    if gpu && e.selection_mode {
        e.curve.select_voltage_range(point.x, point.x);
        return Some(Gesture::Band { x: point.x });
    }
    if let Some(index) = hit(e, x, y, plot) {
        if e.curve
            .selection()
            .is_some_and(|(a, b)| (a..=b).contains(&index))
        {
            e.curve.selected = index;
        } else {
            e.curve.select(index, false);
        }
        e.curve.begin_gesture();
        return Some(Gesture::Point {
            start: e.curve.points.clone(),
            down: point,
            mode: if m.ctrl {
                DragMode::Tilt
            } else if m.shift {
                DragMode::ReferenceOffset
            } else if m.alt {
                DragMode::CurrentOffset
            } else {
                DragMode::Point
            },
        });
    }
    if gpu && m.shift {
        e.curve.select_voltage_range(point.x, point.x);
        return Some(Gesture::Band { x: point.x });
    }
    e.curve.clear_selection();
    None
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Command {
    Edit(Key),
    Revert,
    Zoom { inward: bool },
    Fit,
    EditValue,
    Save,
    Help,
}
/// Slint key text to editor commands (docs/CURVE_EDITOR.md).
fn decode_key(text: &str, shift: bool, ctrl: bool) -> Option<Command> {
    Some(match text {
        "\u{f702}" => Command::Edit(Key::Left),
        "\u{f703}" => Command::Edit(Key::Right),
        "\u{f700}" => Command::Edit(Key::Up),
        "\u{f701}" => Command::Edit(Key::Down),
        "\u{f729}" => Command::Edit(Key::Home),
        "\u{f72b}" => Command::Edit(Key::End),
        "\u{1b}" => Command::Edit(Key::Escape),
        "\u{f727}" => Command::Edit(Key::Insert),
        "\u{7f}" => Command::Edit(Key::Delete),
        "\u{f704}" => Command::Help,
        "\n" | "\r" if shift => Command::Edit(Key::Flatten),
        "\n" | "\r" => Command::EditValue,
        "z" | "Z" if ctrl => Command::Edit(if shift { Key::Redo } else { Key::Undo }),
        "y" | "Y" if ctrl => Command::Edit(Key::Redo),
        "a" | "A" if ctrl => Command::Edit(Key::All),
        "d" | "D" if ctrl => Command::Revert,
        "s" | "S" if ctrl => Command::Save,
        "0" if ctrl => Command::Fit,
        "+" | "=" => Command::Zoom { inward: true },
        "-" => Command::Zoom { inward: false },
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checking_an_empty_curve_field_opens_factory_without_replacing_saved_targets() {
        let state = omen_core::vf::VfState {
            nodes: [(1100, 100), (1300, 100), (1400, 0), (1400, -200)]
                .iter()
                .enumerate()
                .map(|(i, &(mhz, delta))| omen_core::vf::VfNode {
                    index: i as u16,
                    voltage_uv: 800000 + i as u32 * 100000,
                    frequency_khz: mhz * 1000,
                    delta_khz: delta * 1000,
                })
                .collect(),
            minimum_delta_khz: -400000,
            maximum_delta_khz: 400000,
            first_index: 0,
            last_index: 3,
        };
        let empty = Value::VfCurve(vec![]);
        let editor = gpu_editor(state.clone(), Some(&empty)).unwrap();
        assert_eq!(
            editor.points.iter().map(|p| p.y).collect::<Vec<_>>(),
            [1000, 1200, 1400, 1600]
        );
        assert!(editor.value().is_ok());
        // This presentation fallback never makes an empty Apply target valid.
        assert!(
            omen_core::profiles::Setting {
                feature: Feature::GpuVfCurve,
                value: empty
            }
            .validate()
            .is_err()
        );
        let saved = state.value().unwrap();
        assert_eq!(
            gpu_editor(state.clone(), Some(&saved))
                .unwrap()
                .value()
                .unwrap(),
            saved
        );
        let partial = Value::VfCurve(vec![omen_core::profiles::VfPoint {
            voltage_mv: 800,
            frequency_mhz: 1100,
        }]);
        assert!(gpu_editor(state, Some(&partial)).is_err());
    }
    fn editor() -> Editor {
        let state = omen_core::vf::VfState {
            nodes: (0..6)
                .map(|i| omen_core::vf::VfNode {
                    index: i,
                    voltage_uv: 800_000 + u32::from(i) * 50_000,
                    frequency_khz: (1000 + u32::from(i) * 100) * 1000,
                    delta_khz: 0,
                })
                .collect(),
            minimum_delta_khz: -500_000,
            maximum_delta_khz: 500_000,
            first_index: 0,
            last_index: 5,
        };
        let mut e = Editor::new(gpu_editor(state, None).unwrap());
        e.view = Viewport::fit(&e.curve);
        e
    }
    /// Normalized position of point `i` in the editor's viewport.
    fn at(e: &Editor, i: usize) -> (f32, f32) {
        let v = e.view;
        let p = e.curve.points[i];
        (
            (p.x - v.lo.x) as f32 / (v.hi.x - v.lo.x) as f32,
            1.0 - (p.y - v.lo.y) as f32 / (v.hi.y - v.lo.y) as f32,
        )
    }
    const PLOT: (f32, f32) = (600.0, 300.0);
    fn drag(e: &mut Editor, from: (f32, f32), to: (f32, f32), m: Modifiers) {
        e.gesture = press(e, from.0, from.1, PLOT, m);
        let point = e.view.data(to.0, to.1);
        match &e.gesture {
            Some(Gesture::Point { start, down, mode }) => {
                let (start, down, mode) = (start.clone(), *down, *mode);
                let _ = e.curve.drag_gpu(&start, mode, point.y - down.y);
            }
            Some(Gesture::Band { x }) => {
                let x = *x;
                e.curve.select_voltage_range(x, point.x);
            }
            None => {}
        }
        e.finish_gesture(false);
    }

    #[test]
    fn points_are_picked_within_the_radius_only() {
        let e = editor();
        let (x, y) = at(&e, 2);
        assert_eq!(hit(&e, x, y, PLOT), Some(2));
        assert_eq!(hit(&e, x, y + 10.0 / PLOT.1, PLOT), Some(2));
        assert_eq!(hit(&e, x, y + 40.0 / PLOT.1, PLOT), None);
    }

    #[test]
    fn a_selected_band_moves_as_one_group_in_one_undo_step() {
        let mut e = editor();
        e.curve.select_voltage_range(850, 950);
        let before = e.curve.points.clone();
        let (x, y) = at(&e, 2);
        drag(&mut e, (x, y), (x, y - 0.1), Modifiers::default());
        assert_eq!(e.curve.selection(), Some((1, 3)));
        let moved: Vec<i32> = (0..6).map(|i| e.curve.points[i].y - before[i].y).collect();
        assert!(moved[1] > 0 && moved[1] == moved[2] && moved[2] == moved[3]);
        assert_eq!((moved[0], moved[4], moved[5]), (0, 0, 0));
        assert!(e.curve.points.iter().zip(&before).all(|(a, b)| a.x == b.x));
        assert!(e.curve.undo());
        assert_eq!(e.curve.points, before);
        assert!(!e.curve.can_undo());
    }

    #[test]
    fn shift_drag_on_free_space_selects_and_a_plain_click_clears() {
        let mut e = editor();
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        // Free space well below the curve.
        let (x1, x4) = (at(&e, 1).0, at(&e, 4).0);
        drag(&mut e, (x1, 0.98), (x4, 0.98), shift);
        assert_eq!(e.curve.selection(), Some((1, 4)));
        assert!(!e.curve.can_undo(), "Selecting creates no history");
        assert!(press(&mut e, x1, 0.98, PLOT, Modifiers::default()).is_none());
        assert_eq!(e.curve.selection(), None);
    }

    #[test]
    fn escape_during_a_drag_restores_without_history() {
        let mut e = editor();
        let before = e.curve.points.clone();
        let (x, y) = at(&e, 3);
        e.gesture = press(&mut e, x, y, PLOT, Modifiers::default());
        let Some(Gesture::Point { start, down, mode }) = &e.gesture else {
            panic!("point drag expected")
        };
        let (start, down, mode) = (start.clone(), *down, *mode);
        e.curve
            .drag_gpu(&start, mode, 40 - down.y + down.y)
            .unwrap();
        assert_ne!(e.curve.points, before);
        e.finish_gesture(true);
        assert_eq!(e.curve.points, before);
        assert!(!e.curve.can_undo());
        assert!(e.gesture.is_none());
    }

    #[test]
    fn documented_keys_decode_to_editor_commands() {
        assert_eq!(
            decode_key("\n", true, false),
            Some(Command::Edit(Key::Flatten))
        );
        assert_eq!(decode_key("\n", false, false), Some(Command::EditValue));
        assert_eq!(decode_key("d", false, true), Some(Command::Revert));
        assert_eq!(decode_key("s", false, true), Some(Command::Save));
        assert_eq!(decode_key("0", false, true), Some(Command::Fit));
        assert_eq!(
            decode_key("+", false, false),
            Some(Command::Zoom { inward: true })
        );
        assert_eq!(
            decode_key("-", false, true),
            Some(Command::Zoom { inward: false })
        );
        assert_eq!(decode_key("\u{f704}", false, false), Some(Command::Help));
        assert_eq!(decode_key("Z", true, true), Some(Command::Edit(Key::Redo)));
        // Afterburner's L is a voltage lock; it is not silently a flatten here.
        assert_eq!(decode_key("l", false, false), None);
        assert_eq!(decode_key("0", false, false), None);
    }

    #[test]
    fn grid_ticks_sit_on_round_values_with_the_point_mapping() {
        let (lo, hi) = (437, 1162);
        let ticks = ticks(lo, hi, 1000.0, X_TICK_SPACING);
        let values: Vec<i32> = ticks.iter().map(|t| t.label.parse().unwrap()).collect();
        assert_eq!(values.first(), Some(&450));
        assert_eq!(values.last(), Some(&1150));
        assert!(values.windows(2).all(|w| w[1] - w[0] == 50));
        for (tick, value) in ticks.iter().zip(&values) {
            let expected = (value - lo) as f32 / (hi - lo) as f32;
            assert!((tick.position - expected).abs() < 1e-6);
        }
        // A tick exactly on the lower bound is kept; spacing never shrinks
        // below the requested on-screen distance.
        let y = super::ticks(1000, 2000, 300.0, Y_TICK_SPACING);
        assert_eq!(y.first().map(|t| t.position), Some(0.0));
        assert!(y.len() <= 300 / Y_TICK_SPACING as usize + 1);
        assert_eq!(plot_size(0.0, 0.0), (600.0, 300.0));
        assert_eq!(
            (signed(-870), signed(0), signed(25)),
            ("−870".into(), "+0".into(), "+25".into())
        );
    }

    #[test]
    fn offsets_are_shown_against_the_driver_default_not_the_loaded_preset() {
        let state = omen_core::vf::VfState {
            nodes: (0..4)
                .map(|i| omen_core::vf::VfNode {
                    index: i,
                    voltage_uv: 800_000 + u32::from(i) * 50_000,
                    // The live curve carries an applied +100 MHz offset.
                    frequency_khz: (1100 + u32::from(i) * 100) * 1000,
                    delta_khz: 100_000,
                })
                .collect(),
            minimum_delta_khz: -500_000,
            maximum_delta_khz: 500_000,
            first_index: 0,
            last_index: 3,
        };
        let loaded = state.value().unwrap();
        let mut e = Editor::new(gpu_editor(state, Some(&loaded)).unwrap());
        e.view = Viewport::fit(&e.curve);
        assert_eq!(e.baseline()[0].y, 1000);
        assert_eq!(driver_delta(&e), tf("curve.delta_uniform", &[&"+100"]));
        e.curve.points[3].y += 20;
        assert_eq!(
            driver_delta(&e),
            tf("curve.delta_range", &[&"+100", &"+120"])
        );
        assert_eq!(
            readout(&e, 3),
            tf("curve.readout_driver", &[&950, &1420, &"+120"])
        );
        assert!(e.curve.reset().unwrap());
        assert_eq!(driver_delta(&e), t("curve.delta_none"));
    }

    #[test]
    fn hovering_a_point_shows_its_values_without_a_drag() {
        let mut e = editor();
        assert_eq!(readout_point(&e), None);
        let (x, y) = at(&e, 2);
        e.hover = hit(&e, x, y, PLOT);
        assert_eq!(readout_point(&e), Some(2));
        assert_eq!(
            readout(&e, 2),
            tf("curve.readout_driver", &[&900, &1200, &"+0"])
        );
        // Free space shows nothing; a drag shows the dragged point instead.
        e.hover = hit(&e, x, 0.98, PLOT);
        assert_eq!(readout_point(&e), None);
        let (x4, y4) = at(&e, 4);
        e.gesture = press(&mut e, x4, y4, PLOT, Modifiers::default());
        assert_eq!(readout_point(&e), Some(4));
        // A point outside the zoomed view has no readout.
        e.finish_gesture(true);
        e.hover = Some(0);
        e.view.lo.x = 900;
        assert_eq!(readout_point(&e), None);
    }

    #[test]
    fn connected_curve_uses_the_same_viewport_as_markers_and_input() {
        let view = Viewport {
            lo: Point { x: 40, y: 0 },
            hi: Point { x: 100, y: 100 },
        };
        let points = [
            Point { x: 40, y: 20 },
            Point { x: 70, y: 60 },
            Point { x: 100, y: 100 },
        ];
        assert_eq!(
            path(&points, view),
            "M 0.000 800.000 L 500.000 400.000 L 1000.000 0.000 "
        );
        assert_eq!(points[1], Point { x: 70, y: 60 });
        // Zoomed-out points retain their coordinates for plot clipping, rather
        // than being clamped into a misleading horizontal or vertical segment.
        assert!(path(&[Point { x: 10, y: 120 }], view).starts_with("M -500.000 -200.000"));
    }
}
