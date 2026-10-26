//! Decision policy for an independent, bounded qualification guardian.
//! Observation grants only return of this exact session, never another STOP.
use crate::ownership::{Identity, Owner, Record};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const QUALIFICATION_LIMIT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentState {
    Alive,
    Exited,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Open,
    Closed,
    Invalid,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    OwnerExited,
    LinkLost,
    QualificationExpired,
    CoolingUnavailable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Finish {
    NoSession,
    Returned,
    Superseded,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Wait,
    Finish(Finish),
    Recover(Trigger),
}

/// A session can own more than the service. Completion must include every
/// durable duty (for example a temporary maximum-cooling preference).
#[derive(Debug, Clone)]
pub struct Observation {
    pub owner: Owner,
    pub identity: Identity,
    pub complete: bool,
}
impl Observation {
    /// Native handover owns cooling before it owns the service. An older,
    /// completed service ledger must not hide that first durable obligation.
    pub fn coupled(
        owner: &Owner,
        identity: &Identity,
        service: Option<&Record>,
        cooling: Option<&crate::cooling_override::Checkpoint>,
    ) -> Result<Option<Self>, String> {
        owner.validate()?;
        identity.validate()?;
        if let Some(record) = service {
            record.validate()?;
        }
        if let Some(record) = cooling {
            record.validate()?;
        }
        if let Some(cooling) = cooling.filter(|c| &c.owner == owner) {
            if &cooling.original_service.identity != identity {
                return Err("Cooling controller identity changed".into());
            }
            if let Some(service) = service {
                if &service.owner != owner && !service.phase.complete() {
                    return Err(
                        "Cooling and service duties belong to different active sessions".into(),
                    );
                }
                if &service.owner == owner && service.original != cooling.original_service {
                    return Err("Cooling and service recovery originals differ".into());
                }
            }
            return Ok(Some(Self {
                owner: owner.clone(),
                identity: identity.clone(),
                complete: cooling.phase.complete()
                    && service
                        .filter(|r| &r.owner == owner)
                        .is_none_or(|r| r.phase.complete()),
            }));
        }
        if service.is_some_and(|r| &r.owner == owner)
            && cooling.is_some_and(|r| !r.phase.complete())
        {
            return Err("Service and cooling duties belong to different active sessions".into());
        }
        service.map(Self::from_service).transpose()
    }
    pub fn from_service(record: &Record) -> Result<Self, String> {
        record.validate()?;
        Ok(Self {
            owner: record.owner.clone(),
            identity: record.original.identity.clone(),
            complete: record.phase.complete(),
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        self.owner.validate()?;
        self.identity.validate()
    }
}

pub struct Watch {
    owner: Owner,
    identity: Identity,
    seen: bool,
}
impl Watch {
    pub fn new(owner: Owner, identity: Identity) -> Result<Self, String> {
        owner.validate()?;
        identity.validate()?;
        Ok(Self {
            owner,
            identity,
            seen: false,
        })
    }
    pub fn observe(
        &mut self,
        elapsed: Duration,
        parent: ParentState,
        link: LinkState,
        record: Option<&Record>,
    ) -> Result<Decision, String> {
        let observation = record.map(Observation::from_service).transpose()?;
        self.observe_session(elapsed, parent, link, observation.as_ref())
    }
    pub fn observe_session(
        &mut self,
        elapsed: Duration,
        parent: ParentState,
        link: LinkState,
        record: Option<&Observation>,
    ) -> Result<Decision, String> {
        let trigger = if parent == ParentState::Exited {
            Some(Trigger::OwnerExited)
        } else if link != LinkState::Open {
            Some(Trigger::LinkLost)
        } else if elapsed >= QUALIFICATION_LIMIT {
            Some(Trigger::QualificationExpired)
        } else {
            None
        };
        let Some(record) = record else {
            if self.seen {
                return Err("Guardian's durable session record disappeared".into());
            }
            return Ok(if trigger.is_some() {
                Decision::Finish(Finish::NoSession)
            } else {
                Decision::Wait
            });
        };
        record.validate()?;
        if record.owner != self.owner {
            // A previous terminal record may exist before the parent's first
            // write-ahead commit. Once ours was seen, never touch a successor.
            return Ok(if self.seen {
                Decision::Finish(Finish::Superseded)
            } else if trigger.is_some() {
                Decision::Finish(Finish::NoSession)
            } else {
                Decision::Wait
            });
        }
        self.seen = true;
        if record.identity != self.identity {
            return Err("Guardian session controller identity changed".into());
        }
        if record.complete {
            return Ok(Decision::Finish(Finish::Returned));
        }
        if let Some(trigger) = trigger {
            if parent == ParentState::Unknown {
                return Err("Guardian cannot confirm the owner process state".into());
            }
            return Ok(Decision::Recover(trigger));
        }
        Ok(Decision::Wait)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership::{Phase, Snapshot, State, Subject};
    fn fixture() -> (Watch, Record) {
        let owner = Owner {
            user: "user".into(),
            process_id: 10,
            process_started: 100,
            boot: 1,
            session: "a".repeat(64),
        };
        let identity = Identity {
            subject: Subject::HpOmenCap,
            image_sha256: "b".repeat(64),
            configuration_sha256: "c".repeat(64),
        };
        let record = Record {
            schema: 1,
            owner: owner.clone(),
            original: Snapshot {
                identity: identity.clone(),
                state: State::Running,
                process_id: 20,
                process_started: 200,
                can_stop: true,
                has_dependents: false,
            },
            phase: Phase::Owned,
        };
        (Watch::new(owner, identity).unwrap(), record)
    }
    #[test]
    fn coupled_guardian_waits_for_cooling_before_stop_and_after_service_return() {
        use crate::cooling_override::{Checkpoint, HelperIdentity, Phase as CoolingPhase};
        let (mut watch, mut service) = fixture();
        let mut cooling = Checkpoint::new(
            service.owner.clone(),
            service.original.clone(),
            HelperIdentity {
                binary_sha256: "d".repeat(64),
                process_id: 30,
                process_started: 300,
            },
            Some(b"{\"schema\":1,\"mode\":\"curve\"}".to_vec()),
            1000,
        )
        .unwrap();
        let owner = service.owner.clone();
        let identity = service.original.identity.clone();
        let before_stop = Observation::coupled(&owner, &identity, None, Some(&cooling))
            .unwrap()
            .unwrap();
        assert_eq!(
            watch
                .observe_session(
                    Duration::ZERO,
                    ParentState::Exited,
                    LinkState::Closed,
                    Some(&before_stop)
                )
                .unwrap(),
            Decision::Recover(Trigger::OwnerExited)
        );
        service.phase = Phase::Restored;
        let after_service = Observation::coupled(&owner, &identity, Some(&service), Some(&cooling))
            .unwrap()
            .unwrap();
        assert!(!after_service.complete);
        assert_eq!(
            watch
                .observe_session(
                    Duration::ZERO,
                    ParentState::Alive,
                    LinkState::Open,
                    Some(&after_service)
                )
                .unwrap(),
            Decision::Wait
        );
        cooling.phase = CoolingPhase::Restored;
        cooling.restore_requested_unix_ms = Some(2000);
        let complete = Observation::coupled(&owner, &identity, Some(&service), Some(&cooling))
            .unwrap()
            .unwrap();
        assert_eq!(
            watch
                .observe_session(
                    Duration::ZERO,
                    ParentState::Alive,
                    LinkState::Open,
                    Some(&complete)
                )
                .unwrap(),
            Decision::Finish(Finish::Returned)
        );
        service.phase = Phase::Owned;
        service.owner.session = "e".repeat(64);
        assert!(Observation::coupled(&owner, &identity, Some(&service), Some(&cooling)).is_err());
        service.owner = owner.clone();
        service.original.process_started += 1;
        assert!(Observation::coupled(&owner, &identity, Some(&service), Some(&cooling)).is_err());
    }
    #[test]
    fn previous_records_and_no_journal_wait_for_the_write_ahead_commit() {
        let (mut w, mut r) = fixture();
        r.owner.session = "d".repeat(64);
        r.phase = Phase::Restored;
        assert_eq!(
            w.observe(
                Duration::ZERO,
                ParentState::Alive,
                LinkState::Open,
                Some(&r)
            )
            .unwrap(),
            Decision::Wait
        );
        assert_eq!(
            w.observe(Duration::ZERO, ParentState::Alive, LinkState::Open, None)
                .unwrap(),
            Decision::Wait
        );
        assert_eq!(
            w.observe(Duration::ZERO, ParentState::Exited, LinkState::Closed, None)
                .unwrap(),
            Decision::Finish(Finish::NoSession)
        );
    }
    #[test]
    fn death_lost_link_and_expiry_authorize_only_the_matching_session() {
        for (parent, link, elapsed, trigger) in [
            (
                ParentState::Exited,
                LinkState::Open,
                Duration::ZERO,
                Trigger::OwnerExited,
            ),
            (
                ParentState::Alive,
                LinkState::Closed,
                Duration::ZERO,
                Trigger::LinkLost,
            ),
            (
                ParentState::Alive,
                LinkState::Invalid,
                Duration::ZERO,
                Trigger::LinkLost,
            ),
            (
                ParentState::Alive,
                LinkState::Open,
                QUALIFICATION_LIMIT,
                Trigger::QualificationExpired,
            ),
        ] {
            let (mut w, r) = fixture();
            assert_eq!(
                w.observe(elapsed, parent, link, Some(&r)).unwrap(),
                Decision::Recover(trigger)
            );
        }
    }
    #[test]
    fn normal_return_and_successor_sessions_never_restart_a_service() {
        let (mut w, mut r) = fixture();
        assert_eq!(
            w.observe(
                Duration::ZERO,
                ParentState::Alive,
                LinkState::Open,
                Some(&r)
            )
            .unwrap(),
            Decision::Wait
        );
        assert!(
            w.observe(Duration::ZERO, ParentState::Alive, LinkState::Open, None)
                .is_err()
        );
        r.phase = Phase::Restored;
        assert_eq!(
            w.observe(
                Duration::ZERO,
                ParentState::Alive,
                LinkState::Open,
                Some(&r)
            )
            .unwrap(),
            Decision::Finish(Finish::Returned)
        );
        r.owner.session = "d".repeat(64);
        r.phase = Phase::Owned;
        assert_eq!(
            w.observe(
                QUALIFICATION_LIMIT,
                ParentState::Exited,
                LinkState::Closed,
                Some(&r)
            )
            .unwrap(),
            Decision::Finish(Finish::Superseded)
        );
    }
    #[test]
    fn changed_identity_corrupt_records_and_unknown_liveness_are_errors() {
        let (mut w, mut r) = fixture();
        assert!(
            w.observe(
                QUALIFICATION_LIMIT,
                ParentState::Unknown,
                LinkState::Open,
                Some(&r)
            )
            .is_err()
        );
        r.original.identity.image_sha256 = "d".repeat(64);
        assert!(
            w.observe(
                Duration::ZERO,
                ParentState::Exited,
                LinkState::Open,
                Some(&r)
            )
            .is_err()
        );
        r.schema = 2;
        assert!(
            w.observe(
                Duration::ZERO,
                ParentState::Alive,
                LinkState::Open,
                Some(&r)
            )
            .is_err()
        );
    }
}
