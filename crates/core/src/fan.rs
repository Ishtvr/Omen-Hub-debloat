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

#[cfg(test)]
mod tests {
    use super::*;
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
}
