//! Durable, explicit controller handover policy. No Windows or hardware access.
//! A backend may address only its fixed qualified subject. Journal intent must
//! survive before STOP; recovery only returns a previously running owner.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subject {
    HpOmenCap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub subject: Subject,
    pub image_sha256: String,
    /// Canonical configuration digest, including executable, account, type,
    /// start mode and dependencies. Configuration itself is never overwritten.
    pub configuration_sha256: String,
}
impl Identity {
    fn validate(&self) -> Result<(), String> {
        if [&self.image_sha256, &self.configuration_sha256]
            .iter()
            .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("Invalid controller identity digest".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Stopped,
    Running,
    Transitioning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub identity: Identity,
    pub state: State,
    pub process_id: u32,
    pub process_started: u64,
    pub can_stop: bool,
    pub has_dependents: bool,
}
impl Snapshot {
    fn validate(&self) -> Result<(), String> {
        self.identity.validate()?;
        if (self.state == State::Running && (self.process_id == 0 || self.process_started == 0))
            || (self.state == State::Stopped && (self.process_id != 0 || self.process_started != 0))
        {
            return Err("Controller process identity is incomplete".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    pub user: String,
    pub process_id: u32,
    /// Kernel-reported process creation identity, not a PID-only liveness test.
    pub process_started: u64,
    pub boot: u64,
    pub session: String,
}
impl Owner {
    pub fn validate(&self) -> Result<(), String> {
        if self.user.is_empty()
            || self.user.len() > 256
            || self.user.chars().any(char::is_control)
            || self.process_id == 0
            || self.process_started == 0
            || self.boot == 0
            || self.session.len() != 64
            || !self.session.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid controller owner identity".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    StopPending,
    Owned,
    RestorePending,
    Restored,
    /// Another controller started an owner that was originally stopped.
    /// Respect that change; do not issue another STOP to undo it.
    Superseded,
}
impl Phase {
    pub fn complete(self) -> bool {
        matches!(self, Self::Restored | Self::Superseded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema: u32,
    pub owner: Owner,
    pub original: Snapshot,
    pub phase: Phase,
}
impl Record {
    pub fn validate(&self) -> Result<(), String> {
        self.owner.validate()?;
        self.original.validate()?;
        if self.schema != 1
            || self.original.state == State::Transitioning
            || (self.phase == Phase::StopPending && self.original.state != State::Running)
        {
            return Err("Invalid controller recovery record".into());
        }
        Ok(())
    }
}

/// The native adapter must recheck identity and state through the same fixed
/// service handle immediately before either command. It never changes config.
pub trait Backend {
    fn inspect(&mut self) -> Result<Snapshot, String>;
    fn stop(&mut self, expected: &Snapshot) -> Result<(), String>;
    fn start(&mut self, expected: &Identity) -> Result<(), String>;
}

/// Caller owns the machine-wide controller lease for the complete session.
/// `save` compares the expected record and commits/flushed intent atomically.
pub trait Journal {
    fn load(&self) -> Result<Option<Record>, String>;
    fn save(&mut self, expected: Option<&Record>, next: &Record) -> Result<(), String>;
}

/// Deliberately no Default: a plan/status read cannot implicitly grant takeover.
pub enum Consent {
    ExplicitSession,
}

#[derive(Debug, Serialize)]
pub struct Failure {
    pub error: String,
    pub recovery_errors: Vec<String>,
}
impl From<String> for Failure {
    fn from(error: String) -> Self {
        Self {
            error,
            recovery_errors: vec![],
        }
    }
}

fn inspect_expected(backend: &mut impl Backend, identity: &Identity) -> Result<Snapshot, String> {
    let observed = backend.inspect()?;
    observed.validate()?;
    if &observed.identity != identity {
        return Err("Controller image or configuration changed; no command was sent".into());
    }
    Ok(observed)
}

pub fn acquire(
    backend: &mut impl Backend,
    journal: &mut impl Journal,
    owner: Owner,
    expected: &Identity,
    _consent: Consent,
) -> Result<Record, Failure> {
    owner.validate()?;
    expected.validate()?;
    let previous = journal.load()?;
    if let Some(record) = previous.as_ref() {
        record.validate()?;
        if !record.phase.complete() {
            return Err("Pending controller recovery blocks another takeover"
                .to_owned()
                .into());
        }
        if record.owner == owner {
            return Err("A new takeover needs a fresh session identity"
                .to_owned()
                .into());
        }
    }
    let original = inspect_expected(backend, expected)?;
    if original.state == State::Transitioning
        || original.has_dependents
        || (original.state == State::Running && !original.can_stop)
    {
        return Err("Controller is busy, cannot stop, or has dependent services"
            .to_owned()
            .into());
    }
    let mut record = Record {
        schema: 1,
        owner,
        phase: if original.state == State::Running {
            Phase::StopPending
        } else {
            Phase::Owned
        },
        original,
    };
    // If intent cannot be persisted, STOP must never happen.
    journal.save(previous.as_ref(), &record)?;
    if record.original.state == State::Stopped {
        return Ok(record);
    }
    let result = (|| -> Result<(), String> {
        backend.stop(&record.original)?;
        if inspect_expected(backend, expected)?.state != State::Stopped {
            return Err("Controller STOP was not confirmed".into());
        }
        let pending = record.clone();
        record.phase = Phase::Owned;
        journal.save(Some(&pending), &record)
    })();
    match result {
        Ok(()) => Ok(record),
        Err(error) => {
            // STOP can have taken effect even when its API or final journal
            // write failed. Recover the durable intent, never retry STOP.
            let recovery_errors = release(backend, journal, &record.owner, expected)
                .err()
                .into_iter()
                .flat_map(|e| std::iter::once(e.error).chain(e.recovery_errors))
                .collect();
            Err(Failure {
                error,
                recovery_errors,
            })
        }
    }
}

pub fn release(
    backend: &mut impl Backend,
    journal: &mut impl Journal,
    owner: &Owner,
    expected: &Identity,
) -> Result<Record, Failure> {
    let mut record = journal
        .load()?
        .ok_or_else(|| "No controller recovery record exists".to_owned())?;
    record.validate()?;
    if &record.owner != owner || &record.original.identity != expected {
        return Err("Controller recovery owner/identity differs"
            .to_owned()
            .into());
    }
    if record.phase.complete() {
        return Ok(record);
    }
    let current = inspect_expected(backend, expected)?;
    if current.state == State::Transitioning {
        return Err("Controller transition is still pending; recovery retained"
            .to_owned()
            .into());
    }
    let original_record = record.clone();
    record.phase = Phase::RestorePending;
    // Existing STOP intent already authorizes returning its exact original.
    // A storage failure here must not strand a controller that we stopped.
    let mut errors: Vec<String> = journal
        .save(Some(&original_record), &record)
        .err()
        .into_iter()
        .collect();
    if !errors.is_empty() {
        let retained = journal.load()?;
        if retained.as_ref() != Some(&original_record) {
            return Err("Controller journal changed during recovery; no START sent"
                .to_owned()
                .into());
        }
        record = original_record;
    }
    if record.original.state == State::Running && current.state == State::Stopped {
        if let Err(error) = backend.start(expected) {
            errors.push(error);
            return Err(Failure {
                error: "Original controller did not restart; recovery retained".into(),
                recovery_errors: errors,
            });
        }
    }
    let observed = inspect_expected(backend, expected)?;
    let phase = match (record.original.state, observed.state) {
        (State::Running, State::Running) | (State::Stopped, State::Stopped) => Phase::Restored,
        (State::Stopped, State::Running) => Phase::Superseded,
        _ => {
            return Err(
                "Original controller state is not confirmed; recovery retained"
                    .to_owned()
                    .into(),
            );
        }
    };
    let pending = record.clone();
    record.phase = phase;
    if let Err(error) = journal.save(Some(&pending), &record) {
        errors.push(error);
        return Err(Failure {
            error: "Controller state returned but completion could not be saved".into(),
            recovery_errors: errors,
        });
    }
    // A recovered transient storage error is retained as an operation failure,
    // even though the final record now proves successful physical restoration.
    if !errors.is_empty() {
        return Err(Failure {
            error: "Controller restored after a journal write failure".into(),
            recovery_errors: errors,
        });
    }
    Ok(record)
}

#[derive(Clone, Copy)]
pub enum OwnerLiveness {
    Alive,
    Dead,
    Unknown,
}

/// Recheck immediately before an owned operation. An external restart loses
/// ownership; callers must not fight it with a repeated STOP.
pub fn verify_owned(
    backend: &mut impl Backend,
    journal: &impl Journal,
    owner: &Owner,
    expected: &Identity,
) -> Result<Record, Failure> {
    let record = journal
        .load()?
        .ok_or_else(|| "No controller lease exists".to_owned())?;
    record.validate()?;
    if &record.owner != owner
        || &record.original.identity != expected
        || record.phase != Phase::Owned
    {
        return Err("Controller lease is not owned by this session"
            .to_owned()
            .into());
    }
    if inspect_expected(backend, expected)?.state != State::Stopped {
        return Err("The original controller restarted; ownership was lost"
            .to_owned()
            .into());
    }
    Ok(record)
}

/// Only the native process/boot identity check may declare the owner dead.
/// Recovery never takes over or issues STOP, including after a new boot.
pub fn recover(
    backend: &mut impl Backend,
    journal: &mut impl Journal,
    expected: &Identity,
    liveness: OwnerLiveness,
) -> Result<Record, Failure> {
    if !matches!(liveness, OwnerLiveness::Dead) {
        return Err("A live or unknown controller owner blocks recovery"
            .to_owned()
            .into());
    }
    let record = journal
        .load()?
        .ok_or_else(|| "No controller recovery record exists".to_owned())?;
    release(backend, journal, &record.owner, expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Device {
        state: Snapshot,
        stops: usize,
        starts: usize,
        stop_fails_after_effect: bool,
        start_fails: bool,
    }
    impl Backend for Device {
        fn inspect(&mut self) -> Result<Snapshot, String> {
            Ok(self.state.clone())
        }
        fn stop(&mut self, expected: &Snapshot) -> Result<(), String> {
            assert_eq!(expected, &self.state);
            self.stops += 1;
            self.state.state = State::Stopped;
            self.state.process_id = 0;
            self.state.process_started = 0;
            if self.stop_fails_after_effect {
                Err("STOP result lost".into())
            } else {
                Ok(())
            }
        }
        fn start(&mut self, expected: &Identity) -> Result<(), String> {
            assert_eq!(expected, &self.state.identity);
            self.starts += 1;
            if self.start_fails {
                return Err("START failed".into());
            }
            self.state.state = State::Running;
            self.state.process_id = 200;
            self.state.process_started = 2000;
            Ok(())
        }
    }
    #[derive(Default)]
    struct Memory {
        record: Option<Record>,
        writes: usize,
        fail_at: Option<usize>,
    }
    impl Journal for Memory {
        fn load(&self) -> Result<Option<Record>, String> {
            Ok(self.record.clone())
        }
        fn save(&mut self, expected: Option<&Record>, next: &Record) -> Result<(), String> {
            self.writes += 1;
            assert_eq!(expected, self.record.as_ref());
            if self.fail_at == Some(self.writes) {
                return Err("Disk failed".into());
            }
            self.record = Some(next.clone());
            Ok(())
        }
    }
    fn setup() -> (Device, Memory, Owner, Identity) {
        let identity = Identity {
            subject: Subject::HpOmenCap,
            image_sha256: "a".repeat(64),
            configuration_sha256: "b".repeat(64),
        };
        (
            Device {
                state: Snapshot {
                    identity: identity.clone(),
                    state: State::Running,
                    process_id: 100,
                    process_started: 1000,
                    can_stop: true,
                    has_dependents: false,
                },
                stops: 0,
                starts: 0,
                stop_fails_after_effect: false,
                start_fails: false,
            },
            Memory::default(),
            Owner {
                user: "user".into(),
                process_id: 10,
                process_started: 1000,
                boot: 100,
                session: "c".repeat(64),
            },
            identity,
        )
    }
    #[test]
    fn explicit_session_restores_once_and_never_repeats_stop() {
        let (mut d, mut j, o, i) = setup();
        assert_eq!(
            acquire(&mut d, &mut j, o.clone(), &i, Consent::ExplicitSession)
                .unwrap()
                .phase,
            Phase::Owned
        );
        assert_eq!(
            release(&mut d, &mut j, &o, &i).unwrap().phase,
            Phase::Restored
        );
        release(&mut d, &mut j, &o, &i).unwrap();
        assert_eq!((d.stops, d.starts), (1, 1));
    }
    #[test]
    fn no_stop_without_durable_intent_and_partial_api_error_recovers() {
        let (mut d, mut j, o, i) = setup();
        j.fail_at = Some(1);
        assert!(acquire(&mut d, &mut j, o.clone(), &i, Consent::ExplicitSession).is_err());
        assert_eq!(d.stops, 0);
        j.fail_at = None;
        d.stop_fails_after_effect = true;
        assert!(acquire(&mut d, &mut j, o, &i, Consent::ExplicitSession).is_err());
        assert_eq!((d.stops, d.starts), (1, 1));
        assert_eq!(j.record.unwrap().phase, Phase::Restored);
    }
    #[test]
    fn post_stop_storage_failure_returns_the_oem_without_erasing_failure() {
        let (mut d, mut j, o, i) = setup();
        j.fail_at = Some(2);
        assert!(acquire(&mut d, &mut j, o, &i, Consent::ExplicitSession).is_err());
        assert_eq!((d.stops, d.starts), (1, 1));
        assert_eq!(j.record.unwrap().phase, Phase::Restored);
    }
    #[test]
    fn failed_restart_retains_recovery_and_prevents_another_acquire() {
        let (mut d, mut j, o, i) = setup();
        acquire(&mut d, &mut j, o.clone(), &i, Consent::ExplicitSession).unwrap();
        d.start_fails = true;
        assert!(release(&mut d, &mut j, &o, &i).is_err());
        assert!(!j.record.as_ref().unwrap().phase.complete());
        assert!(acquire(&mut d, &mut j, o, &i, Consent::ExplicitSession).is_err());
        assert_eq!(d.stops, 1);
        d.start_fails = false;
        assert_eq!(
            recover(&mut d, &mut j, &i, OwnerLiveness::Dead)
                .unwrap()
                .phase,
            Phase::Restored
        );
    }
    #[test]
    fn originally_stopped_and_external_restarts_are_respected() {
        let (mut d, mut j, o, i) = setup();
        d.state.state = State::Stopped;
        d.state.process_id = 0;
        d.state.process_started = 0;
        acquire(&mut d, &mut j, o.clone(), &i, Consent::ExplicitSession).unwrap();
        d.state.state = State::Running;
        d.state.process_id = 200;
        d.state.process_started = 2000;
        assert_eq!(
            release(&mut d, &mut j, &o, &i).unwrap().phase,
            Phase::Superseded
        );
        assert_eq!((d.stops, d.starts), (0, 0));
    }
    #[test]
    fn changed_identity_foreign_owner_and_uncertain_liveness_never_mutate() {
        let (mut d, mut j, o, i) = setup();
        acquire(&mut d, &mut j, o.clone(), &i, Consent::ExplicitSession).unwrap();
        for live in [OwnerLiveness::Alive, OwnerLiveness::Unknown] {
            assert!(recover(&mut d, &mut j, &i, live).is_err());
        }
        let mut foreign = o.clone();
        foreign.process_started += 1;
        assert!(release(&mut d, &mut j, &foreign, &i).is_err());
        d.state.identity.configuration_sha256 = "d".repeat(64);
        assert!(release(&mut d, &mut j, &o, &i).is_err());
        assert_eq!((d.stops, d.starts), (1, 0));
    }
    #[test]
    fn recovery_storage_error_still_returns_a_stopped_oem_and_keeps_error_visible() {
        let (mut d, mut j, o, i) = setup();
        acquire(&mut d, &mut j, o.clone(), &i, Consent::ExplicitSession).unwrap();
        j.fail_at = Some(3);
        assert!(release(&mut d, &mut j, &o, &i).is_err());
        assert_eq!(d.starts, 1);
        assert_eq!(j.record.unwrap().phase, Phase::Restored);
    }
    #[test]
    fn pending_transitions_dependents_and_invalid_records_are_rejected() {
        for blocked in 0..3 {
            let (mut d, mut j, o, i) = setup();
            match blocked {
                0 => d.state.state = State::Transitioning,
                1 => d.state.has_dependents = true,
                _ => d.state.can_stop = false,
            }
            assert!(acquire(&mut d, &mut j, o, &i, Consent::ExplicitSession).is_err());
            assert_eq!((d.stops, j.writes), (0, 0));
        }
        let (mut d, mut j, o, i) = setup();
        acquire(&mut d, &mut j, o, &i, Consent::ExplicitSession).unwrap();
        let mut record = j.record.unwrap();
        record.schema = 99;
        assert!(record.validate().is_err());
    }

    #[test]
    fn an_external_restart_loses_ownership_without_a_second_stop() {
        let (mut d, mut j, o, i) = setup();
        acquire(&mut d, &mut j, o.clone(), &i, Consent::ExplicitSession).unwrap();
        verify_owned(&mut d, &j, &o, &i).unwrap();
        d.state.state = State::Running;
        d.state.process_id = 300;
        d.state.process_started = 3000;
        assert!(verify_owned(&mut d, &j, &o, &i).is_err());
        assert_eq!(
            release(&mut d, &mut j, &o, &i).unwrap().phase,
            Phase::Restored
        );
        assert_eq!((d.stops, d.starts), (1, 0));
    }

    #[test]
    fn recovery_cannot_accept_a_pid_without_its_creation_identity() {
        let (mut d, mut j, o, i) = setup();
        d.state.process_started = 0;
        assert!(acquire(&mut d, &mut j, o, &i, Consent::ExplicitSession).is_err());
        assert_eq!((d.stops, j.writes), (0, 0));
    }
}
