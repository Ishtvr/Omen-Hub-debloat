//! Adapter-independent preflight, readback and rollback. A normal Apply
//! applies every value it can and returns failed ones to their original;
//! validation trials stay all-or-nothing.
use crate::{
    features::Feature,
    profiles::{DeviceBinding, PerformanceProfile, Setting, Value},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Unsupported,
    NeedsValidation,
    ReadWrite,
}

#[derive(Debug, Clone, Serialize)]
pub struct Capability {
    pub feature: Feature,
    pub access: Access,
    pub reason: String,
    pub integer_range: Option<(i64, i64)>,
    pub requires_ac: bool,
    pub startup_safe: bool,
}

impl Capability {
    pub fn unsupported(feature: Feature, reason: impl Into<String>) -> Self {
        Self {
            feature,
            access: Access::Unsupported,
            reason: reason.into(),
            integer_range: None,
            requires_ac: false,
            startup_safe: false,
        }
    }
    /// Only missing hardware/driver support and values beyond the reported
    /// range refuse a setting (user rule, 30 October 2026). Pending validation,
    /// AC, sign-in use and unknown ranges no longer block: Apply writes and
    /// verifies by readback, and the setting alone is skipped if that fails.
    fn check(&self, setting: &Setting, _context: &Context, _startup: bool) -> Result<(), String> {
        if self.feature != setting.feature || self.access == Access::Unsupported {
            return Err(self.reason.clone());
        }
        if let (Value::Integer(value), Some((minimum, maximum))) =
            (&setting.value, self.integer_range)
        {
            if minimum > maximum || !(minimum..=maximum).contains(value) {
                return Err(crate::message::msg(
                    "transaction.err.bounds",
                    &[value, &minimum, &maximum],
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Context {
    pub binding: DeviceBinding,
    pub ac_connected: Option<bool>,
}

pub trait Backend {
    type RestoreState: Clone;
    fn context(&mut self) -> Result<Context, String>;
    fn capability(&mut self, feature: Feature) -> Capability;
    /// Declare coupled profile intent before capabilities and original capture.
    /// This must only read state; no hardware changes belong in preflight.
    fn prepare_settings(&mut self, _settings: &[Setting]) -> Result<(), String> {
        Ok(())
    }
    /// Indices in hardware dependency order; every setting must occur once.
    /// The journal retains the caller's order and all originals before writes.
    fn write_order(&self, settings: &[Setting]) -> Vec<usize> {
        (0..settings.len()).collect()
    }
    /// Adapter-specific complex values (e.g. the exact V/F grid) also preflight
    /// before any setting in the profile may write hardware.
    fn validate_setting(&mut self, setting: &Setting) -> Result<(), String> {
        setting.validate()
    }
    /// Some controls have a scalar UI value but a multi-value original (AC/DC
    /// policies or all CPU margins). Equality must cover the whole target.
    fn captured_matches_target(
        &mut self,
        saved: &Captured<Self::RestoreState>,
        target: &Setting,
    ) -> Result<bool, String> {
        Ok(saved.value == target.value)
    }
    fn read(&mut self, feature: Feature) -> Result<Value, String>;
    /// Return current observations after every requested control is verified.
    /// Complex adapters can have moving observations with stable programmed data.
    fn verify_target(&mut self, setting: &Setting) -> Result<Value, String> {
        let actual = self.read(setting.feature)?;
        if actual == setting.value {
            Ok(actual)
        } else {
            Err(crate::message::msg(
                "transaction.err.readback",
                &[
                    &format!("{:?}", setting.feature),
                    &format!("{:?}", setting.value),
                    &format!("{actual:?}"),
                ],
            ))
        }
    }
    /// Verify the whole requested set. A partial list of matching values must
    /// never become a successful current-preset or sign-in qualification.
    fn verify_targets(&mut self, settings: &[Setting]) -> Result<Vec<Setting>, String> {
        settings
            .iter()
            .map(|setting| {
                self.verify_target(setting).map(|value| Setting {
                    feature: setting.feature,
                    value,
                })
            })
            .collect()
    }
    /// Capture every field this operation can affect, not only its displayed value.
    fn capture(&mut self, feature: Feature) -> Result<Captured<Self::RestoreState>, String>;
    /// Check every captured control again, including opaque coupled fields. A
    /// displayed sensor observation need not itself be a stable control value.
    fn verify_captured(&mut self, saved: &Captured<Self::RestoreState>) -> Result<(), String> {
        if self.read(saved.feature)? == saved.value {
            Ok(())
        } else {
            Err(crate::message::msg("transaction.err.original_changed", &[]))
        }
    }
    /// Restore and verify the complete adapter-specific image.
    fn restore(&mut self, state: &Captured<Self::RestoreState>) -> Result<(), String>;
    /// Validate persisted restore images and arm only those exact images.
    /// An adapter without a recovery contract must not accept serialized state.
    fn prepare_recovery(
        &mut self,
        _transaction: &PendingTransaction<Self::RestoreState>,
    ) -> Result<(), String> {
        Err(crate::message::msg("transaction.err.adapter_recovery", &[]))
    }
    fn write(&mut self, setting: &Setting) -> Result<(), String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Captured<S> {
    pub feature: Feature,
    pub value: Value,
    pub restore: S,
}

#[derive(Debug, Serialize)]
pub struct Blocker {
    pub feature: Option<Feature>,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct Plan {
    pub profile_id: String,
    pub applicable: bool,
    pub startup: bool,
    pub capabilities: Vec<Capability>,
    pub blockers: Vec<Blocker>,
}

pub fn plan(profile: &PerformanceProfile, backend: &mut impl Backend, startup: bool) -> Plan {
    let profile = &profile.effective();
    let mut result = Plan {
        profile_id: profile.id.clone(),
        applicable: false,
        startup,
        capabilities: vec![],
        blockers: vec![],
    };
    if let Err(reason) = profile.validate_intent() {
        result.blockers.push(Blocker {
            feature: None,
            reason,
        });
        return result;
    }
    if let Err(reason) = backend.prepare_settings(&profile.settings) {
        result.blockers.push(Blocker {
            feature: None,
            reason,
        });
        return result;
    }
    let context = match backend.context() {
        Ok(context) => context,
        Err(reason) => {
            result.blockers.push(Blocker {
                feature: None,
                reason,
            });
            return result;
        }
    };
    if !profile
        .binding
        .for_settings(&profile.settings)
        .matches(&context.binding)
    {
        result.blockers.push(Blocker {
            feature: None,
            reason: crate::message::msg("transaction.err.binding_changed", &[]),
        });
    }
    if profile.settings.is_empty() {
        result.blockers.push(Blocker {
            feature: None,
            reason: crate::message::msg("transaction.err.empty_profile", &[]),
        });
    }
    for setting in &profile.settings {
        let capability = backend.capability(setting.feature);
        if let Err(reason) = capability.check(setting, &context, startup) {
            result.blockers.push(Blocker {
                feature: Some(setting.feature),
                reason,
            });
        } else if let Err(reason) = backend.validate_setting(setting) {
            result.blockers.push(Blocker {
                feature: Some(setting.feature),
                reason,
            });
        }
        result.capabilities.push(capability);
    }
    result.applicable = result.blockers.is_empty();
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingTransaction<S> {
    pub schema: u32,
    pub profile_id: String,
    pub profile_digest: String,
    pub binding: DeviceBinding,
    pub original: Vec<Captured<S>>,
    pub requested: Vec<Setting>,
}

impl<S> PendingTransaction<S> {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 || self.profile_digest.is_empty() || self.profile_digest.len() > 256 {
            return Err(crate::message::msg("transaction.err.schema", &[]));
        }
        PerformanceProfile {
            schema: crate::profiles::PROFILE_SCHEMA,
            id: self.profile_id.clone(),
            name: "Recovery".into(),
            binding: self.binding.clone(),
            settings: self.requested.clone(),
        }
        .validate()?;
        if self.original.is_empty() || self.original.len() != self.requested.len() {
            return Err(crate::message::msg(
                "transaction.err.original_incomplete",
                &[],
            ));
        }
        for (saved, requested) in self.original.iter().zip(&self.requested) {
            if saved.feature != requested.feature {
                return Err(crate::message::msg("transaction.err.feature_mismatch", &[]));
            }
            Setting {
                feature: saved.feature,
                value: saved.value.clone(),
            }
            .validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completion {
    Applied,
    Restored,
}

/// Journal must persist and flush the original state before the first write.
/// It must refuse begin when an earlier incomplete transaction exists.
pub trait Journal<S> {
    fn begin(&mut self, transaction: &PendingTransaction<S>) -> Result<(), String>;
    fn finish(&mut self, completion: Completion) -> Result<(), String>;
    /// Retain the same original image and durably mark recovery as pending.
    fn resume_restore(&mut self, _transaction: &PendingTransaction<S>) -> Result<(), String> {
        Err(crate::message::msg("transaction.err.journal_recovery", &[]))
    }
    /// Replace the originals of the pending transaction (same profile and
    /// requested settings) before the affected setting is written.
    fn update_original(&mut self, _transaction: &PendingTransaction<S>) -> Result<(), String> {
        Err(crate::message::msg("transaction.err.journal_recovery", &[]))
    }
}

/// Recovery is explicit. Unknown context/images fail before any restore write.
fn checked_write_order(backend: &impl Backend, settings: &[Setting]) -> Result<Vec<usize>, String> {
    let order = backend.write_order(settings);
    let mut checked = order.clone();
    checked.sort_unstable();
    if checked != (0..settings.len()).collect::<Vec<_>>() {
        return Err(crate::message::msg("transaction.err.write_order", &[]));
    }
    Ok(order)
}

pub fn recover<B: Backend>(
    saved: &PendingTransaction<B::RestoreState>,
    backend: &mut B,
    journal: &mut impl Journal<B::RestoreState>,
) -> Result<(), ApplyFailure> {
    let fail = |error| ApplyFailure {
        error,
        rollback_errors: vec![],
    };
    saved.validate().map_err(fail)?;
    if !saved
        .binding
        .for_settings(&saved.requested)
        .matches(&backend.context().map_err(fail)?.binding)
    {
        return Err(fail(crate::message::msg(
            "transaction.err.recovery_binding",
            &[],
        )));
    }
    backend.prepare_recovery(saved).map_err(fail)?;
    let attempted = checked_write_order(backend, &saved.requested).map_err(fail)?;
    journal.resume_restore(saved).map_err(fail)?;
    let result = rollback(
        backend,
        journal,
        &saved.original,
        &attempted,
        crate::message::msg("transaction.err.recovery_incomplete", &[]),
    );
    if result.rollback_errors.is_empty() {
        Ok(())
    } else {
        // An explicit Restore reports the values it could not return, but
        // closes the entry instead of leaving every later action blocked.
        let mut result = result;
        if let Err(reason) = journal.finish(Completion::Restored) {
            result.rollback_errors.push(reason);
        }
        Err(result)
    }
}

#[derive(Debug, Serialize)]
pub struct Applied {
    pub profile_id: String,
    pub profile_digest: String,
    pub verified: Vec<Setting>,
    pub observed: Vec<Setting>,
    pub restored_original: bool,
    /// Settings left at (or returned to) their original; the rest applied.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<Skipped>,
    /// Settings that changed outside this transaction before their write;
    /// their live value became the restore original.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub adopted: Vec<Skipped>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skipped {
    pub feature: Feature,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ApplyFailure {
    pub error: String,
    pub rollback_errors: Vec<String>,
}

fn rollback<B: Backend>(
    backend: &mut B,
    journal: &mut impl Journal<B::RestoreState>,
    original: &[Captured<B::RestoreState>],
    attempted: &[usize],
    error: String,
) -> ApplyFailure {
    let mut rollback_errors = vec![];
    // Includes the setter that failed: a failing driver call can still mutate state.
    for index in attempted.iter().rev() {
        let setting = &original[*index];
        let restored: Result<(), String> = (|| {
            backend.restore(setting)?;
            backend.verify_captured(setting)
        })();
        if let Err(reason) = restored {
            rollback_errors.push(format!("{:?}: {reason}", setting.feature));
        }
    }
    // Restoring a coupled setting can undo an earlier restore. Verify all final
    // values, including unchanged settings; never erase an incomplete journal.
    // Without any write attempt, a difference is external drift (already the
    // failure reason): keeping the journal pending would block every later apply
    // and let recovery overwrite a change this transaction never made.
    if !attempted.is_empty() {
        for saved in original {
            if let Err(reason) = backend.verify_captured(saved) {
                rollback_errors.push(format!("{:?}: {reason}", saved.feature));
            }
        }
    }
    // An incomplete rollback keeps the original journal and disables startup retries.
    if rollback_errors.is_empty() {
        if let Err(reason) = journal.finish(Completion::Restored) {
            rollback_errors.push(reason);
        }
    }
    ApplyFailure {
        error,
        rollback_errors,
    }
}

pub fn apply<B: Backend>(
    profile: &PerformanceProfile,
    digest: &str,
    backend: &mut B,
    journal: &mut impl Journal<B::RestoreState>,
    startup: bool,
) -> Result<Applied, ApplyFailure> {
    apply_inner(profile, digest, backend, journal, startup, false)
}

/// A bounded validation transaction exercises setters even for an equal target,
/// verifies readback and leaves every captured state restored. Normal apply
/// still skips settings whose full target already matches.
pub fn trial<B: Backend>(
    profile: &PerformanceProfile,
    digest: &str,
    backend: &mut B,
    journal: &mut impl Journal<B::RestoreState>,
) -> Result<Applied, ApplyFailure> {
    apply_inner(profile, digest, backend, journal, false, true)
}

fn apply_inner<B: Backend>(
    profile: &PerformanceProfile,
    digest: &str,
    backend: &mut B,
    journal: &mut impl Journal<B::RestoreState>,
    startup: bool,
    restore_after: bool,
) -> Result<Applied, ApplyFailure> {
    // The journal records exactly the settings this transaction writes.
    let profile = &profile.effective();
    let fail = |error| ApplyFailure {
        error,
        rollback_errors: vec![],
    };
    let checked = plan(profile, backend, startup);
    if !checked.applicable {
        return Err(fail(
            checked
                .blockers
                .iter()
                .map(|b| b.reason.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    let order = checked_write_order(backend, &profile.settings).map_err(fail)?;
    let mut original = vec![];
    for setting in &profile.settings {
        let saved = backend.capture(setting.feature).map_err(fail)?;
        if saved.feature != setting.feature {
            return Err(fail(crate::message::msg(
                "transaction.err.capture_mismatch",
                &[],
            )));
        }
        Setting {
            feature: saved.feature,
            value: saved.value.clone(),
        }
        .validate()
        .map_err(fail)?;
        original.push(saved);
    }
    let mut transaction = PendingTransaction {
        schema: 1,
        profile_id: profile.id.clone(),
        profile_digest: digest.into(),
        binding: profile.binding.clone(),
        original: original.clone(),
        requested: profile.settings.clone(),
    };
    journal.begin(&transaction).map_err(fail)?;
    // A normal Apply does not stop at one value (user rule, 30 October 2026):
    // a setting that fails is returned to its original and reported, the
    // others stay applied. Validation trials keep all-or-nothing.
    let lenient = !restore_after;
    let mut skipped = vec![];
    let mut adopted = vec![];
    let mut attempted = vec![];
    for index in order {
        let setting = &profile.settings[index];
        // Revalidate power, identity and limits immediately before every write.
        // A different device always stops the whole preset.
        let context = match backend.context() {
            Ok(context) => context,
            Err(error) => return Err(rollback(backend, journal, &original, &attempted, error)),
        };
        if !profile
            .binding
            .for_settings(&profile.settings)
            .matches(&context.binding)
        {
            let error = crate::message::msg("transaction.err.context_changed", &[]);
            return Err(rollback(backend, journal, &original, &attempted, error));
        }
        let checked = backend
            .capability(setting.feature)
            .check(setting, &context, startup)
            .and_then(|()| backend.validate_setting(setting));
        if let Err(error) = checked {
            if !lenient {
                return Err(rollback(backend, journal, &original, &attempted, error));
            }
            skipped.push(Skipped {
                feature: setting.feature,
                reason: error,
            });
            continue;
        }
        if let Err(error) = backend.verify_captured(&original[index]) {
            if !lenient {
                return Err(rollback(backend, journal, &original, &attempted, error));
            }
            // Changed outside this transaction (firmware, user): the live value
            // becomes the original Restore returns to; the preset still applies.
            let fresh = match backend.capture(setting.feature) {
                Ok(fresh) if fresh.feature == setting.feature => fresh,
                Ok(_) => {
                    let error = crate::message::msg("transaction.err.capture_mismatch", &[]);
                    return Err(rollback(backend, journal, &original, &attempted, error));
                }
                Err(reason) => {
                    skipped.push(Skipped {
                        feature: setting.feature,
                        reason,
                    });
                    continue;
                }
            };
            original[index] = fresh;
            transaction.original = original.clone();
            if let Err(reason) = journal.update_original(&transaction) {
                return Err(rollback(backend, journal, &original, &attempted, reason));
            }
            adopted.push(Skipped {
                feature: setting.feature,
                reason: error,
            });
        }
        let unchanged = match backend.captured_matches_target(&original[index], setting) {
            Ok(value) => value,
            Err(error) => return Err(rollback(backend, journal, &original, &attempted, error)),
        };
        if unchanged && !restore_after {
            continue;
        }
        attempted.push(index);
        let mut write = || {
            backend
                .write(setting)
                .and_then(|()| backend.verify_target(setting).map(|_| ()))
        };
        // One more write for a value the firmware took over late.
        let result = match write() {
            Err(_) if lenient => write(),
            result => result,
        };
        if let Err(error) = result {
            if !lenient {
                return Err(rollback(backend, journal, &original, &attempted, error));
            }
            if let Err(reason) = restore_one(backend, &original[index]) {
                // This value cannot return to its original: full rollback, and
                // the journal stays pending for recovery if that fails too.
                let error = format!("{error}; {reason}");
                return Err(rollback(backend, journal, &original, &attempted, error));
            }
            attempted.retain(|i| *i != index);
            skipped.push(Skipped {
                feature: setting.feature,
                reason: error,
            });
        }
    }
    // An AC disconnect or driver/context change during the last write must not
    // result in a committed profile that no longer meets its preconditions.
    let final_context = (|| {
        let context = backend.context()?;
        if !profile
            .binding
            .for_settings(&profile.settings)
            .matches(&context.binding)
        {
            return Err(crate::message::msg("transaction.err.context_final", &[]));
        }
        for setting in &profile.settings {
            backend
                .capability(setting.feature)
                .check(setting, &context, startup)?;
        }
        Ok(())
    })();
    if let Err(error) = final_context {
        return Err(rollback(backend, journal, &original, &attempted, error));
    }
    // A later operation can affect an earlier setting. Verify the complete target.
    for (index, setting) in profile.settings.iter().enumerate() {
        if skipped
            .iter()
            .any(|s: &Skipped| s.feature == setting.feature)
        {
            continue;
        }
        match backend.verify_target(setting) {
            Ok(_) => {}
            Err(error) if lenient => {
                // A later write moved this value: write it once more.
                // Even a target skipped as initially equal can mutate here;
                // include it in every subsequent rollback before the setter.
                if !attempted.contains(&index) {
                    attempted.push(index);
                }
                if backend
                    .write(setting)
                    .and_then(|()| backend.verify_target(setting))
                    .is_ok()
                {
                    continue;
                }
                if let Err(reason) = restore_one(backend, &original[index]) {
                    let error = format!("{error}; {reason}");
                    return Err(rollback(backend, journal, &original, &attempted, error));
                }
                attempted.retain(|i| *i != index);
                skipped.push(Skipped {
                    feature: setting.feature,
                    reason: error,
                });
            }
            Err(error) => {
                return Err(rollback(
                    backend,
                    journal,
                    &original,
                    &attempted,
                    format!(
                        "{} {error}",
                        crate::message::msg("transaction.err.final_profile", &[])
                    ),
                ));
            }
        }
    }
    // Correcting a later target (or restoring a skipped one) can invalidate
    // an earlier observation. This final pass is read-only and bounded: no
    // mutually coupled setters can produce a false successful Apply.
    let verified: Vec<_> = profile
        .settings
        .iter()
        .filter(|setting| !skipped.iter().any(|s| s.feature == setting.feature))
        .cloned()
        .collect();
    let observed = match backend.verify_targets(&verified) {
        Ok(observed) => observed,
        Err(error) => return Err(rollback(backend, journal, &original, &attempted, error)),
    };
    for saved in &original {
        if skipped.iter().any(|s| s.feature == saved.feature) {
            if let Err(error) = backend.verify_captured(saved) {
                return Err(rollback(backend, journal, &original, &attempted, error));
            }
        }
    }
    if restore_after {
        let restore = rollback(
            backend,
            journal,
            &original,
            &attempted,
            crate::message::msg("transaction.err.trial_restore", &[]),
        );
        if !restore.rollback_errors.is_empty() {
            return Err(restore);
        }
    } else if let Err(error) = journal.finish(Completion::Applied) {
        return Err(rollback(backend, journal, &original, &attempted, error));
    }
    Ok(Applied {
        profile_id: profile.id.clone(),
        profile_digest: digest.into(),
        verified,
        observed,
        restored_original: restore_after,
        skipped,
        adopted,
    })
}

/// Return one setting to its captured original and verify it.
fn restore_one<B: Backend>(
    backend: &mut B,
    saved: &Captured<B::RestoreState>,
) -> Result<(), String> {
    backend.restore(saved)?;
    backend.verify_captured(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::PROFILE_SCHEMA;
    use std::collections::BTreeMap;

    struct FakeBackend {
        values: BTreeMap<Feature, Value>,
        writes: Vec<Feature>,
        fail_second: bool,
        fail_restore: bool,
        quantize_memory: bool,
        couple_last: bool,
        couple_both: bool,
        disconnect_after_two_writes: bool,
        reject_adapter_target: bool,
        drift_after_capture: bool,
        require_full_target_write: bool,
        order: Option<Vec<usize>>,
    }
    fn binding() -> DeviceBinding {
        DeviceBinding {
            manufacturer: "Test".into(),
            board: "board".into(),
            bios: "bios".into(),
            cpu_names: vec![],
            nvidia_uuid: Some("GPU-test".into()),
            nvidia_driver: Some("test-driver".into()),
            nvidia_vbios: Some("test-vbios".into()),
        }
    }
    impl Backend for FakeBackend {
        type RestoreState = Value;
        fn write_order(&self, settings: &[Setting]) -> Vec<usize> {
            self.order
                .clone()
                .unwrap_or_else(|| (0..settings.len()).collect())
        }
        fn context(&mut self) -> Result<Context, String> {
            Ok(Context {
                binding: binding(),
                ac_connected: Some(!(self.disconnect_after_two_writes && self.writes.len() >= 2)),
            })
        }
        fn capability(&mut self, feature: Feature) -> Capability {
            if matches!(
                feature,
                Feature::GpuCoreOffsetMhz | Feature::GpuMemoryOffsetMhz
            ) {
                Capability {
                    feature,
                    access: Access::ReadWrite,
                    reason: "test adapter only".into(),
                    integer_range: Some((-100, 100)),
                    requires_ac: true,
                    startup_safe: true,
                }
            } else {
                Capability::unsupported(feature, "test adapter does not implement this")
            }
        }
        fn read(&mut self, feature: Feature) -> Result<Value, String> {
            self.values
                .get(&feature)
                .cloned()
                .ok_or("missing test state".into())
        }
        fn validate_setting(&mut self, setting: &Setting) -> Result<(), String> {
            setting.validate()?;
            if self.reject_adapter_target && setting.feature == Feature::GpuMemoryOffsetMhz {
                return Err(
                    "adapter rejects the target independently of advertised capability".into(),
                );
            }
            Ok(())
        }
        fn capture(&mut self, feature: Feature) -> Result<Captured<Value>, String> {
            let value = self.read(feature)?;
            if self.drift_after_capture && feature == Feature::GpuMemoryOffsetMhz {
                self.values
                    .insert(Feature::GpuCoreOffsetMhz, Value::Integer(7));
            }
            Ok(Captured {
                feature,
                restore: value.clone(),
                value,
            })
        }
        fn captured_matches_target(
            &mut self,
            saved: &Captured<Value>,
            target: &Setting,
        ) -> Result<bool, String> {
            // Models an adapter whose displayed scalar covers only one of
            // several channels, such as AC/DC or individual CPU cores.
            Ok(!self.require_full_target_write && saved.value == target.value)
        }
        fn restore(&mut self, state: &Captured<Value>) -> Result<(), String> {
            self.write(&Setting {
                feature: state.feature,
                value: state.restore.clone(),
            })
        }
        fn prepare_recovery(&mut self, saved: &PendingTransaction<Value>) -> Result<(), String> {
            saved.validate()?;
            if saved.original.iter().any(|s| s.value != s.restore) {
                return Err("test restore image differs".into());
            }
            Ok(())
        }
        fn write(&mut self, setting: &Setting) -> Result<(), String> {
            self.writes.push(setting.feature);
            if self.fail_restore && setting.value == Value::Integer(0) {
                return Err("restore failure".into());
            }
            self.values.insert(setting.feature, setting.value.clone());
            if self.couple_both
                && setting.feature == Feature::GpuCoreOffsetMhz
                && setting.value != Value::Integer(0)
            {
                self.values
                    .insert(Feature::GpuMemoryOffsetMhz, Value::Integer(99));
            }
            if setting.feature == Feature::GpuMemoryOffsetMhz && setting.value != Value::Integer(0)
            {
                if self.quantize_memory {
                    self.values.insert(setting.feature, Value::Integer(14));
                }
                if self.couple_last || self.couple_both {
                    self.values
                        .insert(Feature::GpuCoreOffsetMhz, Value::Integer(99));
                }
            }
            if self.fail_second
                && setting.feature == Feature::GpuMemoryOffsetMhz
                && setting.value != Value::Integer(0)
            {
                return Err("failure after mutation".into());
            }
            Ok(())
        }
    }
    #[derive(Default)]
    struct FakeJournal {
        pending: bool,
        completed: Option<Completion>,
        fail_begin: bool,
        saved: Option<PendingTransaction<Value>>,
    }
    impl Journal<Value> for FakeJournal {
        fn begin(&mut self, saved: &PendingTransaction<Value>) -> Result<(), String> {
            if self.pending || self.fail_begin {
                return Err("journal unavailable".into());
            }
            self.pending = true;
            self.saved = Some(saved.clone());
            Ok(())
        }
        fn resume_restore(&mut self, saved: &PendingTransaction<Value>) -> Result<(), String> {
            if self.saved.as_ref() != Some(saved) {
                return Err("test journal differs".into());
            }
            self.pending = true;
            self.completed = None;
            Ok(())
        }
        fn finish(&mut self, completion: Completion) -> Result<(), String> {
            self.pending = false;
            self.completed = Some(completion);
            Ok(())
        }
        fn update_original(&mut self, saved: &PendingTransaction<Value>) -> Result<(), String> {
            if !self.pending {
                return Err("test journal inactive".into());
            }
            self.saved = Some(saved.clone());
            Ok(())
        }
    }
    fn fixture() -> (PerformanceProfile, FakeBackend, FakeJournal) {
        let features = [Feature::GpuCoreOffsetMhz, Feature::GpuMemoryOffsetMhz];
        let profile = PerformanceProfile {
            schema: PROFILE_SCHEMA,
            id: "test".into(),
            name: "Test".into(),
            binding: binding(),
            settings: features
                .iter()
                .map(|f| Setting {
                    feature: *f,
                    value: Value::Integer(15),
                })
                .collect(),
        };
        let backend = FakeBackend {
            values: features.iter().map(|f| (*f, Value::Integer(0))).collect(),
            writes: vec![],
            fail_second: false,
            fail_restore: false,
            quantize_memory: false,
            couple_last: false,
            couple_both: false,
            disconnect_after_two_writes: false,
            reject_adapter_target: false,
            drift_after_capture: false,
            require_full_target_write: false,
            order: None,
        };
        (profile, backend, FakeJournal::default())
    }
    #[test]
    fn equal_displayed_scalar_does_not_skip_an_adapters_full_target() {
        let (mut profile, mut backend, mut journal) = fixture();
        profile.settings.truncate(1);
        profile.settings[0].value = Value::Integer(0);
        backend.require_full_target_write = true;
        let result = apply(&profile, "full-target", &mut backend, &mut journal, false).unwrap();
        assert!(!result.restored_original);
        assert_eq!(backend.writes.len(), 1);
        assert_eq!(journal.completed, Some(Completion::Applied));
    }
    #[test]
    fn qualification_exercises_an_idempotent_setter_but_normal_apply_skips_it() {
        let (mut profile, mut backend, mut journal) = fixture();
        profile.settings.truncate(1);
        profile.settings[0].value = Value::Integer(0);
        apply(&profile, "same-target", &mut backend, &mut journal, false).unwrap();
        assert!(backend.writes.is_empty());
        trial(&profile, "same-target-trial", &mut backend, &mut journal).unwrap();
        assert_eq!(backend.writes.len(), 2);
        assert_eq!(journal.completed, Some(Completion::Restored));
    }
    #[test]
    fn all_features_preflight_before_any_write() {
        let (mut profile, mut backend, mut journal) = fixture();
        profile.settings.push(Setting {
            feature: Feature::GpuMemoryVoltageOffsetMv,
            value: Value::Integer(-10),
        });
        assert!(apply(&profile, "digest", &mut backend, &mut journal, false).is_err());
        assert!(backend.writes.is_empty());
        assert!(!journal.pending);
    }
    #[test]
    fn retired_intel_intent_blocks_the_whole_profile_before_any_journal_or_write() {
        let (mut profile, mut backend, mut journal) = fixture();
        profile.settings.push(
            serde_json::from_str(
                r#"{"feature":"intel_pl1_mw","value":{"kind":"integer","value":45000}}"#,
            )
            .unwrap(),
        );
        profile.validate().unwrap();
        let error = apply(&profile, "legacy", &mut backend, &mut journal, false).unwrap_err();
        assert!(error.error.contains("profile.err.removed_intel"));
        assert!(backend.writes.is_empty());
        assert!(journal.saved.is_none());
    }
    #[test]
    fn hardware_specific_target_validation_blocks_all_domains_before_journaling() {
        let (profile, mut backend, mut journal) = fixture();
        backend.reject_adapter_target = true;
        let error = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap_err();
        assert!(error.error.contains("adapter rejects"));
        assert!(backend.writes.is_empty());
        assert!(journal.saved.is_none());
    }
    #[test]
    fn adapter_dependencies_change_write_and_rollback_order_without_losing_journal_values() {
        let (profile, mut backend, mut journal) = fixture();
        backend.order = Some(vec![1, 0]);
        trial(&profile, "digest", &mut backend, &mut journal).unwrap();
        assert_eq!(
            backend.writes,
            [
                Feature::GpuMemoryOffsetMhz,
                Feature::GpuCoreOffsetMhz,
                Feature::GpuCoreOffsetMhz,
                Feature::GpuMemoryOffsetMhz
            ]
        );
        assert_eq!(journal.saved.unwrap().requested, profile.settings);
        assert!(backend.values.values().all(|v| *v == Value::Integer(0)));
    }
    #[test]
    fn omitted_duplicate_or_out_of_range_adapter_indices_never_reach_hardware() {
        for order in [vec![0], vec![0, 0], vec![0, 2]] {
            let (profile, mut backend, mut journal) = fixture();
            backend.order = Some(order);
            assert!(apply(&profile, "digest", &mut backend, &mut journal, false).is_err());
            assert!(backend.writes.is_empty());
            assert!(journal.saved.is_none());
        }
    }
    #[test]
    fn persisted_recovery_uses_adapter_dependencies_and_verifies_the_whole_original() {
        let (profile, mut backend, mut journal) = fixture();
        backend.order = Some(vec![1, 0]);
        apply(&profile, "digest", &mut backend, &mut journal, false).unwrap();
        let saved = journal.saved.clone().unwrap();
        backend.writes.clear();
        recover(&saved, &mut backend, &mut journal).unwrap();
        assert_eq!(
            backend.writes,
            [Feature::GpuCoreOffsetMhz, Feature::GpuMemoryOffsetMhz]
        );
        assert!(backend.values.values().all(|v| *v == Value::Integer(0)));
    }
    #[test]
    fn a_failing_setter_returns_to_its_original_and_the_rest_stays_applied() {
        let (profile, mut backend, mut journal) = fixture();
        backend.fail_second = true;
        let applied = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap();
        assert_eq!(applied.skipped.len(), 1);
        assert_eq!(applied.skipped[0].feature, Feature::GpuMemoryOffsetMhz);
        assert_eq!(applied.verified, profile.settings[..1]);
        assert_eq!(
            backend.values[&Feature::GpuCoreOffsetMhz],
            Value::Integer(15)
        );
        assert_eq!(
            backend.values[&Feature::GpuMemoryOffsetMhz],
            Value::Integer(0)
        );
        // Write, one more write, then the restore of only that setting.
        assert_eq!(
            backend.writes,
            [
                Feature::GpuCoreOffsetMhz,
                Feature::GpuMemoryOffsetMhz,
                Feature::GpuMemoryOffsetMhz,
                Feature::GpuMemoryOffsetMhz
            ]
        );
        assert_eq!(journal.completed, Some(Completion::Applied));
        // Validation trials stay all-or-nothing.
        let (profile, mut backend, mut journal) = fixture();
        backend.fail_second = true;
        let error = trial(&profile, "digest", &mut backend, &mut journal).unwrap_err();
        assert!(error.rollback_errors.is_empty());
        assert!(backend.values.values().all(|v| *v == Value::Integer(0)));
        assert_eq!(journal.completed, Some(Completion::Restored));
    }
    #[test]
    fn failed_restore_leaves_journal_pending_for_recovery() {
        let (profile, mut backend, mut journal) = fixture();
        backend.fail_second = true;
        backend.fail_restore = true;
        let error = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap_err();
        assert!(error.rollback_errors.len() >= 2);
        assert!(journal.pending);
        assert!(journal.completed.is_none());
    }
    #[test]
    fn external_drift_becomes_the_restore_original_and_the_preset_still_applies() {
        let (profile, mut backend, mut journal) = fixture();
        backend.drift_after_capture = true;
        let applied = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap();
        assert!(applied.skipped.is_empty());
        assert_eq!(applied.adopted.len(), 1);
        assert_eq!(applied.adopted[0].feature, Feature::GpuCoreOffsetMhz);
        assert!(backend.values.values().all(|v| *v == Value::Integer(15)));
        assert_eq!(journal.completed, Some(Completion::Applied));
        let saved = journal.saved.clone().unwrap();
        assert_eq!(saved.original[0].value, Value::Integer(7));
        // Restore returns to the adopted value, not to the stale capture.
        recover(&saved, &mut backend, &mut journal).unwrap();
        assert_eq!(
            backend.values[&Feature::GpuCoreOffsetMhz],
            Value::Integer(7)
        );
        // A trial still refuses a control that changed under it.
        let (profile, mut backend, mut journal) = fixture();
        backend.drift_after_capture = true;
        let error = trial(&profile, "digest", &mut backend, &mut journal).unwrap_err();
        assert!(error.error.contains("transaction.err.original_changed"));
        assert!(backend.writes.is_empty());
    }
    #[test]
    fn no_hardware_write_without_durable_original_snapshot() {
        let (profile, mut backend, mut journal) = fixture();
        journal.fail_begin = true;
        assert!(apply(&profile, "digest", &mut backend, &mut journal, false).is_err());
        assert!(backend.writes.is_empty());
    }
    #[test]
    fn bios_changes_and_duplicate_settings_cannot_apply() {
        let (mut profile, mut backend, mut journal) = fixture();
        profile.binding.bios = "new".into();
        assert!(apply(&profile, "digest", &mut backend, &mut journal, false).is_err());
        profile.binding = binding();
        profile.settings.push(profile.settings[0].clone());
        assert!(apply(&profile, "digest", &mut backend, &mut journal, false).is_err());
        assert!(backend.writes.is_empty());
    }
    #[test]
    fn driver_quantization_skips_that_domain_and_keeps_the_others() {
        let (profile, mut backend, mut journal) = fixture();
        backend.quantize_memory = true;
        let applied = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap();
        assert_eq!(applied.skipped.len(), 1);
        assert!(
            applied.skipped[0]
                .reason
                .contains("transaction.err.readback")
        );
        assert_eq!(
            backend.values[&Feature::GpuCoreOffsetMhz],
            Value::Integer(15)
        );
        assert_eq!(
            backend.values[&Feature::GpuMemoryOffsetMhz],
            Value::Integer(0)
        );
        let (profile, mut backend, mut journal) = fixture();
        backend.quantize_memory = true;
        let error = trial(&profile, "digest", &mut backend, &mut journal).unwrap_err();
        assert!(error.error.contains("transaction.err.readback"));
        assert!(backend.values.values().all(|v| *v == Value::Integer(0)));
    }
    #[test]
    fn complete_target_is_rechecked_after_coupled_writes() {
        let (profile, mut backend, mut journal) = fixture();
        backend.couple_last = true;
        let applied = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap();
        // The coupled write moved core away; the final check writes it again.
        assert!(applied.skipped.is_empty());
        assert!(backend.values.values().all(|v| *v == Value::Integer(15)));
        let (profile, mut backend, mut journal) = fixture();
        backend.couple_last = true;
        let error = trial(&profile, "digest", &mut backend, &mut journal).unwrap_err();
        assert!(error.error.contains("transaction.err.final_profile"));
        assert!(error.rollback_errors.is_empty());
        assert!(backend.values.values().all(|v| *v == Value::Integer(0)));
    }
    #[test]
    fn a_partial_readback_cannot_verify_the_whole_preset() {
        let (profile, mut backend, _) = fixture();
        backend
            .values
            .insert(Feature::GpuCoreOffsetMhz, Value::Integer(15));
        assert!(backend.verify_targets(&profile.settings).is_err());
        backend
            .values
            .insert(Feature::GpuMemoryOffsetMhz, Value::Integer(15));
        assert_eq!(
            backend.verify_targets(&profile.settings).unwrap(),
            profile.settings
        );
    }
    #[test]
    fn corrective_writes_cannot_commit_stale_earlier_readbacks() {
        let (profile, mut backend, mut journal) = fixture();
        backend.couple_both = true;
        assert!(apply(&profile, "coupled", &mut backend, &mut journal, false).is_err());
        assert_ne!(journal.completed, Some(Completion::Applied));
        // This is a bounded correction, never an endless fight with firmware.
        assert!(backend.writes.len() <= 8);
    }
    #[test]
    fn an_ac_disconnect_during_apply_no_longer_undoes_the_preset() {
        // AC is no longer a precondition (user rule, 30 October 2026).
        let (profile, mut backend, mut journal) = fixture();
        backend.disconnect_after_two_writes = true;
        let applied = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap();
        assert!(applied.skipped.is_empty());
        assert_eq!(journal.completed, Some(Completion::Applied));
    }
    #[test]
    fn trial_verifies_target_but_leaves_original_state() {
        let (profile, mut backend, mut journal) = fixture();
        let result = trial(&profile, "digest", &mut backend, &mut journal).unwrap();
        assert!(result.restored_original);
        assert_eq!(result.verified, profile.settings);
        assert!(backend.values.values().all(|v| *v == Value::Integer(0)));
        assert_eq!(journal.completed, Some(Completion::Restored));
    }
    #[test]
    fn persisted_restore_rejects_incomplete_images_before_writing() {
        let (profile, mut backend, mut journal) = fixture();
        apply(&profile, "digest", &mut backend, &mut journal, false).unwrap();
        let mut saved = journal.saved.clone().unwrap();
        backend.writes.clear();
        saved.original.pop();
        assert!(recover(&saved, &mut backend, &mut journal).is_err());
        assert!(backend.writes.is_empty());
        let saved = journal.saved.clone().unwrap();
        recover(&saved, &mut backend, &mut journal).unwrap();
        assert!(backend.values.values().all(|v| *v == Value::Integer(0)));
        assert_eq!(journal.completed, Some(Completion::Restored));
    }
}
