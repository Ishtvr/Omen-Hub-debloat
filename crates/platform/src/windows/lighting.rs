//! Direct four-zone controls, independently journaled from performance profiles.
//! Reuse the qualified RGB and 64/E4 contracts, never an arbitrary BIOS writer.
use super::{ControlLease, Controller, RestoreState};
use crate::profile_store::{ProfileStore, profile_digest};
use omen_core::{
    features::Feature,
    keyboard::KeyboardState,
    profiles::{Choice, PROFILE_SCHEMA, PerformanceProfile, Setting, Value},
    transaction::{self, Backend},
};

pub struct DirectLighting {
    store: ProfileStore,
}
impl DirectLighting {
    pub fn new() -> Result<Self, String> {
        let user = ProfileStore::user_data()?;
        Self::with_store(ProfileStore::at(user.root().join("keyboard-control"))?)
    }
    /// An isolated store is also useful for bounded hardware qualification.
    pub fn with_store(store: ProfileStore) -> Result<Self, String> {
        Ok(Self { store })
    }
    fn recover_owned(&mut self, controller: &mut Controller) -> Result<(), String> {
        if self.store.pending()? {
            if ProfileStore::user_data()?.pending()? {
                return Err("An incomplete performance transaction requires recovery first".into());
            }
            let pending = self.store.last_transaction::<RestoreState>()?;
            if pending.profile_id != "keyboard-direct"
                || pending.requested.iter().any(|s| {
                    !matches!(s.feature, Feature::KeyboardColors | Feature::KeyboardEffect)
                })
                || pending
                    .original
                    .iter()
                    .any(|s| !matches!(s.restore, RestoreState::HpKeyboardColors { .. }))
            {
                return Err(
                    "Lighting recovery contains unrelated controls; no write dispatched".into(),
                );
            }
            transaction::recover(&pending, controller, &mut self.store).map_err(|e| e.error)?;
        }
        Ok(())
    }
    pub fn read(&mut self) -> Result<KeyboardState, String> {
        let _lease = ControlLease::acquire()?;
        let mut controller = Controller::new(false, None)?;
        self.recover_owned(&mut controller)?;
        let state = controller.keyboard_state()?;
        if state.capability.raw != [1, 0, 0, 0] {
            return Err(
                "Direct lighting is qualified only for the measured four-zone keyboard".into(),
            );
        }
        Ok(state)
    }
    pub fn apply(
        &mut self,
        expected: &KeyboardState,
        colors: &[[u8; 3]; 4],
        enabled: bool,
    ) -> Result<KeyboardState, String> {
        expected.validate()?;
        let _lease = ControlLease::acquire()?;
        // Never bypass an unresolved transaction belonging to the main engine.
        if ProfileStore::user_data()?.pending()? {
            return Err("An incomplete performance transaction requires recovery first".into());
        }
        let mut controller = Controller::new(false, None)?;
        self.recover_owned(&mut controller)?;
        if controller.keyboard_state()? != *expected {
            return Err("Lighting changed outside this window; refresh and choose again".into());
        }
        let settings = vec![
            Setting {
                feature: Feature::KeyboardColors,
                value: Value::Colors(colors.to_vec()),
            },
            // Off uses the already qualified64-byte mode. The old serialized
            // firmware_default name remains accepted for existing journals.
            Setting {
                feature: Feature::KeyboardEffect,
                value: Value::Choice(if enabled { Choice::Static } else { Choice::Off }),
            },
        ];
        let profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "keyboard-direct".into(),
            name: "Beleuchtung".into(),
            binding: controller.context()?.binding.for_settings(&settings),
            settings,
        };
        self.store.save(&profile)?;
        let digest = profile_digest(&profile)?;
        transaction::apply(&profile, &digest, &mut controller, &mut self.store, false)
            .map_err(|e| e.error)?;
        let actual = controller.keyboard_state()?;
        let target = expected.with_colors(colors)?.with_classic_mode(enabled)?;
        if actual != target {
            return Err(
                "Lighting state changed after verification; reload before another edit".into(),
            );
        }
        Ok(actual)
    }
}
