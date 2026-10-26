//! Fixed own-helper cooling responsibility for the bounded qualification.
//! No HP service is a fan transport dependency; only the temporary migration
//! session has an OEM return duty. Ordinary fan operation does not use this file.
use super::*;
use crate::windows::fan_task;
use crate::{
    cooling_store::{CoolingJournal, bounded_bytes},
    profile_store,
};
use omen_core::cooling_override::{
    self as policy, Acknowledgement, Checkpoint, MaximumAction, Mode, Phase, RecoveryReadiness,
    RestoreAction,
};

pub(super) struct Cooling {
    journal: CoolingJournal,
    preference: PathBuf,
}
pub(super) fn now_ms() -> Result<u64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis()
        .try_into()
        .map_err(|_| "Cooling timestamp is out of range".into())
}
impl Cooling {
    pub(super) fn open(current_build: bool) -> Result<Self, String> {
        let installation = super::super::startup::inspect_installed_binary(current_build)?;
        Ok(Self {
            journal: CoolingJournal::at(
                installation
                    .executable
                    .parent()
                    .ok_or("Protected cooling directory absent")?
                    .to_path_buf(),
            )?,
            preference: super::super::profile_directory()?.join("fan-mode.json"),
        })
    }
    pub(super) fn load(&self) -> Result<Option<Checkpoint>, String> {
        self.journal.load()
    }
    fn preference(&self) -> Result<Option<Vec<u8>>, String> {
        bounded_bytes(&self.preference, policy::MAX_PREFERENCE_BYTES)
    }
    fn record(&self, owner: &Owner, identity: &Identity) -> Result<Checkpoint, String> {
        let record = self
            .load()?
            .ok_or("Cooling responsibility record is absent")?;
        if &record.owner != owner || &record.original_service.identity != identity {
            return Err("Cooling responsibility belongs to another session".into());
        }
        Ok(record)
    }
    fn preference_lock(&self) -> Result<std::fs::File, String> {
        profile_store::lock_fan_preference(
            self.preference
                .parent()
                .ok_or("Fan preference directory absent")?,
        )
    }
    pub(super) fn request_return(&self, owner: &Owner, identity: &Identity) -> Result<(), String> {
        let _hardware = hardware_lease(Duration::from_secs(45))?;
        let previous = self.record(owner, identity)?;
        let mut next = previous.clone();
        next.request_return(owner, identity)?;
        self.journal.save(Some(&previous), &next)
    }
    /// Called with the hardware lease, before guardian launch and before any
    /// cooling preference write. The existing helper must already be active.
    pub(super) fn prepare(
        &self,
        owner: Owner,
        service: Snapshot,
        binary: &str,
    ) -> Result<Checkpoint, String> {
        let _preference = self.preference_lock()?;
        let previous = self.load()?;
        if previous.as_ref().is_some_and(|r| !r.phase.complete()) {
            return Err("Pending cooling recovery blocks another qualification".into());
        }
        let original = self.preference()?;
        let mode = policy::preference_mode(original.as_deref())?;
        if !matches!(mode, Mode::Curve | Mode::Max) {
            return Err("Qualification requires an active own Curve/Max helper; BIOS return is not qualified".into());
        }
        let ack = fan_task::acknowledgement(binary)?;
        let now = now_ms()?;
        fresh_ack(&ack, mode, now)?;
        let record = Checkpoint::new(owner, service, ack.helper, original, now)?;
        self.journal.save(previous.as_ref(), &record)?;
        Ok(record)
    }
    /// Ready guardian required by the caller. The helper keeps its heartbeat
    /// throughout the session; the owner never pauses it for service calls.
    pub(super) fn maximum(&self, owner: &Owner, identity: &Identity) -> Result<(), String> {
        {
            let _hardware = hardware_lease(Duration::from_secs(2))?;
            let _preference = self.preference_lock()?;
            let record = self.record(owner, identity)?;
            let current = self.preference()?;
            if record.maximum_action(owner, identity, current.as_deref())?
                == MaximumAction::WriteMaximum
            {
                profile_store::atomic_bytes(&self.preference, policy::FORCED_MAXIMUM)?;
            }
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let record = self.record(owner, identity)?;
            if record.phase != Phase::PendingMaximum {
                return Err("Cooling session was revoked before maximum confirmation".into());
            }
            let attempt = fan_task::acknowledgement(&record.helper.binary_sha256).and_then(|ack| {
                let _hardware = hardware_lease(Duration::from_secs(2))?;
                let _preference = self.preference_lock()?;
                let previous = self.record(owner, identity)?;
                let mut next = previous.clone();
                next.confirm_maximum(
                    owner,
                    identity,
                    self.preference()?.as_deref(),
                    &ack,
                    now_ms()?,
                )?;
                self.journal.save(Some(&previous), &next)
            });
            match attempt {
                Ok(()) => return Ok(()),
                Err(error) if Instant::now() >= deadline => {
                    return Err(format!("Maximum cooling was not acknowledged: {error}"));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        }
    }
    pub(super) fn verify_maximum(&self, owner: &Owner, identity: &Identity) -> Result<(), String> {
        let record = self.record(owner, identity)?;
        if record.phase != Phase::MaximumConfirmed
            || self.preference()?.as_deref() != Some(policy::FORCED_MAXIMUM)
        {
            return Err("Maximum cooling responsibility changed".into());
        }
        let ack = fan_task::acknowledgement(&record.helper.binary_sha256)?;
        fresh_ack(&ack, Mode::Max, now_ms()?)?;
        if ack.helper != record.helper
            || ack
                .last_heartbeat_unix_ms
                .is_none_or(|v| v < record.prepared_unix_ms)
        {
            return Err(
                "Maximum cooling acknowledgement belongs to a different helper/session".into(),
            );
        }
        Ok(())
    }
    pub(super) fn await_maximum_ack(
        &self,
        owner: &Owner,
        identity: &Identity,
    ) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let record = self.record(owner, identity)?;
            if record.phase != Phase::MaximumConfirmed {
                return Err("Cooling session was revoked".into());
            }
            match self.verify_maximum(owner, identity) {
                Ok(()) => return Ok(()),
                Err(error) if Instant::now() >= deadline => return Err(error),
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        }
    }
    /// Called by the guardian at a bounded cadence. Missing cooling triggers
    /// recovery; it never repeats a service STOP or overwrites a user's choice.
    pub(super) fn maintain(&self, owner: &Owner, identity: &Identity) -> Result<(), String> {
        let record = self.record(owner, identity)?;
        if record.phase.complete()
            || matches!(record.phase, Phase::ReturnRequested | Phase::RestorePending)
        {
            return Ok(());
        }
        let current = self.preference()?;
        if record.phase == Phase::PendingMaximum && current == record.original_preference {
            return Ok(());
        }
        if current.as_deref() != Some(policy::FORCED_MAXIMUM) {
            return Err("Cooling preference changed during handover".into());
        }
        let ack = fan_task::acknowledgement(&record.helper.binary_sha256)?;
        let now = now_ms()?;
        // Give the existing two-second helper cycle time to see the first write.
        if record.phase == Phase::PendingMaximum
            && now.saturating_sub(record.prepared_unix_ms) < 5_000
        {
            return Ok(());
        }
        fresh_ack(&ack, Mode::Max, now)
    }
    /// Ensure a live, verified helper before service return or any lowering of
    /// the forced maximum. This can only run its already-installed fixed task.
    pub(super) fn ensure_helper(&self, owner: &Owner, identity: &Identity) -> Result<(), String> {
        let record = self.record(owner, identity)?;
        if !record.phase.complete() {
            fan_task::ensure_running(&record.helper.binary_sha256)?;
            // A restarted process existing is not an acknowledgement. When
            // maximum is still selected, observe its fresh heartbeat/output
            // before lowering the preference during return.
            if self.preference()?.as_deref() == Some(policy::FORCED_MAXIMUM) {
                let deadline = Instant::now() + Duration::from_secs(8);
                loop {
                    let result = fan_task::acknowledgement(&record.helper.binary_sha256)
                        .and_then(|ack| fresh_ack(&ack, Mode::Max, now_ms()?));
                    match result {
                        Ok(()) => break,
                        Err(error) if Instant::now() >= deadline => return Err(error),
                        Err(_) => std::thread::sleep(Duration::from_millis(200)),
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn restore(
        &self,
        owner: &Owner,
        identity: &Identity,
        mut readiness: impl FnMut() -> Result<RecoveryReadiness, String>,
    ) -> Result<Checkpoint, String> {
        let record = self.record(owner, identity)?;
        if record.phase.complete() {
            return Ok(record);
        }
        self.ensure_helper(owner, identity)?;
        {
            let _hardware = hardware_lease(Duration::from_secs(2))?;
            let _preference = self.preference_lock()?;
            let previous = self.record(owner, identity)?;
            let ready = readiness()?;
            let mut next = previous.clone();
            next.begin_restore(owner, identity, ready, now_ms()?)?;
            self.journal.save(Some(&previous), &next)?;
            let current = self.preference()?;
            if let RestoreAction::WriteOriginal(original) =
                next.restore_action(owner, identity, ready, current.as_deref())?
            {
                // BIOS/absent originals cannot be created by this qualifier.
                let original =
                    original.ok_or("Absent/Bios cooling original is not qualified for restore")?;
                profile_store::atomic_bytes(&self.preference, &original)?;
            }
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let record = self.record(owner, identity)?;
            if record.phase.complete() {
                return Ok(record);
            }
            let attempt = fan_task::acknowledgement(&record.helper.binary_sha256).and_then(|ack| {
                let _hardware = hardware_lease(Duration::from_secs(2))?;
                let _preference = self.preference_lock()?;
                let previous = self.record(owner, identity)?;
                if previous.phase.complete() {
                    return Ok(previous);
                }
                let mut next = previous.clone();
                next.confirm_restored(
                    owner,
                    identity,
                    readiness()?,
                    self.preference()?.as_deref(),
                    &ack,
                    now_ms()?,
                )?;
                self.journal.save(Some(&previous), &next)?;
                Ok(next)
            });
            match attempt {
                Ok(record) => return Ok(record),
                Err(error) if Instant::now() >= deadline => {
                    return Err(format!("Cooling return remains pending: {error}"));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        }
    }
}
fn fresh_ack(ack: &Acknowledgement, mode: Mode, now: u64) -> Result<(), String> {
    ack.helper.validate()?;
    let updated = ack.updated_unix_ms;
    if ack.mode.effective() != mode
        || updated > now
        || now - updated > 5_000
        || ack
            .last_heartbeat_unix_ms
            .is_none_or(|v| v > updated || now.saturating_sub(v) > 25_000)
    {
        return Err("Cooling helper has no fresh mode/heartbeat acknowledgement".into());
    }
    Ok(())
}
