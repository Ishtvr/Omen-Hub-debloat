//! "Keep preset" policy. Firmware (HP BIOS / NVIDIA platform controller) can
//! rewrite CPU limits and the HP GPU flags after a preset was applied. While
//! the setting is on, the app compares the applied preset's targets with the
//! live values and rewrites only those that drifted. A value overwritten again
//! after every rewrite is given up for that applied revision (clear limit:
//! three rewrites); a new Apply starts again.
use crate::{features::Feature, profiles::Setting};
use std::{collections::BTreeMap, time::Duration};

/// Values the firmware is known or suspected to rewrite on its own.
pub const FEATURES: &[Feature] = &[
    Feature::CpuPptLimitMw,
    Feature::CpuSlowPptLimitMw,
    Feature::CpuTdcMa,
    Feature::CpuEdcMa,
    Feature::CpuTemperatureLimitC,
    Feature::CpuPboScalar,
    Feature::CpuCurveOptimizerAllCore,
    Feature::CpuCurveOptimizerPerCore,
    Feature::GpuCustomTgp,
    Feature::GpuUnleashed,
    Feature::GpuDynamicBoost,
];
/// Consecutive checks with drift that are still corrected.
pub const MAX_REWRITES: u8 = 3;
/// Check interval right after an Apply, a Windows power event or a drift.
pub const FAST_INTERVAL: Duration = Duration::from_secs(5);
/// How long the fast interval lasts after such a trigger.
pub const FAST_PHASE: Duration = Duration::from_secs(60);
/// Interval while every kept value stays stable. Firmware on 8E35 rewrites
/// limits without any Windows event, so a rare check remains necessary.
pub const SLOW_INTERVAL: Duration = Duration::from_secs(30);
/// Stable interval on battery: each check reads the SMU and the BIOS, which
/// costs wakeups; the firmware resets seen so far happened on AC.
pub const BATTERY_INTERVAL: Duration = Duration::from_secs(60);
/// Windows sends several power events within a few hundred milliseconds
/// (lid, AC adapter, display); one check follows the last of them.
pub const EVENT_DEBOUNCE: Duration = Duration::from_millis(500);

/// Event-driven pacing: a trigger (Apply, resume, AC/DC, power plan, display
/// on, observed drift) starts a short fast phase; otherwise checks are rare.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pace {
    fast_until: Option<Duration>,
}
impl Pace {
    /// `now` is a monotonic time since an arbitrary origin.
    pub fn trigger(&mut self, now: Duration) {
        self.fast_until = Some(now + FAST_PHASE);
    }
    /// After giving up, checks return to the slow interval.
    pub fn calm(&mut self) {
        self.fast_until = None;
    }
    pub fn interval(&self, now: Duration) -> Duration {
        match self.fast_until {
            Some(until) if now < until => FAST_INTERVAL,
            _ => SLOW_INTERVAL,
        }
    }
}

/// The applied settings this policy keeps.
pub fn targets(applied: &[Setting]) -> Vec<Setting> {
    applied
        .iter()
        .filter(|s| FEATURES.contains(&s.feature))
        .cloned()
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Matches its target; nothing to do.
    Keep,
    /// Drifted: write the target again.
    Rewrite,
    /// Drifted after every rewrite: stop for this revision and report once.
    GiveUp,
    /// Already given up for this applied revision.
    Ignore,
}

#[derive(Debug, Default)]
pub struct Hold {
    revision: Option<String>,
    drift: BTreeMap<Feature, u8>,
}
impl Hold {
    /// A newly applied revision starts with fresh counters; returns whether
    /// it is new.
    pub fn track(&mut self, revision: &str) -> bool {
        if self.revision.as_deref() == Some(revision) {
            return false;
        }
        self.revision = Some(revision.into());
        self.drift.clear();
        true
    }
    pub fn clear(&mut self) {
        self.revision = None;
        self.drift.clear();
    }
    /// One check result for `feature`. A stable check resets its counter;
    /// once given up, the value stays abandoned until a new revision.
    pub fn decide(&mut self, feature: Feature, drifted: bool) -> Action {
        let given_up = MAX_REWRITES + 1;
        let count = self.drift.entry(feature).or_insert(0);
        if *count >= given_up {
            return Action::Ignore;
        }
        if !drifted {
            *count = 0;
            return Action::Keep;
        }
        *count += 1;
        match (*count).cmp(&given_up) {
            std::cmp::Ordering::Less => Action::Rewrite,
            std::cmp::Ordering::Equal => Action::GiveUp,
            std::cmp::Ordering::Greater => Action::Ignore,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::Value;

    #[test]
    fn only_firmware_rewritten_values_are_kept() {
        let applied = [
            Setting {
                feature: Feature::CpuPptLimitMw,
                value: Value::Integer(75_000),
            },
            Setting {
                feature: Feature::WindowsBoostMode,
                value: Value::Choice(crate::profiles::Choice::Aggressive),
            },
        ];
        assert_eq!(targets(&applied), applied[..1]);
    }

    #[test]
    fn three_rewrites_then_one_report_then_abandoned_until_a_new_apply() {
        let mut hold = Hold::default();
        hold.track("a");
        let f = Feature::CpuSlowPptLimitMw;
        for _ in 0..MAX_REWRITES {
            assert_eq!(hold.decide(f, true), Action::Rewrite);
        }
        assert_eq!(hold.decide(f, true), Action::GiveUp);
        for drifted in [true, false, true] {
            assert_eq!(hold.decide(f, drifted), Action::Ignore);
        }
        assert!(!hold.track("a"));
        assert_eq!(hold.decide(f, true), Action::Ignore);
        // Only a new Apply (revision) starts counting again.
        assert!(hold.track("b"));
        assert_eq!(hold.decide(f, true), Action::Rewrite);
    }

    #[test]
    fn triggers_start_a_short_fast_phase_then_checks_become_rare() {
        let mut pace = Pace::default();
        let at = Duration::from_secs;
        assert_eq!(pace.interval(at(0)), SLOW_INTERVAL);
        pace.trigger(at(100));
        assert_eq!(pace.interval(at(100)), FAST_INTERVAL);
        assert_eq!(pace.interval(at(159)), FAST_INTERVAL);
        assert_eq!(pace.interval(at(160)), SLOW_INTERVAL);
        // A later trigger extends the fast phase from its own time.
        pace.trigger(at(150));
        assert_eq!(pace.interval(at(200)), FAST_INTERVAL);
    }

    #[test]
    fn a_stable_check_resets_the_drift_counter() {
        let mut hold = Hold::default();
        hold.track("a");
        let f = Feature::CpuPptLimitMw;
        assert_eq!(hold.decide(f, true), Action::Rewrite);
        assert_eq!(hold.decide(f, true), Action::Rewrite);
        assert_eq!(hold.decide(f, false), Action::Keep);
        for _ in 0..MAX_REWRITES {
            assert_eq!(hold.decide(f, true), Action::Rewrite);
        }
        assert_eq!(hold.decide(f, true), Action::GiveUp);
    }
}
