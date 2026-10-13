//! Pure curve editing. No OS access, sensor claims or hardware writes.
use crate::{
    features::Feature,
    profiles::{FanPoint, Setting, Value, VfPoint},
    vf::VfState,
};

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
    pub fn title(self) -> &'static str {
        match self {
            Self::Gpu => "GPU V/F",
            Self::CpuFan => "CPU-Lüfter",
            Self::GpuFan => "GPU-Lüfter",
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
}
impl Curve {
    pub fn gpu(state: VfState, value: Option<&Value>) -> Result<Self, String> {
        let value = value.cloned().map_or_else(|| state.value(), Ok)?;
        let Value::VfCurve(points) = value else {
            return Err("Keine vollständige GPU-Kurve vorhanden.".into());
        };
        state.target_deltas(&points)?;
        let points = points
            .iter()
            .map(|p| {
                Ok(Point {
                    x: i32::try_from(p.voltage_mv).map_err(|_| "Spannungsanker zu groß")?,
                    y: i32::try_from(p.frequency_mhz).map_err(|_| "Frequenz zu groß")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            kind: Kind::Gpu,
            original: points.clone(),
            selected: points.len() - 1,
            points,
            gpu: Some(state),
        })
    }
    pub fn fan(kind: Kind, value: Option<&Value>) -> Result<Self, String> {
        if kind == Kind::Gpu {
            return Err("Lüfterkurventyp erwartet.".into());
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
            _ => return Err("Lüfterkurve hat einen falschen Datentyp.".into()),
        };
        let curve = Self {
            kind,
            original: points.clone(),
            points,
            selected: 0,
            gpu: None,
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
                                .map_err(|_| "Ungültiger Spannungsanker")?,
                            frequency_mhz: u32::try_from(p.y).map_err(|_| "Ungültige Frequenz")?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            )
        } else {
            if self.points.iter().any(|p| !(0..=110).contains(&p.x)) {
                return Err("Temperaturen müssen zwischen 0 und 110 °C liegen.".into());
            }
            Value::FanCurve(
                self.points
                    .iter()
                    .map(|p| {
                        Ok(FanPoint {
                            temperature_c: u8::try_from(p.x).map_err(|_| "Ungültige Temperatur")?,
                            duty_percent: u8::try_from(p.y).map_err(|_| "Ungültiger Lüfterwert")?,
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
        let p = *self.points.get(index).ok_or("Kein Punkt ausgewählt.")?;
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
                y: i32::try_from(min).map_err(|_| "Ungültige GPU-Untergrenze")?,
            };
            hi = Point {
                x: p.x,
                y: i32::try_from(max).map_err(|_| "Ungültige GPU-Obergrenze")?,
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
            return Err("Die Nachbarpunkte lassen keine gültige Änderung zu.".into());
        }
        Ok((lo, hi))
    }
    pub fn set(&mut self, index: usize, p: Point, clamp: bool) -> Result<bool, String> {
        let (lo, hi) = self.limits(index)?;
        if self.kind == Kind::Gpu && p.x != lo.x {
            return Err("GPU-Spannungsanker bleiben unverändert.".into());
        }
        let p = if clamp {
            Point {
                x: p.x.clamp(lo.x, hi.x),
                y: p.y.clamp(lo.y, hi.y),
            }
        } else {
            if p.x < lo.x || p.x > hi.x || p.y < lo.y || p.y > hi.y {
                return Err(format!(
                    "Punkt außerhalb der Grenzen: {} bis {} {}.",
                    lo.y,
                    hi.y,
                    self.kind.units().1
                ));
            }
            p
        };
        let old = self.points[index];
        self.points[index] = p;
        if let Err(e) = self.value() {
            self.points[index] = old;
            return Err(e);
        }
        Ok(old != p)
    }
    pub fn plateau(&mut self, frequency: i32) -> Result<bool, String> {
        if self.kind != Kind::Gpu {
            return Err("Ein Plateau ist nur für die GPU-Kurve verfügbar.".into());
        }
        if self.selected >= self.points.len() {
            return Err("Kein Punkt ausgewählt.".into());
        }
        let before = self.points.clone();
        for p in &mut self.points[self.selected..] {
            p.y = frequency;
        }
        if let Err(e) = self.value() {
            self.points = before;
            return Err(e);
        }
        Ok(self.points != before)
    }
    pub fn insert(&mut self) -> Result<(), String> {
        if self.kind == Kind::Gpu {
            return Err("GPU-Spannungsanker sind durch den Treiber festgelegt.".into());
        }
        if self.points.len() >= 32 {
            return Err("Höchstens 32 Lüfterpunkte sind möglich.".into());
        }
        let a = *self
            .points
            .get(self.selected)
            .ok_or("Kein Punkt ausgewählt.")?;
        let point = if let Some(b) = self.points.get(self.selected + 1) {
            if b.x - a.x < 2 {
                return Err("Zwischen diesen Temperaturen ist kein weiterer Punkt möglich.".into());
            }
            Point {
                x: (a.x + b.x) / 2,
                y: (a.y + b.y + 1) / 2,
            }
        } else {
            if a.x >= 110 {
                return Err("Am rechten Rand ist kein weiterer Punkt möglich.".into());
            }
            Point {
                x: (a.x + 10).min(110),
                y: a.y,
            }
        };
        self.selected += 1;
        self.points.insert(self.selected, point);
        self.value()?;
        Ok(())
    }
    pub fn remove(&mut self) -> Result<(), String> {
        if self.kind == Kind::Gpu {
            return Err("GPU-Spannungsanker können nicht entfernt werden.".into());
        }
        if self.points.len() <= 2 {
            return Err("Mindestens zwei Lüfterpunkte müssen erhalten bleiben.".into());
        }
        if self.selected >= self.points.len() {
            return Err("Kein Punkt ausgewählt.".into());
        }
        self.points.remove(self.selected);
        self.selected = self.selected.min(self.points.len() - 1);
        Ok(())
    }
    pub fn reset(&mut self) {
        self.points = self.original.clone();
        self.selected = self.selected.min(self.points.len() - 1);
    }
    pub fn bounds(&self) -> (Point, Point) {
        if let Some(gpu) = &self.gpu {
            let max = gpu
                .nodes
                .iter()
                .map(|p| {
                    (i64::from(p.frequency_khz) - i64::from(p.delta_khz)
                        + i64::from(gpu.maximum_delta_khz))
                    .div_euclid(1000)
                })
                .max()
                .unwrap_or(4000)
                .clamp(1, 10000) as i32;
            (
                Point {
                    x: self.points[0].x,
                    y: 0,
                },
                Point {
                    x: self.points.last().unwrap().x,
                    y: ((max + 999) / 1000) * 1000,
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
}
