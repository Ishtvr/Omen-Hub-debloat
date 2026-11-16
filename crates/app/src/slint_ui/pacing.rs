//! Coalesce software redraws to the actual display period. Slint 1.18.1's
//! Windows timer truncates 144 Hz to 6 ms, scheduling ~167 rather than 144 Hz.
//! Only pending visible redraws are delayed; there is no idle repaint timer.
use std::time::{Duration, Instant};

/// Slint 1.18's internal Instant truncates sub-ms Duration additions. Never
/// schedule a zero-ms wake for a fractional display deadline.
pub(super) fn timer_delay(delay: Duration) -> Duration {
    let millis = delay.as_millis() as u64;
    Duration::from_millis(millis + u64::from(!delay.subsec_nanos().is_multiple_of(1_000_000)))
        .max(Duration::from_millis(1))
}

pub(super) struct Pacer {
    pub enabled: bool,
    pub refresh_known: bool,
    interval: Duration,
    next: Option<Instant>,
}
impl Pacer {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            refresh_known: false,
            interval: Duration::from_secs_f64(1.0 / 60.0),
            next: None,
        }
    }
    pub fn set_refresh(&mut self, millihertz: Option<u32>) {
        let rate = millihertz
            .filter(|r| (1000..=1_000_000).contains(r))
            .unwrap_or(60000);
        self.interval = Duration::from_secs_f64(1000.0 / f64::from(rate));
        self.refresh_known = true;
        self.next = None;
    }
    pub fn reset(&mut self) {
        self.next = None;
        self.refresh_known = false;
    }
    pub fn delay(&mut self, now: Instant) -> Option<Duration> {
        self.next = Some(if let Some(next) = self.next {
            if now < next {
                return Some(next.duration_since(now));
            }
            // Keep the display phase despite timer rounding. A late frame is
            // dropped rather than scheduling catch-up redraws in a burst.
            if now.duration_since(next) < self.interval {
                next + self.interval
            } else {
                now + self.interval
            }
        } else {
            now + self.interval
        });
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_wakes_never_create_zero_millisecond_timers() {
        assert_eq!(
            timer_delay(Duration::from_nanos(1)),
            Duration::from_millis(1)
        );
        assert_eq!(
            timer_delay(Duration::from_micros(6944)),
            Duration::from_millis(7)
        );
        assert_eq!(
            timer_delay(Duration::from_millis(16)),
            Duration::from_millis(16)
        );
    }
    #[test]
    fn high_refresh_period_is_not_truncated_to_whole_milliseconds() {
        let mut p = Pacer::new(true);
        p.set_refresh(Some(144000));
        let now = Instant::now();
        assert!(p.delay(now).is_none());
        assert_eq!(
            p.delay(now + Duration::from_millis(6)),
            Some(Duration::from_nanos(944444))
        );
        assert!(p.delay(now + Duration::from_millis(7)).is_none());
    }
    #[test]
    fn delayed_requests_coalesce_and_restore_has_no_first_frame_delay() {
        let mut p = Pacer::new(true);
        p.set_refresh(None);
        let now = Instant::now();
        assert!(p.delay(now).is_none());
        assert!(p.delay(now + Duration::from_millis(3)).is_some());
        assert!(p.delay(now + Duration::from_millis(17)).is_none());
        p.reset();
        assert!(p.delay(now + Duration::from_millis(18)).is_none());
    }
}
