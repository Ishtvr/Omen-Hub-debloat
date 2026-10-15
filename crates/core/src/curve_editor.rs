//! Pure curve editing. No OS access, sensor claims or hardware writes.
use crate::{
    features::Feature,
    message::msg,
    profiles::{FanPoint, Setting, Value, VfPoint},
    vf::VfState,
};
use std::collections::VecDeque;

/// Editing gestures affect a draft, never the driver's voltage anchors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragMode {
    Point,
    ReferenceOffset,
    CurrentOffset,
    Tilt,
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Snapshot {
    points: Vec<Point>,
    selected: usize,
    selection: Option<(usize, usize)>,
}
const HISTORY_LIMIT: usize = 64;
/// Fan curves accept up to one point per 2 °C over 0…110 °C.
pub const MAX_FAN_POINTS: usize = 56;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Gpu,
    CpuFan,
    GpuFan,
}
impl Kind {
    pub fn feature(self) -> Feature {
        match self {
            Self::Gpu => Feature::GpuVfCurve,
            Self::CpuFan => Feature::CpuFanCurve,
            Self::GpuFan => Feature::GpuFanCurve,
        }
    }
    pub fn units(self) -> (&'static str, &'static str) {
        if self == Self::Gpu {
            ("mV", "MHz")
        } else {
            ("°C", "%")
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
#[derive(Debug, Clone)]
pub struct Curve {
    pub kind: Kind,
    pub points: Vec<Point>,
    pub selected: usize,
    original: Vec<Point>,
    gpu: Option<VfState>,
    selection: Option<(usize, usize)>,
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    gesture: Option<Snapshot>,
}
impl Curve {
    pub fn gpu(state: VfState, value: Option<&Value>) -> Result<Self, String> {
        let value = value.cloned().map_or_else(|| state.value(), Ok)?;
        let Value::VfCurve(points) = value else {
            return Err(msg("curve.err.incomplete_gpu", &[]));
        };
        state.target_deltas(&points)?;
        let points = points
            .iter()
            .map(|p| {
                Ok(Point {
                    x: i32::try_from(p.voltage_mv)
                        .map_err(|_| msg("curve.err.anchor_too_large", &[]))?,
                    y: i32::try_from(p.frequency_mhz)
                        .map_err(|_| msg("curve.err.frequency_too_large", &[]))?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            kind: Kind::Gpu,
            original: points.clone(),
            selected: points
                .iter()
                .enumerate()
                .min_by_key(|(_, p)| (p.x - 950).abs())
                .map_or(0, |(i, _)| i),
            points,
            gpu: Some(state),
            selection: None,
            undo: VecDeque::new(),
            redo: Vec::new(),
            gesture: None,
        })
    }
    pub fn fan(kind: Kind, value: Option<&Value>) -> Result<Self, String> {
        if kind == Kind::Gpu {
            return Err(msg("curve.err.fan_expected", &[]));
        }
        let points = match value {
            Some(Value::FanCurve(points)) => points
                .iter()
                .map(|p| Point {
                    x: i32::from(p.temperature_c),
                    y: i32::from(p.duty_percent),
                })
                .collect(),
            None => vec![
                Point { x: 30, y: 30 },
                Point { x: 55, y: 45 },
                Point { x: 75, y: 70 },
                Point { x: 90, y: 100 },
            ],
            _ => return Err(msg("curve.err.fan_type", &[])),
        };
        let curve = Self {
            kind,
            original: points.clone(),
            points,
            selected: 0,
            gpu: None,
            selection: None,
            undo: VecDeque::new(),
            redo: Vec::new(),
            gesture: None,
        };
        curve.value()?;
        Ok(curve)
    }
    pub fn value(&self) -> Result<Value, String> {
        let value = if self.kind == Kind::Gpu {
            Value::VfCurve(
                self.points
                    .iter()
                    .map(|p| {
                        Ok(VfPoint {
                            voltage_mv: u32::try_from(p.x)
                                .map_err(|_| msg("curve.err.invalid_anchor", &[]))?,
                            frequency_mhz: u32::try_from(p.y)
                                .map_err(|_| msg("curve.err.invalid_frequency", &[]))?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            )
        } else {
            if self.points.iter().any(|p| !(0..=110).contains(&p.x)) {
                return Err(msg("curve.err.temperature_range", &[]));
            }
            Value::FanCurve(
                self.points
                    .iter()
                    .map(|p| {
                        Ok(FanPoint {
                            temperature_c: u8::try_from(p.x)
                                .map_err(|_| msg("curve.err.invalid_temperature", &[]))?,
                            duty_percent: u8::try_from(p.y)
                                .map_err(|_| msg("curve.err.invalid_fan", &[]))?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            )
        };
        Setting {
            feature: self.kind.feature(),
            value: value.clone(),
        }
        .validate()?;
        if let (Some(gpu), Value::VfCurve(points)) = (&self.gpu, &value) {
            gpu.target_deltas(points)?;
        }
        Ok(value)
    }
    pub fn limits(&self, index: usize) -> Result<(Point, Point), String> {
        let p = *self
            .points
            .get(index)
            .ok_or_else(|| msg("curve.err.no_point", &[]))?;
        let mut lo = Point { x: 0, y: 0 };
        let mut hi = Point { x: 110, y: 100 };
        if let Some(gpu) = &self.gpu {
            let node = &gpu.nodes[index];
            let base = i64::from(node.frequency_khz) - i64::from(node.delta_khz);
            // Ceil the minimum, floor the maximum; never round across a bound.
            let min = (base + i64::from(gpu.minimum_delta_khz) + 999)
                .div_euclid(1000)
                .max(1);
            let max = (base + i64::from(gpu.maximum_delta_khz)).div_euclid(1000);
            lo = Point {
                x: p.x,
                y: i32::try_from(min).map_err(|_| msg("curve.err.gpu_lower", &[]))?,
            };
            hi = Point {
                x: p.x,
                y: i32::try_from(max).map_err(|_| msg("curve.err.gpu_upper", &[]))?,
            };
        }
        if let Some(previous) = index.checked_sub(1).and_then(|i| self.points.get(i)) {
            lo.y = lo.y.max(previous.y);
            if self.kind != Kind::Gpu {
                lo.x = previous.x + 1;
            }
        }
        if let Some(next) = self.points.get(index + 1) {
            hi.y = hi.y.min(next.y);
            if self.kind != Kind::Gpu {
                hi.x = next.x - 1;
            }
        }
        if lo.x > hi.x || lo.y > hi.y {
            return Err(msg("curve.err.neighbours", &[]));
        }
        Ok((lo, hi))
    }
    pub fn set(&mut self, index: usize, p: Point, clamp: bool) -> Result<bool, String> {
        let (lo, hi) = self.limits(index)?;
        if self.kind == Kind::Gpu && p.x != lo.x {
            return Err(msg("curve.err.anchors_fixed", &[]));
        }
        let p = if clamp {
            Point {
                x: p.x.clamp(lo.x, hi.x),
                y: p.y.clamp(lo.y, hi.y),
            }
        } else {
            if p.x < lo.x || p.x > hi.x || p.y < lo.y || p.y > hi.y {
                return Err(msg(
                    "curve.err.out_of_bounds",
                    &[&lo.y, &hi.y, &self.kind.units().1],
                ));
            }
            p
        };
        let mut points = self.points.clone();
        points[index] = p;
        self.replace(points)
    }
    pub fn plateau(&mut self, frequency: i32) -> Result<bool, String> {
        if self.kind != Kind::Gpu {
            return Err(msg("curve.err.plateau_gpu", &[]));
        }
        if self.selected >= self.points.len() {
            return Err(msg("curve.err.no_point", &[]));
        }
        let mut points = self.points.clone();
        for p in &mut points[self.selected..] {
            p.y = frequency;
        }
        self.replace(points)
    }
    pub fn insert(&mut self) -> Result<(), String> {
        if self.kind == Kind::Gpu {
            return Err(msg("curve.err.anchors_driver", &[]));
        }
        if self.points.len() >= MAX_FAN_POINTS {
            return Err(msg("curve.err.max_points", &[&MAX_FAN_POINTS]));
        }
        let a = *self
            .points
            .get(self.selected)
            .ok_or_else(|| msg("curve.err.no_point", &[]))?;
        let point = if let Some(b) = self.points.get(self.selected + 1) {
            if b.x - a.x < 2 {
                return Err(msg("curve.err.no_room", &[]));
            }
            Point {
                x: (a.x + b.x) / 2,
                y: (a.y + b.y + 1) / 2,
            }
        } else {
            if a.x >= 110 {
                return Err(msg("curve.err.right_edge", &[]));
            }
            Point {
                x: (a.x + 10).min(110),
                y: a.y,
            }
        };
        let mut points = self.points.clone();
        let selected = self.selected + 1;
        points.insert(selected, point);
        self.replace(points)?;
        self.select(selected, false);
        Ok(())
    }
    /// Adds a fan point at a chart position. The duty is clamped between its
    /// neighbours so the curve stays non-decreasing.
    pub fn insert_at(&mut self, at: Point) -> Result<(), String> {
        if self.kind == Kind::Gpu {
            return Err(msg("curve.err.anchors_driver", &[]));
        }
        if self.points.len() >= MAX_FAN_POINTS {
            return Err(msg("curve.err.max_points", &[&MAX_FAN_POINTS]));
        }
        let x = at.x.clamp(0, 110);
        if self.points.iter().any(|p| p.x == x) {
            return Err(msg("curve.err.duplicate", &[]));
        }
        let index = self
            .points
            .iter()
            .position(|p| p.x > x)
            .unwrap_or(self.points.len());
        let low = index.checked_sub(1).map_or(0, |i| self.points[i].y);
        let high = self.points.get(index).map_or(100, |p| p.y);
        let mut points = self.points.clone();
        points.insert(
            index,
            Point {
                x,
                y: at.y.clamp(low, high),
            },
        );
        self.replace(points)?;
        self.select(index, false);
        Ok(())
    }
    pub fn remove(&mut self) -> Result<(), String> {
        if self.kind == Kind::Gpu {
            return Err(msg("curve.err.anchor_remove", &[]));
        }
        if self.points.len() <= 2 {
            return Err(msg("curve.err.min_points", &[]));
        }
        if self.selected >= self.points.len() {
            return Err(msg("curve.err.no_point", &[]));
        }
        let mut points = self.points.clone();
        points.remove(self.selected);
        self.replace(points)?;
        self.select(self.selected.min(self.points.len() - 1), false);
        Ok(())
    }
    pub fn reset(&mut self) {
        // The original was validated when loaded and uses the same GPU grid.
        let _ = self.replace(self.original.clone());
        self.select(self.selected.min(self.points.len() - 1), false);
    }
    pub fn reference(&self) -> &[Point] {
        &self.original
    }
    pub fn select(&mut self, index: usize, extend: bool) {
        let index = index.min(self.points.len() - 1);
        self.selection = if extend {
            let anchor =
                self.selection.map_or(
                    self.selected,
                    |(a, b)| {
                        if self.selected == a { b } else { a }
                    },
                );
            Some((anchor.min(index), anchor.max(index)))
        } else {
            None
        };
        self.selected = index;
    }
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }
    pub fn select_all(&mut self) {
        self.selection = Some((0, self.points.len() - 1));
    }
    pub fn select_voltage_range(&mut self, a: i32, b: i32) -> bool {
        let mut indices = self
            .points
            .iter()
            .enumerate()
            .filter(|(_, p)| (a.min(b)..=a.max(b)).contains(&p.x))
            .map(|(i, _)| i);
        let Some(first) = indices.next() else {
            self.clear_selection();
            return false;
        };
        let last = indices.next_back().unwrap_or(first);
        self.selection = Some((first, last));
        self.selected = first;
        true
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            points: self.points.clone(),
            selected: self.selected,
            selection: self.selection,
        }
    }
    fn restore_snapshot(&mut self, s: Snapshot) {
        self.points = s.points;
        self.selected = s.selected.min(self.points.len() - 1);
        self.selection = s.selection;
    }
    fn remember(&mut self, before: Snapshot) {
        if self.undo.len() == HISTORY_LIMIT {
            self.undo.pop_front();
        }
        self.undo.push_back(before);
        self.redo.clear();
    }
    fn replace(&mut self, points: Vec<Point>) -> Result<bool, String> {
        if self.points == points {
            return Ok(false);
        }
        let before = self.snapshot();
        self.points = points;
        if let Err(e) = self.value() {
            self.restore_snapshot(before);
            return Err(e);
        }
        self.selected = self.selected.min(self.points.len() - 1);
        if self.gesture.is_none() {
            self.remember(before);
        }
        Ok(true)
    }
    pub fn begin_gesture(&mut self) {
        self.end_gesture();
        self.gesture = Some(self.snapshot());
    }
    pub fn end_gesture(&mut self) {
        if let Some(before) = self.gesture.take() {
            if before.points != self.points {
                self.remember(before);
            }
        }
    }
    pub fn cancel_gesture(&mut self) -> bool {
        if let Some(before) = self.gesture.take() {
            let changed = before.points != self.points;
            self.restore_snapshot(before);
            changed
        } else {
            false
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self) -> bool {
        self.end_gesture();
        if let Some(before) = self.undo.pop_back() {
            self.redo.push(self.snapshot());
            self.restore_snapshot(before);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(after) = self.redo.pop() {
            let before = self.snapshot();
            self.restore_snapshot(after);
            if self.undo.len() == HISTORY_LIMIT {
                self.undo.pop_front();
            }
            self.undo.push_back(before);
            true
        } else {
            false
        }
    }
    /// Transform from a fixed gesture snapshot, so mouse events never accumulate
    /// drift. Group offsets share one clamped delta, preserving their shape.
    pub fn drag_gpu(
        &mut self,
        start: &[Point],
        mode: DragMode,
        delta: i32,
    ) -> Result<bool, String> {
        if self.kind != Kind::Gpu
            || start.len() != self.points.len()
            || start.iter().zip(&self.points).any(|(a, b)| a.x != b.x)
        {
            return Err(msg("curve.err.grid", &[]));
        }
        let selected = self.selected;
        if mode == DragMode::Point && self.selection.is_none() {
            return self.set(
                selected,
                Point {
                    x: start[selected].x,
                    y: start[selected].y.saturating_add(delta),
                },
                true,
            );
        }
        if mode == DragMode::Tilt {
            let pivot = if selected < self.points.len() / 2 {
                self.points.len() - 1
            } else {
                0
            };
            let divisor = i64::from(start[selected].x - start[pivot].x);
            if divisor == 0 {
                return Ok(false);
            }
            let mut points = start.to_vec();
            for p in &mut points {
                let change = i64::from(delta) * i64::from(p.x - start[pivot].x) / divisor;
                p.y = i32::try_from(i64::from(p.y) + change)
                    .map_err(|_| msg("curve.err.frequency_range", &[]))?;
            }
            return self.replace(points);
        }
        let (first, last) = self.selection.unwrap_or((0, self.points.len() - 1));
        let basis = if mode == DragMode::ReferenceOffset {
            &self.original
        } else {
            start
        };
        let requested =
            i64::from(start[selected].y) + i64::from(delta) - i64::from(basis[selected].y);
        let mut lo = i64::MIN;
        let mut hi = i64::MAX;
        let gpu = self.gpu.as_ref().ok_or("GPU-Treibergrenzen fehlen.")?;
        for (i, p) in basis.iter().enumerate().take(last + 1).skip(first) {
            let n = &gpu.nodes[i];
            let base = i64::from(n.frequency_khz) - i64::from(n.delta_khz);
            lo = lo.max(
                (base + i64::from(gpu.minimum_delta_khz) + 999)
                    .div_euclid(1000)
                    .max(1)
                    - i64::from(p.y),
            );
            hi =
                hi.min((base + i64::from(gpu.maximum_delta_khz)).div_euclid(1000) - i64::from(p.y));
        }
        if first > 0 {
            lo = lo.max(i64::from(start[first - 1].y) - i64::from(basis[first].y));
        }
        if last + 1 < start.len() {
            hi = hi.min(i64::from(start[last + 1].y) - i64::from(basis[last].y));
        }
        if lo > hi {
            return Err("Auswahl und Nachbarpunkte erlauben keinen gemeinsamen Offset.".into());
        }
        let offset = requested.clamp(lo, hi);
        let mut points = start.to_vec();
        for i in first..=last {
            points[i].y = i32::try_from(i64::from(basis[i].y) + offset)
                .map_err(|_| msg("curve.err.frequency_range", &[]))?;
        }
        self.replace(points)
    }
    pub fn flatten_selection(&mut self) -> Result<bool, String> {
        if self.kind != Kind::Gpu {
            return Err(msg("curve.err.flatten_gpu", &[]));
        }
        let Some((first, last)) = self.selection else {
            return self.plateau(self.points[self.selected].y);
        };
        let mut points = self.points.clone();
        let frequency = self.points[self.selected].y;
        for p in &mut points[first..=last] {
            p.y = frequency;
        }
        self.replace(points)
    }
    pub fn bounds(&self) -> (Point, Point) {
        if self.kind == Kind::Gpu {
            let min = self
                .points
                .iter()
                .chain(&self.original)
                .map(|p| p.y)
                .min()
                .unwrap_or(0);
            let max = self
                .points
                .iter()
                .chain(&self.original)
                .map(|p| p.y)
                .max()
                .unwrap_or(4000);
            (
                Point {
                    x: self.points[0].x,
                    y: (min.saturating_sub(200).max(0) / 100) * 100,
                },
                Point {
                    x: self.points.last().unwrap().x,
                    y: ((max.saturating_add(299)) / 100) * 100,
                },
            )
        } else {
            (Point { x: 0, y: 0 }, Point { x: 110, y: 100 })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn editor() -> Curve {
        Curve::gpu(
            VfState {
                nodes: (0..4)
                    .map(|i| crate::vf::VfNode {
                        index: i,
                        voltage_uv: 800000 + u32::from(i) * 100000,
                        frequency_khz: 1000000 + u32::from(i) * 200000,
                        delta_khz: 0,
                    })
                    .collect(),
                minimum_delta_khz: -400000,
                maximum_delta_khz: 400000,
                first_index: 0,
                last_index: 3,
            },
            None,
        )
        .unwrap()
    }
    #[test]
    fn shift_reference_and_alt_current_offsets_have_distinct_shapes() {
        let mut c = editor();
        c.select(1, false);
        c.set(1, Point { x: 900, y: 1250 }, false).unwrap();
        let start = c.points.clone();
        c.drag_gpu(&start, DragMode::CurrentOffset, 15).unwrap();
        assert_eq!(
            c.points.iter().map(|p| p.y).collect::<Vec<_>>(),
            [1015, 1265, 1415, 1615]
        );
        c.drag_gpu(&start, DragMode::ReferenceOffset, 15).unwrap();
        assert_eq!(
            c.points.iter().map(|p| p.y).collect::<Vec<_>>(),
            [1065, 1265, 1465, 1665]
        );
        assert_eq!(
            c.points.iter().map(|p| p.x).collect::<Vec<_>>(),
            [800, 900, 1000, 1100]
        );
    }
    #[test]
    fn a_selected_band_shares_one_bound_and_a_drag_is_one_undo_step() {
        let mut c = editor();
        assert!(c.select_voltage_range(1050, 850));
        assert_eq!(c.selection(), Some((1, 2)));
        let start = c.points.clone();
        c.begin_gesture();
        c.drag_gpu(&start, DragMode::CurrentOffset, 30).unwrap();
        c.drag_gpu(&start, DragMode::CurrentOffset, 900).unwrap();
        assert_eq!(
            c.points.iter().map(|p| p.y).collect::<Vec<_>>(),
            [1000, 1400, 1600, 1600]
        );
        c.end_gesture();
        assert!(c.undo());
        assert_eq!(c.points, start);
        assert!(!c.can_undo());
        assert!(c.redo());
        assert_eq!(c.points[2].y, 1600);
        c.begin_gesture();
        let before = c.points.clone();
        c.drag_gpu(&before, DragMode::CurrentOffset, -50).unwrap();
        assert!(c.cancel_gesture());
        assert_eq!(c.points, before);
    }
    #[test]
    fn tilt_keeps_the_opposite_anchor_and_invalid_edits_are_atomic() {
        let mut c = editor();
        let start = c.points.clone();
        c.select(3, false);
        c.drag_gpu(&start, DragMode::Tilt, 200).unwrap();
        assert_eq!(
            c.points.iter().map(|p| p.y).collect::<Vec<_>>(),
            [1000, 1266, 1533, 1800]
        );
        let before = c.points.clone();
        assert!(c.drag_gpu(&start, DragMode::Tilt, 500).is_err());
        assert_eq!(c.points, before);
        assert!(c.drag_gpu(&start[..3], DragMode::CurrentOffset, 5).is_err());
        assert_eq!(c.points, before);
    }
    #[test]
    fn selection_flatten_and_redo_preserve_the_grid_and_branch_history() {
        let mut c = editor();
        c.select_voltage_range(900, 1000);
        c.flatten_selection().unwrap();
        assert_eq!(
            c.points.iter().map(|p| p.y).collect::<Vec<_>>(),
            [1000, 1200, 1200, 1600]
        );
        assert!(c.undo());
        assert!(c.can_redo());
        c.select(3, false);
        c.set(3, Point { x: 1100, y: 1610 }, false).unwrap();
        assert!(!c.can_redo());
        c.select_all();
        c.select(0, true);
        assert!(c.selection().is_some());
        c.value().unwrap();
        for i in 0..100 {
            c.set(
                3,
                Point {
                    x: 1100,
                    y: 1610 + i,
                },
                false,
            )
            .unwrap();
        }
        let mut undos = 0;
        while c.undo() {
            undos += 1;
        }
        assert_eq!(undos, HISTORY_LIMIT);
    }
    #[test]
    fn gpu_anchors_and_complete_driver_bounds_survive_edit_and_rejected_plateau() {
        let state = VfState {
            nodes: vec![
                crate::vf::VfNode {
                    index: 0,
                    voltage_uv: 800000,
                    frequency_khz: 1000000,
                    delta_khz: 0,
                },
                crate::vf::VfNode {
                    index: 1,
                    voltage_uv: 900000,
                    frequency_khz: 1200000,
                    delta_khz: 0,
                },
                crate::vf::VfNode {
                    index: 2,
                    voltage_uv: 1000000,
                    frequency_khz: 1400000,
                    delta_khz: 0,
                },
            ],
            minimum_delta_khz: -200000,
            maximum_delta_khz: 200000,
            first_index: 0,
            last_index: 2,
        };
        let mut curve = Curve::gpu(state, None).unwrap();
        assert!(curve.set(1, Point { x: 905, y: 1200 }, true).is_err());
        assert!(curve.set(1, Point { x: 900, y: 9000 }, false).is_err());
        curve.selected = 1;
        assert!(curve.plateau(1000).is_err());
        assert_eq!(curve.points[2].y, 1400);
        curve.plateau(1250).unwrap();
        assert_eq!(
            curve.points.iter().map(|p| p.x).collect::<Vec<_>>(),
            [800, 900, 1000]
        );
        assert!(curve.insert().is_err());
        curve.value().unwrap();
    }
    #[test]
    fn fan_drag_preserves_order_and_non_decreasing_cooling_and_can_persist() {
        let mut curve = Curve::fan(Kind::CpuFan, None).unwrap();
        curve.set(1, Point { x: 100, y: 0 }, true).unwrap();
        assert_eq!(curve.points[1], Point { x: 74, y: 30 });
        assert!(curve.set(1, Point { x: 75, y: 20 }, false).is_err());
        curve.selected = 0;
        curve.insert().unwrap();
        assert_eq!(curve.points.len(), 5);
        curve.remove().unwrap();
        Setting {
            feature: Feature::CpuFanCurve,
            value: curve.value().unwrap(),
        }
        .validate()
        .unwrap();
        while curve.points.len() > 2 {
            curve.remove().unwrap();
        }
        assert!(curve.remove().is_err());
    }
    #[test]
    fn fan_points_insert_at_chart_position_and_stay_monotonic() {
        let mut curve = Curve::fan(Kind::CpuFan, None).unwrap();
        curve.insert_at(Point { x: 40, y: 99 }).unwrap();
        // Between 30 °C/30 % and 55 °C/45 %: duty clamps to the right neighbour.
        assert_eq!(curve.points[1], Point { x: 40, y: 45 });
        assert_eq!(curve.selected, 1);
        assert!(curve.insert_at(Point { x: 40, y: 40 }).is_err());
        curve.insert_at(Point { x: 100, y: 0 }).unwrap();
        assert_eq!(*curve.points.last().unwrap(), Point { x: 100, y: 100 });
        while curve.points.len() < MAX_FAN_POINTS {
            let x = (0..=110)
                .find(|x| curve.points.iter().all(|p| p.x != *x))
                .unwrap();
            curve.insert_at(Point { x, y: 50 }).unwrap();
        }
        assert!(curve.insert_at(Point { x: 1, y: 50 }).is_err());
        assert!(curve.value().is_ok());
    }
}
