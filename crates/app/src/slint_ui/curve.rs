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
    gesture: Option<(Vec<Point>, Point, Modifiers)>,
    selection_mode: bool,
    range_gesture: Option<Point>,
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
        let view = Viewport::initial(&curve);
        *self.curve.borrow_mut() = Some(Editor {
            curve,
            view,
            gesture: None,
            selection_mode: false,
            range_gesture: None,
        });
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
        d.set_curve_path(path(&curve.points, v).into());
        d.set_reference_path(path(curve.reference(), v).into());
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
                reference_y: curve
                    .reference()
                    .get(i)
                    .map_or((p.y - v.lo.y) as f32 / sy, |p| (p.y - v.lo.y) as f32 / sy),
                selected: curve
                    .selection()
                    .map_or(i == curve.selected, |(a, b)| (a..=b).contains(&i)),
            })
            .collect::<Vec<_>>();
        d.set_nodes(Rc::new(VecModel::from(nodes)).into());
        d.set_y_ticks(
            Rc::new(VecModel::from(
                (0..6)
                    .map(|i| AxisTick {
                        position: i as f32 / 5.0,
                        label: format!("{}", v.lo.y + (v.hi.y - v.lo.y) * i / 5).into(),
                    })
                    .collect::<Vec<_>>(),
            ))
            .into(),
        );
        d.set_can_undo(curve.can_undo());
        d.set_can_redo(curve.can_redo());
        let (xunit, yunit) = curve.kind.units();
        d.set_curve_x_label(xunit.into());
        d.set_curve_y_label(yunit.into());
        d.set_curve_low(format!("{}", v.lo.x).into());
        d.set_curve_high(format!("{}", v.hi.x).into());
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
                    e.range_gesture = None;
                }
                "select-all" => {
                    e.curve.select_all();
                }
                "clear-selection" => {
                    e.curve.clear_selection();
                    e.range_gesture = None;
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
        shift: bool,
        ctrl: bool,
        alt: bool,
    ) {
        let result = (|| -> Result<(), String> {
            let mut current = self.curve.borrow_mut();
            let Some(e) = current.as_mut() else {
                return Ok(());
            };
            let point = Point {
                x: (e.view.lo.x as f32 + x.clamp(0., 1.) * (e.view.hi.x - e.view.lo.x) as f32)
                    .round() as i32,
                y: (e.view.hi.y as f32 - y.clamp(0., 1.) * (e.view.hi.y - e.view.lo.y) as f32)
                    .round() as i32,
            };
            if kind == 0 {
                if e.curve.kind == Kind::Gpu && e.selection_mode {
                    e.range_gesture = Some(point);
                    e.curve.select_voltage_range(point.x, point.x);
                } else {
                    let index = e
                        .curve
                        .points
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, p)| (p.x - point.x).abs())
                        .map_or(0, |(i, _)| i);
                    e.curve.select(index, ctrl);
                    e.curve.begin_gesture();
                    e.gesture = Some((
                        e.curve.points.clone(),
                        point,
                        Modifiers { shift, ctrl, alt },
                    ));
                }
            } else if kind == 1 || kind == 2 {
                if let Some(start) = e.range_gesture {
                    e.curve.select_voltage_range(start.x, point.x);
                    if kind == 2
                        && let Some(w) = self.window.upgrade()
                    {
                        let d = w.global::<Workbench>();
                        d.set_range_from(start.x.min(point.x).to_string().into());
                        d.set_range_to(start.x.max(point.x).to_string().into());
                    }
                } else if let Some((original, start, m)) = &e.gesture {
                    if e.curve.kind == Kind::Gpu && (m.shift || m.alt || m.ctrl) {
                        let mode = if m.ctrl {
                            DragMode::Tilt
                        } else if m.shift {
                            DragMode::ReferenceOffset
                        } else {
                            DragMode::CurrentOffset
                        };
                        e.curve.drag_gpu(original, mode, point.y - start.y)?;
                    } else {
                        let selected = e.curve.selected;
                        let original = original[selected];
                        e.curve.set(
                            selected,
                            Point {
                                x: if e.curve.kind == Kind::Gpu {
                                    original.x
                                } else {
                                    original.x + point.x - start.x
                                },
                                y: original.y + point.y - start.y,
                            },
                            true,
                        )?;
                    }
                }
                if kind == 2 {
                    e.curve.end_gesture();
                    e.gesture = None;
                    e.range_gesture = None;
                }
            } else if kind == 3 {
                e.curve.cancel_gesture();
                e.gesture = None;
                e.range_gesture = None;
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.status(&crate::i18n::error_message(&e), true);
        }
        self.draw_curve();
    }
    pub(super) fn chart_key(&self, text: &str, shift: bool, ctrl: bool, alt: bool) -> bool {
        let key = match text {
            "\u{f702}" => Some(Key::Left),
            "\u{f703}" => Some(Key::Right),
            "\u{f700}" => Some(Key::Up),
            "\u{f701}" => Some(Key::Down),
            "\u{f729}" => Some(Key::Home),
            "\u{f72b}" => Some(Key::End),
            "\u{1b}" => Some(Key::Escape),
            "z" | "Z" if ctrl => Some(if shift { Key::Redo } else { Key::Undo }),
            "y" | "Y" if ctrl => Some(Key::Redo),
            "a" | "A" if ctrl => Some(Key::All),
            "l" | "L" => Some(Key::Flatten),
            _ => None,
        };
        if let Some(key) = key {
            if let Some(e) = self.curve.borrow_mut().as_mut() {
                if matches!(key, Key::Escape) {
                    e.range_gesture = None;
                }
                let result = if matches!(key, Key::Flatten)
                    && e.selection_mode
                    && e.curve.selection().is_none()
                {
                    Err(omen_core::message::msg("curve.err.selection_required", &[]))
                } else {
                    curve_input::edit(&mut e.curve, key, Modifiers { shift, ctrl, alt })
                };
                if let Err(err) = result {
                    self.status(&crate::i18n::error_message(&err), true);
                }
            }
            self.draw_curve();
            true
        } else {
            false
        }
    }
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
