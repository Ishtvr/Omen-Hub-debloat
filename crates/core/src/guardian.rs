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
        if record.original.identity != self.identity {
            return Err("Guardian session controller identity changed".into());
        }
        if record.phase.complete() {
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
