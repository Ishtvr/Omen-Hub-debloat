//! "Keep preset" policy. Firmware (HP BIOS / NVIDIA platform controller) can
//! rewrite CPU limits and the HP GPU flags after a preset was applied. While
//! the setting is on, the app compares the applied preset's targets with the
//! live values and rewrites only those that drifted. A value overwritten again
//! after every rewrite is given up instead of written endlessly.
use crate::{features::Feature, profiles::Setting};
use std::collections::BTreeMap;

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
/// Drifted checks after giving up before the next attempt (about five
/// minutes at the dashboard's 5-second interval); holding never ends for good.
pub const RETRY_AFTER: u8 = 60;

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
    /// Drifted after every recent rewrite: stop and report once.
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
    /// A newly applied revision starts with fresh counters.
    pub fn track(&mut self, revision: &str) {
        if self.revision.as_deref() != Some(revision) {
            self.revision = Some(revision.into());
            self.drift.clear();
        }
    }
    pub fn clear(&mut self) {
        self.revision = None;
        self.drift.clear();
    }
    /// One check result for `feature`. A stable check resets its counter;
    /// after giving up, the value is tried again once the back-off expired.
    pub fn decide(&mut self, feature: Feature, drifted: bool) -> Action {
        let count = self.drift.entry(feature).or_insert(0);
        if !drifted {
            *count = 0;
            return Action::Keep;
        }
        *count = count.saturating_add(1);
        let given_up = MAX_REWRITES + 1;
        if *count >= given_up + RETRY_AFTER {
            *count = 1;
            return Action::Rewrite;
        }
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
    fn three_rewrites_then_one_report_then_a_new_attempt_after_the_back_off() {
        let mut hold = Hold::default();
        hold.track("a");
        let f = Feature::CpuSlowPptLimitMw;
        for _ in 0..MAX_REWRITES {
            assert_eq!(hold.decide(f, true), Action::Rewrite);
        }
        assert_eq!(hold.decide(f, true), Action::GiveUp);
        for _ in 1..RETRY_AFTER {
            assert_eq!(hold.decide(f, true), Action::Ignore);
        }
        hold.track("a");
        // Holding never stops for good: the back-off ends with a new attempt.
        assert_eq!(hold.decide(f, true), Action::Rewrite);
        // A stable value or a new revision starts counting again.
        assert_eq!(hold.decide(f, false), Action::Keep);
        hold.track("b");
        assert_eq!(hold.decide(f, true), Action::Rewrite);
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
