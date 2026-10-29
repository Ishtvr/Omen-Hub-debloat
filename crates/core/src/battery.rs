//! Automatic preset change when the charger is unplugged or plugged in again.
//! Pure policy: the dashboard applies the chosen selection through the same
//! elevated commands as a manual apply.
use crate::hp_mode::HpMode;

/// A built-in HP mode or a saved custom preset, as chosen on the dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Mode(HpMode),
    Preset(String),
}
impl Selection {
    /// Stable stored form: `mode:<name>` or `preset:<id>`.
    pub fn key(&self) -> String {
        match self {
            Self::Mode(mode) => format!("mode:{}", mode.name()),
            Self::Preset(id) => format!("preset:{id}"),
        }
    }
    /// Unknown or malformed values read as no selection.
    pub fn parse(stored: &str) -> Option<Self> {
        if let Some(mode) = stored.strip_prefix("mode:") {
            HpMode::parse(mode).ok().map(Self::Mode)
        } else {
            stored
                .strip_prefix("preset:")
                .filter(|id| crate::profiles::validate_id(id).is_ok())
                .map(|id| Self::Preset(id.to_owned()))
        }
    }
}

/// What to apply after Windows reported a power-source change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Switch {
    /// Charger unplugged: apply the battery selection.
    ToBattery(Selection),
    /// Charger back: return to the selection used before unplugging.
    BackToAc(Selection),
}

/// `previous` and `current` are the AC states before and after the event
/// (`None` = Windows could not tell). `ac_selection` is the last selection
/// applied on AC; `switched` says whether the battery selection was applied
/// automatically and not replaced by hand since.
pub fn on_source_change(
    previous: Option<bool>,
    current: Option<bool>,
    battery: Option<&Selection>,
    ac_selection: Option<&Selection>,
    switched: bool,
) -> Option<Switch> {
    match (previous, current) {
        (Some(true), Some(false)) => {
            let battery = battery?;
            // Nothing to do if the battery choice is already active on AC.
            (ac_selection != Some(battery)).then(|| Switch::ToBattery(battery.clone()))
        }
        (Some(false), Some(true)) if switched => Some(Switch::BackToAc(
            // Balanced is the BIOS default mode and the Hub's own restore.
            ac_selection
                .cloned()
                .unwrap_or(Selection::Mode(HpMode::Balanced)),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_keys_round_trip_and_reject_unknown_values() {
        for selection in [
            Selection::Mode(HpMode::Quiet),
            Selection::Mode(HpMode::Unleashed),
            Selection::Preset("preset-abc123".into()),
        ] {
            assert_eq!(Selection::parse(&selection.key()), Some(selection));
        }
        for bad in ["", "mode:turbo", "preset:", "preset:../x", "quiet"] {
            assert_eq!(Selection::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn unplugging_applies_the_battery_choice_and_plugging_in_returns() {
        let battery = Selection::Mode(HpMode::Quiet);
        let ac = Selection::Preset("preset-gaming".into());
        assert_eq!(
            on_source_change(Some(true), Some(false), Some(&battery), Some(&ac), false),
            Some(Switch::ToBattery(battery.clone()))
        );
        assert_eq!(
            on_source_change(Some(false), Some(true), Some(&battery), Some(&ac), true),
            Some(Switch::BackToAc(ac.clone()))
        );
        // Without a known AC choice the BIOS default mode returns.
        assert_eq!(
            on_source_change(Some(false), Some(true), Some(&battery), None, true),
            Some(Switch::BackToAc(Selection::Mode(HpMode::Balanced)))
        );
    }

    #[test]
    fn nothing_changes_without_a_choice_a_real_transition_or_after_a_manual_change() {
        let battery = Selection::Mode(HpMode::Quiet);
        // No battery preset chosen.
        assert_eq!(
            on_source_change(Some(true), Some(false), None, None, false),
            None
        );
        // Battery percentage events and unknown sources are no transition.
        for (before, after) in [
            (Some(false), Some(false)),
            (Some(true), Some(true)),
            (None, Some(false)),
            (Some(true), None),
        ] {
            assert_eq!(
                on_source_change(before, after, Some(&battery), None, true),
                None
            );
        }
        // A preset chosen by hand on battery stays when the charger returns.
        assert_eq!(
            on_source_change(Some(false), Some(true), Some(&battery), None, false),
            None
        );
        // The battery choice is already the active AC choice.
        assert_eq!(
            on_source_change(
                Some(true),
                Some(false),
                Some(&battery),
                Some(&battery),
                false
            ),
            None
        );
    }
}
