//! Adapter-independent preflight, readback and rollback. No partial profile success.
use crate::{
    features::Feature,
    profiles::{DeviceBinding, PerformanceProfile, Setting, Value},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    Unsupported,
    ReadOnly,
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
    fn check(&self, setting: &Setting, context: &Context, startup: bool) -> Result<(), String> {
        if self.feature != setting.feature || self.access != Access::ReadWrite {
            return Err(self.reason.clone());
        }
        if self.requires_ac && context.ac_connected != Some(true) {
            return Err("This operation requires a confirmed AC connection".into());
        }
        if startup && !self.startup_safe {
            return Err("Automatic application is not validated for this feature".into());
        }
        if let Value::Integer(value) = setting.value {
            let (minimum, maximum) = self
                .integer_range
                .ok_or("No hardware numeric limits were obtained")?;
            if minimum > maximum || !(minimum..=maximum).contains(&value) {
                return Err(format!(
                    "Requested value {value} is outside driver/model limits {minimum}..{maximum}"
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
    fn read(&mut self, feature: Feature) -> Result<Value, String>;
    /// Return current observations after every requested control is verified.
    /// Complex adapters can have moving observations with stable programmed data.
    fn verify_target(&mut self, setting: &Setting) -> Result<Value, String> {
        let actual = self.read(setting.feature)?;
        if actual == setting.value {
            Ok(actual)
        } else {
            Err(format!(
                "Readback mismatch for {:?}: requested {:?}, obtained {:?}",
                setting.feature, setting.value, actual
            ))
        }
    }
    /// Capture every field this operation can affect, not only its displayed value.
    fn capture(&mut self, feature: Feature) -> Result<Captured<Self::RestoreState>, String>;
    /// Check every captured control again, including opaque coupled fields. A
    /// displayed sensor observation need not itself be a stable control value.
    fn verify_captured(&mut self, saved: &Captured<Self::RestoreState>) -> Result<(), String> {
        if self.read(saved.feature)? == saved.value {
            Ok(())
        } else {
            Err("Control readback differs from the captured original".into())
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
        Err("This adapter does not support recovery from a persisted image".into())
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
    let mut result = Plan {
        profile_id: profile.id.clone(),
        applicable: false,
        startup,
        capabilities: vec![],
        blockers: vec![],
    };
    if let Err(reason) = profile.validate() {
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
            reason: "Device/BIOS/driver binding differs; revalidation is required".into(),
        });
    }
    if profile.settings.is_empty() {
        result.blockers.push(Blocker {
            feature: None,
            reason: "Profile contains no requested settings".into(),
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
            return Err("Unknown or invalid transaction schema/digest".into());
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
            return Err("Transaction has no complete original image".into());
        }
        for (saved, requested) in self.original.iter().zip(&self.requested) {
            if saved.feature != requested.feature {
                return Err("Transaction original/target features differ".into());
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
        Err("Journal does not support persisted recovery".into())
    }
}

/// Recovery is explicit. Unknown context/images fail before any restore write.
fn checked_write_order(backend: &impl Backend, settings: &[Setting]) -> Result<Vec<usize>, String> {
    let order = backend.write_order(settings);
    let mut checked = order.clone();
    checked.sort_unstable();
    if checked != (0..settings.len()).collect::<Vec<_>>() {
        return Err("Adapter write order omits or duplicates a setting".into());
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
        return Err(fail(
            "Device/BIOS/driver changed; persisted recovery was blocked".into(),
        ));
    }
    backend.prepare_recovery(saved).map_err(fail)?;
    let attempted = checked_write_order(backend, &saved.requested).map_err(fail)?;
    journal.resume_restore(saved).map_err(fail)?;
    let result = rollback(
        backend,
        journal,
        &saved.original,
        &attempted,
        "Recovery incomplete".into(),
    );
    if result.rollback_errors.is_empty() {
        Ok(())
    } else {
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

/// A bounded validation transaction leaves every captured state restored.
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
            return Err(fail("Adapter captured a different feature".into()));
        }
        Setting {
            feature: saved.feature,
            value: saved.value.clone(),
        }
        .validate()
        .map_err(fail)?;
        original.push(saved);
    }
    let transaction = PendingTransaction {
        schema: 1,
        profile_id: profile.id.clone(),
        profile_digest: digest.into(),
        binding: profile.binding.clone(),
        original: original.clone(),
        requested: profile.settings.clone(),
    };
    journal.begin(&transaction).map_err(fail)?;
    let mut attempted = vec![];
    for index in order {
        let setting = &profile.settings[index];
        // Revalidate power, identity and limits immediately before every write.
        let prewrite = (|| {
            let context = backend.context()?;
            if !profile
                .binding
                .for_settings(&profile.settings)
                .matches(&context.binding)
            {
                return Err("Device context changed during application".into());
            }
            backend
                .capability(setting.feature)
                .check(setting, &context, startup)?;
            backend.validate_setting(setting)?;
            backend.verify_captured(&original[index])?;
            Ok(())
        })();
        if let Err(error) = prewrite {
            return Err(rollback(backend, journal, &original, &attempted, error));
        }
        if setting.value == original[index].value {
            continue;
        }
        attempted.push(index);
        let write = backend
            .write(setting)
            .and_then(|()| backend.verify_target(setting).map(|_| ()));
        if let Err(error) = write {
            return Err(rollback(backend, journal, &original, &attempted, error));
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
            return Err("Device context changed before final verification".into());
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
    let mut observed = vec![];
    for setting in &profile.settings {
        match backend.verify_target(setting) {
            Ok(value) => observed.push(Setting {
                feature: setting.feature,
                value,
            }),
            Err(error) => {
                return Err(rollback(
                    backend,
                    journal,
                    &original,
                    &attempted,
                    format!("Final profile: {error}"),
                ));
            }
        }
    }
    if restore_after {
        let restore = rollback(
            backend,
            journal,
            &original,
            &attempted,
            "Trial restore failed".into(),
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
        verified: profile.settings.clone(),
        observed,
        restored_original: restore_after,
    })
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
        disconnect_after_two_writes: bool,
        reject_adapter_target: bool,
        drift_after_capture: bool,
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
            if setting.feature == Feature::GpuMemoryOffsetMhz && setting.value != Value::Integer(0)
            {
                if self.quantize_memory {
                    self.values.insert(setting.feature, Value::Integer(14));
                }
                if self.couple_last {
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
            disconnect_after_two_writes: false,
            reject_adapter_target: false,
            drift_after_capture: false,
            order: None,
        };
        (profile, backend, FakeJournal::default())
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
    fn partial_failure_restores_failing_setter_then_previous_writes() {
        let (profile, mut backend, mut journal) = fixture();
        backend.fail_second = true;
        let error = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap_err();
        assert!(error.rollback_errors.is_empty());
        assert!(backend.values.values().all(|v| *v == Value::Integer(0)));
        assert_eq!(
            backend.writes,
            [
                Feature::GpuCoreOffsetMhz,
                Feature::GpuMemoryOffsetMhz,
                Feature::GpuMemoryOffsetMhz,
                Feature::GpuCoreOffsetMhz
            ]
        );
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
    fn external_drift_before_any_write_closes_the_journal_without_restoring() {
        let (profile, mut backend, mut journal) = fixture();
        backend.drift_after_capture = true;
        let error = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap_err();
        assert!(error.error.contains("differs from the captured original"));
        assert!(error.rollback_errors.is_empty());
        assert!(backend.writes.is_empty());
        assert!(!journal.pending);
        assert_eq!(journal.completed, Some(Completion::Restored));
        assert_eq!(
            backend.values[&Feature::GpuCoreOffsetMhz],
            Value::Integer(7)
        );
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
    fn driver_quantization_is_failure_and_restores_all_attempted_domains() {
        let (profile, mut backend, mut journal) = fixture();
        backend.quantize_memory = true;
        let error = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap_err();
        assert!(error.error.contains("Readback mismatch"));
        assert!(error.rollback_errors.is_empty());
        assert!(backend.values.values().all(|v| *v == Value::Integer(0)));
    }
    #[test]
    fn complete_target_is_rechecked_after_coupled_writes() {
        let (profile, mut backend, mut journal) = fixture();
        backend.couple_last = true;
        let error = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap_err();
        assert!(error.error.contains("Final profile"));
        assert!(error.rollback_errors.is_empty());
        assert!(backend.values.values().all(|v| *v == Value::Integer(0)));
    }
    #[test]
    fn last_write_ac_disconnect_cannot_commit_success() {
        let (profile, mut backend, mut journal) = fixture();
        backend.disconnect_after_two_writes = true;
        let error = apply(&profile, "digest", &mut backend, &mut journal, false).unwrap_err();
        assert!(error.error.contains("AC connection"));
        assert!(error.rollback_errors.is_empty());
        assert_eq!(journal.completed, Some(Completion::Restored));
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
