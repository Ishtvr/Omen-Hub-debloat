//! Keep-alive lifecycle policy. This module schedules no real hardware calls.
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct Supervisor {
    suspended: bool,
    validated: bool,
    ac: Option<bool>,
    next: Option<Instant>,
}

impl Supervisor {
    pub fn power_changed(&mut self, ac: Option<bool>) {
        self.ac = ac;
        // Every power transition invalidates the previous session's validation.
        self.disarm();
    }
    pub fn suspend(&mut self) {
        self.suspended = true;
        self.disarm();
    }
    pub fn resume(&mut self) {
        self.suspended = false;
        self.disarm();
    }
    pub fn disarm(&mut self) {
        self.validated = false;
        self.next = None;
    }
    /// Intended for a future adapter after identity, capability and readback validation.
    pub fn arm(&mut self, verified: bool, now: Instant) -> Result<(), &'static str> {
        self.disarm();
        if !verified || self.suspended || self.ac != Some(true) {
            return Err("Keep-alive requires fresh validation, AC and an awake system");
        }
        self.validated = true;
        self.next = Some(now);
        Ok(())
    }
    pub fn due(&self, now: Instant) -> bool {
        self.validated
            && !self.suspended
            && self.ac == Some(true)
            && self.next.is_some_and(|next| now >= next)
    }
    /// The adapter supplies the model-specific interval. Failure immediately disarms.
    pub fn completed(&mut self, now: Instant, interval: Duration, success: bool) {
        if !success || interval.is_zero() || !self.validated {
            self.disarm();
        } else {
            self.next = now.checked_add(interval);
            if self.next.is_none() {
                self.disarm();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_support_or_power_cannot_arm() {
        let mut s = Supervisor::default();
        let now = Instant::now();
        assert!(s.arm(true, now).is_err());
        s.power_changed(Some(true));
        assert!(s.arm(false, now).is_err());
        assert!(!s.due(now));
    }
    #[test]
    fn suspend_resume_and_ac_changes_require_fresh_validation() {
        let now = Instant::now();
        let mut s = Supervisor::default();
        s.power_changed(Some(true));
        s.arm(true, now).unwrap();
        s.suspend();
        assert!(s.arm(true, now).is_err());
        s.resume();
        assert!(!s.due(now));
        s.arm(true, now).unwrap();
        s.power_changed(Some(false));
        assert!(!s.due(now));
        assert!(s.arm(true, now).is_err());
    }
    #[test]
    fn schedules_one_heartbeat_without_catchup_and_stops_on_error() {
        let now = Instant::now();
        let mut s = Supervisor::default();
        s.power_changed(Some(true));
        s.arm(true, now).unwrap();
        s.completed(now, Duration::from_secs(30), true);
        assert!(!s.due(now + Duration::from_secs(29)));
        assert!(s.due(now + Duration::from_secs(60)));
        s.completed(
            now + Duration::from_secs(60),
            Duration::from_secs(30),
            false,
        );
        assert!(!s.due(now + Duration::from_secs(90)));
    }
}
