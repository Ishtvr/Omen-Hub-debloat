//! Public driver-reported power-limit state, in milliwatts throughout.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerLimitState {
    pub current_mw: u32,
    pub minimum_mw: u32,
    pub maximum_mw: u32,
}
impl PowerLimitState {
    pub fn validate(&self) -> Result<(), String> {
        if self.minimum_mw == 0
            || self.minimum_mw > self.maximum_mw
            || !(self.minimum_mw..=self.maximum_mw).contains(&self.current_mw)
        {
            return Err("Driver power-limit state is inconsistent".into());
        }
        Ok(())
    }
    pub fn target(&self, requested_mw: i64) -> Result<u32, String> {
        self.validate()?;
        let requested =
            u32::try_from(requested_mw).map_err(|_| "Power limit exceeds the public ABI")?;
        if !(self.minimum_mw..=self.maximum_mw).contains(&requested) {
            return Err("Power limit is outside the limits reported by the driver".into());
        }
        Ok(requested)
    }
    pub fn same_limits(&self, other: &Self) -> bool {
        self.minimum_mw == other.minimum_mw && self.maximum_mw == other.maximum_mw
    }
    /// Return only our original or our attempted target, never external tuning.
    pub fn owns(&self, live: &Self, attempted: Option<i64>) -> Result<bool, String> {
        self.validate()?;
        live.validate()?;
        let target = attempted.map(|v| self.target(v)).transpose()?;
        Ok(self.same_limits(live)
            && (live.current_mw == self.current_mw || Some(live.current_mw) == target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn power_uses_exact_milliwatts_and_current_driver_bounds() {
        let state = PowerLimitState {
            current_mw: 90_000,
            minimum_mw: 60_000,
            maximum_mw: 115_000,
        };
        assert_eq!(state.target(90_125).unwrap(), 90_125);
        for value in [-1, 59_999, 115_001, i64::MAX] {
            assert!(state.target(value).is_err());
        }
        let fixed = PowerLimitState {
            current_mw: 90_000,
            minimum_mw: 90_000,
            maximum_mw: 90_000,
        };
        assert_eq!(fixed.target(90_000).unwrap(), 90_000);
        assert!(fixed.target(90_001).is_err());
    }
    #[test]
    fn recovery_accepts_attempted_effect_but_rejects_changed_limits_and_other_writers() {
        let original = PowerLimitState {
            current_mw: 90_000,
            minimum_mw: 60_000,
            maximum_mw: 115_000,
        };
        let mut live = original.clone();
        live.current_mw = 80_000;
        assert!(!original.owns(&live, None).unwrap());
        assert!(original.owns(&live, Some(80_000)).unwrap());
        live.current_mw = 85_000;
        assert!(!original.owns(&live, Some(80_000)).unwrap());
        live = original.clone();
        live.maximum_mw = 120_000;
        assert!(!original.owns(&live, Some(80_000)).unwrap());
        live.minimum_mw = 0;
        assert!(original.owns(&live, Some(80_000)).is_err());
    }
}
