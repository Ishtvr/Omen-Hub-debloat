//! Percentage policy only. EC units/RPM conversion must be verified per board.

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub temperature_c: i32,
    pub percent: u8,
}

#[derive(Debug)]
pub struct FanCurve(Vec<Point>);

impl FanCurve {
    pub fn new(points: Vec<Point>) -> Result<Self, &'static str> {
        if points.len() < 2 {
            return Err("A curve needs at least two points");
        }
        if points
            .iter()
            .any(|p| !(0..=110).contains(&p.temperature_c) || p.percent > 100)
        {
            return Err("Invalid temperature or fan percentage");
        }
        if points
            .windows(2)
            .any(|p| p[0].temperature_c >= p[1].temperature_c || p[0].percent > p[1].percent)
        {
            return Err("Temperatures must increase and fan speed must not decrease");
        }
        Ok(Self(points))
    }

    /// Missing telemetry requests maximum cooling. A hardware adapter must still fail closed.
    pub fn target(&self, temperature_c: Option<i32>) -> u8 {
        let Some(t) = temperature_c.filter(|t| (-20..=150).contains(t)) else {
            return 100;
        };
        if !(0..100).contains(&t) {
            return 100;
        }
        if t <= self.0[0].temperature_c {
            return self.0[0].percent;
        }
        for pair in self.0.windows(2) {
            let [a, b] = [pair[0], pair[1]];
            if t <= b.temperature_c {
                let rise = i32::from(b.percent - a.percent) * (t - a.temperature_c);
                let span = b.temperature_c - a.temperature_c;
                // Round upward so interpolation never undershoots requested cooling.
                return a.percent + ((rise + span - 1) / span) as u8;
            }
        }
        self.0.last().unwrap().percent
    }
}

/// HP notebook fan levels are 100-RPM units (`0x2E`). The Hub never commands
/// more than 48 for a custom curve and keeps a running fan at 19 or above.
pub const MAX_LEVEL: u8 = 48;
pub const MIN_RUNNING_LEVEL: u8 = 19;
/// Temperatures at which the service gives full cooling regardless of curve.
pub const CPU_EMERGENCY_C: f32 = 92.0;
pub const GPU_EMERGENCY_C: f32 = 85.0;
pub const IR_EMERGENCY_C: u8 = 55;

/// Curve percent to HP level: 0 % stops the fan, anything else runs at
/// least at the Hub's minimum running speed.
pub fn level(percent: u8) -> u8 {
    if percent == 0 {
        return 0;
    }
    let scaled = (u32::from(percent.min(100)) * u32::from(MAX_LEVEL) + 50) / 100;
    (scaled as u8).clamp(MIN_RUNNING_LEVEL, MAX_LEVEL)
}

/// Rises at once; falls by at most two levels per step so the fan does not
/// pump on short temperature dips.
#[derive(Debug, Default)]
pub struct Smoother {
    current: Option<u8>,
}
impl Smoother {
    pub fn step(&mut self, target: u8) -> u8 {
        let next = match self.current {
            Some(current) if target < current => current - (current - target).min(2),
            _ => target,
        };
        self.current = Some(next);
        next
    }
}

/// True when any reading demands full cooling: a hot sensor, a missing CPU
/// reading or a set BIOS over-temperature/over-current protection bit.
pub fn emergency(cpu_c: Option<f32>, gpu_c: Option<f32>, ir_c: Option<u8>, protection: u8) -> bool {
    protection != 0
        || cpu_c.is_none_or(|t| !t.is_finite() || t >= CPU_EMERGENCY_C)
        || gpu_c.is_some_and(|t| t >= GPU_EMERGENCY_C)
        || ir_c.is_some_and(|t| t >= IR_EMERGENCY_C)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn levels_smoothing_and_emergency() {
        assert_eq!(level(0), 0);
        assert_eq!(level(1), MIN_RUNNING_LEVEL);
        assert_eq!(level(50), 24);
        assert_eq!(level(100), MAX_LEVEL);
        let mut s = Smoother::default();
        assert_eq!(s.step(40), 40);
        assert_eq!(s.step(20), 38);
        assert_eq!(s.step(45), 45);
        assert!(emergency(None, Some(50.0), None, 0));
        assert!(emergency(Some(60.0), Some(50.0), None, 2));
        assert!(emergency(Some(93.0), None, None, 0));
        assert!(!emergency(Some(70.0), None, Some(40), 0));
    }
    fn curve() -> FanCurve {
        FanCurve::new(vec![
            Point {
                temperature_c: 40,
                percent: 30,
            },
            Point {
                temperature_c: 80,
                percent: 90,
            },
        ])
        .unwrap()
    }
    #[test]
    fn missing_or_extreme_sensor_never_reduces_cooling() {
        for t in [None, Some(-10), Some(101), Some(i32::MAX)] {
            assert_eq!(curve().target(t), 100);
        }
    }
    #[test]
    fn interpolates_and_clamps() {
        assert_eq!(curve().target(Some(20)), 30);
        assert_eq!(curve().target(Some(60)), 60);
        assert_eq!(curve().target(Some(41)), 32);
        assert_eq!(curve().target(Some(90)), 90);
    }
    #[test]
    fn rejects_dangerous_curves() {
        for points in [
            vec![
                Point {
                    temperature_c: 40,
                    percent: 101,
                },
                Point {
                    temperature_c: 80,
                    percent: 100,
                },
            ],
            vec![
                Point {
                    temperature_c: 40,
                    percent: 80,
                },
                Point {
                    temperature_c: 80,
                    percent: 30,
                },
            ],
            vec![
                Point {
                    temperature_c: 80,
                    percent: 30,
                },
                Point {
                    temperature_c: 80,
                    percent: 90,
                },
            ],
        ] {
            assert!(FanCurve::new(points).is_err());
        }
    }
    #[test]
    fn emergency_triggers_on_nan_hot_gpu_or_surface() {
        assert!(emergency(Some(f32::NAN), Some(50.0), None, 0));
        assert!(emergency(Some(f32::INFINITY), Some(50.0), None, 0));
        assert!(emergency(Some(70.0), Some(GPU_EMERGENCY_C), None, 0));
        assert!(emergency(Some(70.0), Some(86.0), None, 0));
        assert!(!emergency(Some(70.0), Some(84.9), None, 0));
        assert!(emergency(Some(70.0), None, Some(IR_EMERGENCY_C), 0));
        assert!(emergency(Some(70.0), None, Some(56), 0));
        assert!(!emergency(Some(70.0), None, Some(54), 0));
        assert!(emergency(Some(70.0), None, None, 1));
    }
}
