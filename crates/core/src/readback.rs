//! Bounded observation of asynchronous controls; this policy never retries a SET.
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Wait,
    Matched,
    TimedOut,
}

pub struct Settling {
    deadline: Duration,
    quiet: Duration,
    matched_since: Option<Duration>,
}
impl Settling {
    pub fn new(deadline: Duration, quiet: Duration) -> Self {
        Self {
            deadline,
            quiet,
            matched_since: None,
        }
    }
    pub fn observe(&mut self, elapsed: Duration, matched: bool) -> Decision {
        if elapsed >= self.deadline {
            return Decision::TimedOut;
        }
        if !matched {
            self.matched_since = None;
            return Decision::Wait;
        }
        let since = *self.matched_since.get_or_insert(elapsed);
        if elapsed.saturating_sub(since) >= self.quiet {
            Decision::Matched
        } else {
            Decision::Wait
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }
    #[test]
    fn a_delayed_target_needs_a_stable_observation_window() {
        let mut state = Settling::new(ms(2000), ms(100));
        assert_eq!(state.observe(ms(0), false), Decision::Wait);
        assert_eq!(state.observe(ms(900), false), Decision::Wait);
        assert_eq!(state.observe(ms(1000), true), Decision::Wait);
        assert_eq!(state.observe(ms(1050), true), Decision::Wait);
        assert_eq!(state.observe(ms(1100), true), Decision::Matched);
    }
    #[test]
    fn an_intervening_wrong_value_resets_the_observation_window() {
        let mut state = Settling::new(ms(2000), ms(100));
        assert_eq!(state.observe(ms(0), true), Decision::Wait);
        assert_eq!(state.observe(ms(80), false), Decision::Wait);
        assert_eq!(state.observe(ms(100), true), Decision::Wait);
        assert_eq!(state.observe(ms(150), true), Decision::Wait);
        assert_eq!(state.observe(ms(200), true), Decision::Matched);
    }
    #[test]
    fn timeout_also_rejects_a_first_or_unsettled_late_match() {
        let mut state = Settling::new(ms(2000), ms(100));
        assert_eq!(state.observe(ms(1950), true), Decision::Wait);
        assert_eq!(state.observe(ms(2050), true), Decision::TimedOut);
        assert_eq!(
            Settling::new(ms(2000), ms(100)).observe(ms(2500), true),
            Decision::TimedOut
        );
        assert_eq!(
            Settling::new(ms(2000), ms(100)).observe(ms(2000), false),
            Decision::TimedOut
        );
    }
}
