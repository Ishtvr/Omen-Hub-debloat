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

pub(super) struct Editor {
    pub curve: Curve,
    view: Viewport,
    gesture: Option<(Vec<Point>, Point, Modifiers)>,
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
        let baseline = self.fan_baseline.borrow();
        let changed = |curve: &Curve| curve.value().ok() != *baseline;
        self.gpu_curve_dirty()
            || self.fan_draft.borrow().as_ref().is_some_and(changed)
            || self
                .curve
                .borrow()
                .as_ref()
                .is_some_and(|e| e.curve.kind != Kind::Gpu && changed(&e.curve))
    }
    pub(super) fn open_curve(&self, gpu: bool) {
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
                .and_then(|state| Curve::gpu(state, value.as_ref()))
        } else if self.test {
            Curve::fan(Kind::CpuFan, None)
        } else {
            self.send_aux(Aux::ReadFan);
            return;
        };
        match result {
            Ok(curve) => self.open_loaded_curve(curve),
            Err(e) => self.status(&crate::i18n::error_message(&e), true),
        }
    }
    pub(super) fn open_loaded_curve(&self, curve: Curve) {
        let gpu = curve.kind == Kind::Gpu;
        if !gpu && self.fan_baseline.borrow().is_none() {
            *self.fan_baseline.borrow_mut() = curve.value().ok();
        }
        let view = Viewport::initial(&curve);
        *self.curve.borrow_mut() = Some(Editor {
            curve,
            view,
            gesture: None,
        });
        if let Some(w) = self.window.upgrade() {
            let data = w.global::<Workbench>();
            data.set_curve_open(true);
            data.set_curve_gpu(gpu);
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
        let sx = (v.hi.x - v.lo.x).max(1) as f32;
        let sy = (v.hi.y - v.lo.y).max(1) as f32;
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
                    if self.test {
                        self.status(t("ui.test_no_write"), true);
                    } else {
                        self.send_aux(Aux::SaveFan(points));
                    }
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
                    e.curve.flatten_selection()?;
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
            } else if kind == 1 || kind == 2 {
                if let Some((original, start, m)) = &e.gesture {
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
                }
            } else if kind == 3 {
                e.curve.cancel_gesture();
                e.gesture = None;
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
            if let Some(e) = self.curve.borrow_mut().as_mut()
                && let Err(err) =
                    curve_input::edit(&mut e.curve, key, Modifiers { shift, ctrl, alt })
            {
                self.status(&crate::i18n::error_message(&err), true);
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
