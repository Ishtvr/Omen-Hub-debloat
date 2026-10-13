//! Integer viewport policy, separate from editable points and driver limits.
use crate::curve_editor::{Curve, Kind, Point};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub lo: Point,
    pub hi: Point,
}
impl Viewport {
    pub fn fit(curve: &Curve) -> Self {
        let (mut lo, mut hi) = curve.bounds();
        if curve.kind == Kind::Gpu {
            lo.x = (lo.x / 25) * 25;
            hi.x = hi.x.saturating_add(24) / 25 * 25;
        }
        Self { lo, hi }
    }
    /// Zoom one axis about a cursor or selected point; keep the voltage grid in
    /// reach. Zoom never edits a point or limits the saved profile's domain.
    pub fn zoom(&mut self, curve: &Curve, at: Point, vertical: bool, inward: bool) {
        let full = Self::fit(curve);
        let (lo, hi, anchor, min_span, domain_lo, domain_hi) = if vertical {
            (
                self.lo.y,
                self.hi.y,
                at.y,
                if curve.kind == Kind::Gpu { 100 } else { 10 },
                0,
                if curve.kind == Kind::Gpu {
                    full.hi.y.max(self.hi.y).max(at.y.saturating_add(300))
                } else {
                    100
                },
            )
        } else {
            (
                self.lo.x,
                self.hi.x,
                at.x,
                if curve.kind == Kind::Gpu { 50 } else { 10 },
                full.lo.x,
                full.hi.x,
            )
        };
        let span = i64::from(hi - lo);
        let available = i64::from(domain_hi - domain_lo).max(1);
        let next = (if inward {
            span * 4 / 5
        } else {
            (span * 5 + 3) / 4
        })
        .clamp(i64::from(min_span).min(available), available);
        let anchor = anchor.clamp(lo, hi);
        let start = (i64::from(anchor) - i64::from(anchor - lo) * next / span.max(1))
            .clamp(i64::from(domain_lo), i64::from(domain_hi) - next) as i32;
        if vertical {
            self.lo.y = start;
            self.hi.y = start + next as i32;
        } else {
            self.lo.x = start;
            self.hi.x = start + next as i32;
        }
    }
    pub fn pan(&mut self, curve: &Curve, direction: i32) {
        let full = Self::fit(curve);
        let span = self.hi.x - self.lo.x;
        let delta = (span / 8).max(1) * direction.signum();
        let start = self
            .lo
            .x
            .saturating_add(delta)
            .clamp(full.lo.x, full.hi.x - span);
        self.lo.x = start;
        self.hi.x = start + span;
    }
}

/// A readable labelled grid. Minor ticks are separately drawn by the renderer.
pub fn tick_step(span: i32, pixels: i32, spacing: i32) -> usize {
    let needed = i64::from(span) * i64::from(spacing) / i64::from(pixels.max(1));
    [
        1, 2, 5, 10, 25, 50, 100, 200, 250, 500, 1000, 2000, 5000, 10000,
    ]
    .into_iter()
    .find(|s| *s as i64 >= needed)
    .unwrap_or(((needed + 9999) / 10000 * 10000).max(10000) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_and_pan_keep_the_complete_curve_unchanged_and_reachable() {
        let curve = Curve::fan(Kind::CpuFan, None).unwrap();
        let original = curve.value().unwrap();
        let mut v = Viewport::fit(&curve);
        for _ in 0..40 {
            v.zoom(&curve, Point { x: 55, y: 45 }, false, true);
        }
        assert_eq!(v.hi.x - v.lo.x, 10);
        for _ in 0..200 {
            v.pan(&curve, 1);
        }
        assert_eq!(v.hi.x, 110);
        for _ in 0..200 {
            v.pan(&curve, -1);
        }
        assert_eq!(v.lo.x, 0);
        for _ in 0..40 {
            v.zoom(&curve, Point { x: 55, y: 45 }, false, false);
        }
        assert_eq!(v, Viewport::fit(&curve));
        assert_eq!(curve.value().unwrap(), original);
        assert_eq!(tick_step(550, 1400, 48), 25);
        assert_eq!(tick_step(550, 400, 48), 100);
    }
}
