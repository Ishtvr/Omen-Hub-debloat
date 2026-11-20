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

/// Persisted state of the automatic battery switch (`power-switch.json`).
/// It survives restarts and crashes, so a later plug-in still returns to the
/// AC state saved before unplugging.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    /// Last selection applied on AC: the one to return to.
    pub ac_selection: Option<Selection>,
    /// The battery selection was applied automatically in this DC session.
    pub switched: bool,
    /// The user chose another selection by hand on battery; no automatic
    /// battery switch until the charger is connected again.
    pub manual_override: bool,
}

/// What to apply for the current power source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Switch {
    /// On battery: apply the battery selection.
    ToBattery(Selection),
    /// Charger back: undo the automatic battery preset (`restore` names the
    /// preset whose journal is restored first), then the saved AC selection.
    BackToAc {
        selection: Selection,
        restore: Option<String>,
    },
}

/// Level-based and idempotent: called at startup, after every debounced
/// power-source event and after every resume (Windows may not report a plug
/// change made during sleep). `ac` is the current source; `None` means
/// Windows could not tell, which never switches.
pub fn decide(ac: Option<bool>, battery: Option<&Selection>, state: &State) -> Option<Switch> {
    match ac? {
        false => {
            let battery = battery?;
            if state.switched || state.manual_override {
                return None;
            }
            // The battery choice is already the active AC choice.
            (state.ac_selection.as_ref() != Some(battery))
                .then(|| Switch::ToBattery(battery.clone()))
        }
        true if state.switched || state.manual_override => Some(Switch::BackToAc {
            // Balanced is the BIOS default mode and the Hub's own restore.
            selection: state
                .ac_selection
                .clone()
                .unwrap_or(Selection::Mode(HpMode::Balanced)),
            restore: match battery {
                Some(Selection::Preset(id)) if state.switched => Some(id.clone()),
                _ => None,
            },
        }),
        true => None,
    }
}

impl State {
    /// After an automatic switch completed.
    pub fn switched_to(&mut self, battery: bool) {
        self.switched = battery;
        self.manual_override = false;
    }
    /// After a manual Apply of `selection`; `ac` is the current source.
    pub fn manual(&mut self, selection: &Selection, ac: Option<bool>) {
        if ac == Some(false) {
            // Only an active battery session can be overridden; a selection
            // made on battery is never saved as the AC state.
            self.manual_override = self.switched || self.manual_override;
        } else {
            self.ac_selection = Some(selection.clone());
            self.manual_override = false;
        }
        self.switched = false;
    }
}

/// Minimum quiet time after the last power event before the source is read
/// and acted on: lid, adapter and display events arrive in bursts.
pub const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(750);
#[cfg(test)]
mod tests {
    use super::*;

    fn quiet() -> Selection {
        Selection::Mode(HpMode::Quiet)
    }
    fn gaming() -> Selection {
        Selection::Preset("preset-gaming".into())
    }

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
        let battery = Selection::Preset("preset-battery".into());
        let mut state = State {
            ac_selection: Some(gaming()),
            ..State::default()
        };
        assert_eq!(
            decide(Some(false), Some(&battery), &state),
            Some(Switch::ToBattery(battery.clone()))
        );
        state.switched_to(true);
        // Idempotent: a repeated event or resume on battery does nothing.
        assert_eq!(decide(Some(false), Some(&battery), &state), None);
        assert_eq!(
            decide(Some(true), Some(&battery), &state),
            Some(Switch::BackToAc {
                selection: gaming(),
                restore: Some("preset-battery".into())
            })
        );
        state.switched_to(false);
        assert_eq!(decide(Some(true), Some(&battery), &state), None);
        // Without a known AC choice the BIOS default mode returns; a mode
        // has no journal to restore.
        let state = State {
            switched: true,
            ..State::default()
        };
        assert_eq!(
            decide(Some(true), Some(&quiet()), &state),
            Some(Switch::BackToAc {
                selection: Selection::Mode(HpMode::Balanced),
                restore: None
            })
        );
    }

    #[test]
    fn startup_and_wake_sync_act_on_the_current_level_only() {
        // Started (or woken) on battery after a crash: the saved state decides.
        let state = State {
            ac_selection: Some(gaming()),
            ..State::default()
        };
        assert_eq!(
            decide(Some(false), Some(&quiet()), &state),
            Some(Switch::ToBattery(quiet()))
        );
        // Plugged in during sleep: the next check returns to AC.
        let state = State {
            ac_selection: Some(gaming()),
            switched: true,
            manual_override: false,
        };
        assert!(matches!(
            decide(Some(true), Some(&quiet()), &state),
            Some(Switch::BackToAc { .. })
        ));
        // An unknown source never switches.
        assert_eq!(decide(None, Some(&quiet()), &state), None);
    }

    #[test]
    fn a_manual_choice_on_battery_blocks_the_switch_until_the_charger_returns() {
        let mut state = State {
            ac_selection: Some(gaming()),
            ..State::default()
        };
        state.switched_to(true);
        let other = Selection::Preset("preset-office".into());
        state.manual(&other, Some(false));
        assert!(state.manual_override && !state.switched);
        assert_eq!(
            state.ac_selection,
            Some(gaming()),
            "battery choice is not the AC state"
        );
        assert_eq!(decide(Some(false), Some(&quiet()), &state), None);
        // Charger back: the saved AC state returns; the manual preset left no
        // automatic journal to restore.
        assert_eq!(
            decide(Some(true), Some(&quiet()), &state),
            Some(Switch::BackToAc {
                selection: gaming(),
                restore: None
            })
        );
        state.switched_to(false);
        assert!(!state.manual_override);
        // A manual choice on AC becomes the new AC state.
        state.manual(&other, Some(true));
        assert_eq!(state.ac_selection, Some(other));
    }

    #[test]
    fn nothing_changes_without_a_choice_or_when_it_is_already_active() {
        let state = State::default();
        assert_eq!(decide(Some(false), None, &state), None);
        assert_eq!(decide(Some(true), Some(&quiet()), &state), None);
        let state = State {
            ac_selection: Some(quiet()),
            ..State::default()
        };
        assert_eq!(decide(Some(false), Some(&quiet()), &state), None);
        // A manual choice on battery without an automatic session is no override.
        let mut state = State::default();
        state.manual(&gaming(), Some(false));
        assert!(!state.manual_override);
    }
}
