//! Pure policy for a temporary, journaled maximum-cooling preference.
//! The native coordinator must persist intent before changing the fixed fan
//! preference file, verify process/task identity, and measure actual cooling.
//! An acknowledgement here proves helper state, never BIOS fan ownership.
use crate::ownership::{self, Identity, Owner, Snapshot};
use serde::{Deserialize, Serialize};

pub const MAX_PREFERENCE_BYTES: usize = 1024;
/// Exact bytes written by the native override, independent of user formatting.
pub const FORCED_MAXIMUM: &[u8] = b"{\n  \"mode\": \"max\",\n  \"schema\": 1\n}\n";
const ACK_MAX_AGE_MS: u64 = 5_000;
/// The helper's current heartbeat interval is 20 s; allow one status cycle of
/// scheduling slack without accepting an arbitrarily old lease anchor.
const HEARTBEAT_MAX_AGE_MS: u64 = 25_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Bios,
    Curve,
    Max,
    #[serde(rename = "auto")]
    LegacyCurve,
}
impl Mode {
    pub fn effective(self) -> Self {
        if self == Self::LegacyCurve {
            Self::Curve
        } else {
            self
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preference {
    schema: u32,
    mode: Mode,
}

/// Absence is the existing BIOS preference. Unknown/corrupt bytes are errors,
/// not a default that could later overwrite the user's original configuration.
pub fn preference_mode(bytes: Option<&[u8]>) -> Result<Mode, String> {
    let Some(bytes) = bytes else {
        return Ok(Mode::Bios);
    };
    if bytes.is_empty() || bytes.len() > MAX_PREFERENCE_BYTES {
        return Err("Fan preference image exceeds its bound or is empty".into());
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let value: Preference =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid fan preference image: {e}"))?;
    if value.schema != 1 {
        return Err("Unknown fan preference schema".into());
    }
    Ok(value.mode.effective())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelperIdentity {
    pub binary_sha256: String,
    pub process_id: u32,
    pub process_started: u64,
}
impl HelperIdentity {
    pub fn validate(&self) -> Result<(), String> {
        if self.binary_sha256.len() != 64
            || !self.binary_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || self.process_id == 0
            || self.process_started == 0
        {
            return Err("Invalid cooling helper identity".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    PendingMaximum,
    MaximumConfirmed,
    /// Revoke further acquisition immediately, while maximum remains selected
    /// until service/hardware recovery permits the original preference.
    ReturnRequested,
    RestorePending,
    Restored,
    Superseded,
}
impl Phase {
    pub fn complete(self) -> bool {
        matches!(self, Self::Restored | Self::Superseded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub schema: u32,
    pub owner: Owner,
    pub original_service: Snapshot,
    pub helper: HelperIdentity,
    pub original_preference: Option<Vec<u8>>,
    pub forced_maximum: Vec<u8>,
    pub prepared_unix_ms: u64,
    pub restore_requested_unix_ms: Option<u64>,
    pub phase: Phase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryReadiness {
    pub service_returned: bool,
    pub hardware_recovery_clear: bool,
}
impl RecoveryReadiness {
    pub fn ready(self) -> bool {
        self.service_returned && self.hardware_recovery_clear
    }
}

/// Construct only after native process/task verification and a fresh status
/// read. The native layer separately checks physical RPM and temperature data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acknowledgement {
    pub helper: HelperIdentity,
    pub mode: Mode,
    pub updated_unix_ms: u64,
    pub last_heartbeat_unix_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaximumAction {
    WriteMaximum,
    AwaitAcknowledgement,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreAction {
    /// No preference write or completion is authorized until recovery is clear.
    HoldResponsibility,
    WriteOriginal(Option<Vec<u8>>),
    AwaitAcknowledgement,
    /// Preserve an independently changed, valid preference byte-for-byte.
    RespectExternal,
}

impl Checkpoint {
    pub fn new(
        owner: Owner,
        original_service: Snapshot,
        helper: HelperIdentity,
        original_preference: Option<Vec<u8>>,
        prepared_unix_ms: u64,
    ) -> Result<Self, String> {
        let record = Self {
            schema: 1,
            owner,
            original_service,
            helper,
            original_preference,
            forced_maximum: FORCED_MAXIMUM.to_vec(),
            prepared_unix_ms,
            restore_requested_unix_ms: None,
            phase: Phase::PendingMaximum,
        };
        record.validate()?;
        Ok(record)
    }
    pub fn validate(&self) -> Result<(), String> {
        // Reuse the service record's complete owner/snapshot validation without
        // inventing a weaker second interpretation of those recovery images.
        ownership::Record {
            schema: 1,
            owner: self.owner.clone(),
            original: self.original_service.clone(),
            phase: ownership::Phase::Owned,
        }
        .validate()?;
        self.helper.validate()?;
        preference_mode(self.original_preference.as_deref())?;
        if self.schema != 1 || self.prepared_unix_ms == 0 || self.forced_maximum != FORCED_MAXIMUM {
            return Err("Invalid cooling override checkpoint".into());
        }
        match (self.phase, self.restore_requested_unix_ms) {
            (Phase::PendingMaximum | Phase::MaximumConfirmed | Phase::ReturnRequested, None) => (),
            (Phase::RestorePending | Phase::Restored | Phase::Superseded, Some(at))
                if at >= self.prepared_unix_ms => {}
            _ => return Err("Cooling recovery phase/timestamp is inconsistent".into()),
        }
        Ok(())
    }
    fn authorize(&self, owner: &Owner, controller: &Identity) -> Result<(), String> {
        self.validate()?;
        if &self.owner != owner || &self.original_service.identity != controller {
            return Err("Cooling responsibility belongs to another owner/controller".into());
        }
        Ok(())
    }
    pub fn maximum_action(
        &self,
        owner: &Owner,
        controller: &Identity,
        current: Option<&[u8]>,
    ) -> Result<MaximumAction, String> {
        self.authorize(owner, controller)?;
        if self.phase != Phase::PendingMaximum {
            return Err("Maximum cooling intent is no longer pending".into());
        }
        preference_mode(current)?;
        if current == Some(self.forced_maximum.as_slice()) {
            return Ok(MaximumAction::AwaitAcknowledgement);
        }
        if current == self.original_preference.as_deref() {
            return Ok(MaximumAction::WriteMaximum);
        }
        Err("Fan preference changed before maximum cooling; no overwrite authorized".into())
    }
    pub fn confirm_maximum(
        &mut self,
        owner: &Owner,
        controller: &Identity,
        current: Option<&[u8]>,
        ack: &Acknowledgement,
        now_unix_ms: u64,
    ) -> Result<(), String> {
        self.authorize(owner, controller)?;
        if self.phase != Phase::PendingMaximum || current != Some(self.forced_maximum.as_slice()) {
            return Err("Maximum cooling preference was not observed".into());
        }
        self.check_ack(ack, Mode::Max, self.prepared_unix_ms, now_unix_ms, false)?;
        let heartbeat = ack
            .last_heartbeat_unix_ms
            .ok_or("Fresh maximum-cooling heartbeat is missing")?;
        if heartbeat < self.prepared_unix_ms
            || heartbeat > ack.updated_unix_ms
            || ack.updated_unix_ms - heartbeat > HEARTBEAT_MAX_AGE_MS
        {
            return Err(
                "Maximum-cooling heartbeat does not follow this session's preparation".into(),
            );
        }
        self.phase = Phase::MaximumConfirmed;
        Ok(())
    }
    /// Persist the changed checkpoint before executing any original-preference
    /// write. If persistence or the write fails, leave RestorePending durable.
    pub fn begin_restore(
        &mut self,
        owner: &Owner,
        controller: &Identity,
        readiness: RecoveryReadiness,
        now_unix_ms: u64,
    ) -> Result<(), String> {
        self.authorize(owner, controller)?;
        if !readiness.ready() {
            return Err(
                "Service/hardware recovery is unresolved; cooling responsibility retained".into(),
            );
        }
        if self.phase.complete() || self.phase == Phase::RestorePending {
            return Ok(());
        }
        if now_unix_ms < self.prepared_unix_ms {
            return Err("Cooling recovery timestamp precedes preparation".into());
        }
        self.phase = Phase::RestorePending;
        self.restore_requested_unix_ms = Some(now_unix_ms);
        Ok(())
    }
    pub fn request_return(&mut self, owner: &Owner, controller: &Identity) -> Result<(), String> {
        self.authorize(owner, controller)?;
        if matches!(self.phase, Phase::PendingMaximum | Phase::MaximumConfirmed) {
            self.phase = Phase::ReturnRequested;
        }
        Ok(())
    }
    pub fn restore_action(
        &self,
        owner: &Owner,
        controller: &Identity,
        readiness: RecoveryReadiness,
        current: Option<&[u8]>,
    ) -> Result<RestoreAction, String> {
        self.authorize(owner, controller)?;
        if !readiness.ready() {
            return Ok(RestoreAction::HoldResponsibility);
        }
        if self.phase != Phase::RestorePending {
            return Err("Cooling restore intent must be persisted first".into());
        }
        preference_mode(current)?;
        if current == self.original_preference.as_deref() {
            return Ok(RestoreAction::AwaitAcknowledgement);
        }
        if current == Some(self.forced_maximum.as_slice()) {
            return Ok(RestoreAction::WriteOriginal(
                self.original_preference.clone(),
            ));
        }
        Ok(RestoreAction::RespectExternal)
    }
    pub fn confirm_restored(
        &mut self,
        owner: &Owner,
        controller: &Identity,
        readiness: RecoveryReadiness,
        current: Option<&[u8]>,
        ack: &Acknowledgement,
        now_unix_ms: u64,
    ) -> Result<(), String> {
        let action = self.restore_action(owner, controller, readiness, current)?;
        if matches!(
            action,
            RestoreAction::HoldResponsibility | RestoreAction::WriteOriginal(_)
        ) {
            return Err("Original cooling preference/recovery is not yet confirmed".into());
        }
        let mode = preference_mode(current)?;
        let since = self
            .restore_requested_unix_ms
            .ok_or("Missing cooling restore timestamp")?;
        self.check_ack(ack, mode, since, now_unix_ms, true)?;
        self.phase = if action == RestoreAction::RespectExternal {
            Phase::Superseded
        } else {
            Phase::Restored
        };
        Ok(())
    }
    fn check_ack(
        &self,
        ack: &Acknowledgement,
        mode: Mode,
        since: u64,
        now: u64,
        replacement_allowed: bool,
    ) -> Result<(), String> {
        ack.helper.validate()?;
        if !ack
            .helper
            .binary_sha256
            .eq_ignore_ascii_case(&self.helper.binary_sha256)
            || (!replacement_allowed
                && (ack.helper.process_id != self.helper.process_id
                    || ack.helper.process_started != self.helper.process_started))
            || ack.mode.effective() != mode.effective()
            || ack.updated_unix_ms < since
            || now < ack.updated_unix_ms
            || now - ack.updated_unix_ms > ACK_MAX_AGE_MS
        {
            return Err(
                "Cooling helper acknowledgement is stale, foreign or for another mode".into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership::{State, Subject};
    #[test]
    fn return_request_revokes_late_acquisition_before_service_return() {
        let mut record = fixture(Some(b"{\"schema\":1,\"mode\":\"curve\"}".to_vec()));
        let owner = record.owner.clone();
        let id = record.original_service.identity.clone();
        record.request_return(&owner, &id).unwrap();
        record.validate().unwrap();
        assert_eq!(record.phase, Phase::ReturnRequested);
        assert!(
            record
                .maximum_action(&owner, &id, record.original_preference.as_deref())
                .is_err()
        );
        assert!(
            record
                .begin_restore(
                    &owner,
                    &id,
                    RecoveryReadiness {
                        service_returned: false,
                        hardware_recovery_clear: true,
                    },
                    2000
                )
                .is_err()
        );
        assert_eq!(record.phase, Phase::ReturnRequested);
        record.begin_restore(&owner, &id, ready(), 2000).unwrap();
        assert_eq!(record.phase, Phase::RestorePending);
    }
    fn fixture(original: Option<Vec<u8>>) -> Checkpoint {
        Checkpoint::new(
            Owner {
                user: "user".into(),
                process_id: 10,
                process_started: 100,
                boot: 1,
                session: "a".repeat(64),
            },
            Snapshot {
                identity: Identity {
                    subject: Subject::HpOmenCap,
                    image_sha256: "b".repeat(64),
                    configuration_sha256: "c".repeat(64),
                },
                state: State::Running,
                process_id: 20,
                process_started: 200,
                can_stop: true,
                has_dependents: false,
            },
            HelperIdentity {
                binary_sha256: "d".repeat(64),
                process_id: 30,
                process_started: 300,
            },
            original,
            1_000,
        )
        .unwrap()
    }
    fn original() -> Vec<u8> {
        b"\xef\xbb\xbf{\"schema\":1, \"mode\":\"auto\"}\r\n".to_vec()
    }
    fn ready() -> RecoveryReadiness {
        RecoveryReadiness {
            service_returned: true,
            hardware_recovery_clear: true,
        }
    }
    fn ack(record: &Checkpoint, mode: Mode, at: u64) -> Acknowledgement {
        Acknowledgement {
            helper: record.helper.clone(),
            mode,
            updated_unix_ms: at,
            last_heartbeat_unix_ms: Some(at),
        }
    }
    #[test]
    fn crash_before_preference_write_needs_no_write_but_a_fresh_ack() {
        let mut record = fixture(Some(original()));
        let owner = record.owner.clone();
        let id = record.original_service.identity.clone();
        record.begin_restore(&owner, &id, ready(), 2_000).unwrap();
        assert_eq!(
            record
                .restore_action(&owner, &id, ready(), Some(&original()))
                .unwrap(),
            RestoreAction::AwaitAcknowledgement
        );
        assert!(
            record
                .confirm_restored(
                    &owner,
                    &id,
                    ready(),
                    Some(&original()),
                    &ack(&record, Mode::Curve, 1_999),
                    2_000
                )
                .is_err()
        );
        record
            .confirm_restored(
                &owner,
                &id,
                ready(),
                Some(&original()),
                &ack(&record, Mode::Curve, 2_001),
                2_001,
            )
            .unwrap();
        assert_eq!(record.phase, Phase::Restored);
    }
    #[test]
    fn maximum_requires_same_instance_fresh_heartbeat_and_exact_target() {
        let mut record = fixture(Some(original()));
        let owner = record.owner.clone();
        let id = record.original_service.identity.clone();
        assert_eq!(
            record
                .maximum_action(&owner, &id, Some(&original()))
                .unwrap(),
            MaximumAction::WriteMaximum
        );
        let mut stale = ack(&record, Mode::Max, 1_001);
        stale.last_heartbeat_unix_ms = Some(999);
        assert!(
            record
                .confirm_maximum(&owner, &id, Some(FORCED_MAXIMUM), &stale, 1_002)
                .is_err()
        );
        stale.last_heartbeat_unix_ms = Some(1_001);
        stale.updated_unix_ms = 30_000;
        assert!(
            record
                .confirm_maximum(&owner, &id, Some(FORCED_MAXIMUM), &stale, 30_000)
                .is_err()
        );
        stale.updated_unix_ms = 1_001;
        stale.helper.process_started += 1;
        assert!(
            record
                .confirm_maximum(&owner, &id, Some(FORCED_MAXIMUM), &stale, 1_002)
                .is_err()
        );
        record
            .confirm_maximum(
                &owner,
                &id,
                Some(FORCED_MAXIMUM),
                &ack(&record, Mode::Max, 1_001),
                1_002,
            )
            .unwrap();
        assert_eq!(record.phase, Phase::MaximumConfirmed);
        assert!(
            record
                .maximum_action(&owner, &id, Some(&original()))
                .is_err()
        );
    }
    #[test]
    fn exact_original_and_absence_are_preserved_for_restore() {
        for prior in [Some(original()), None] {
            let mut record = fixture(prior.clone());
            let owner = record.owner.clone();
            let id = record.original_service.identity.clone();
            record.begin_restore(&owner, &id, ready(), 2_000).unwrap();
            assert_eq!(
                record
                    .restore_action(&owner, &id, ready(), Some(FORCED_MAXIMUM))
                    .unwrap(),
                RestoreAction::WriteOriginal(prior.clone())
            );
            let mode = preference_mode(prior.as_deref()).unwrap();
            record
                .confirm_restored(
                    &owner,
                    &id,
                    ready(),
                    prior.as_deref(),
                    &ack(&record, mode, 2_001),
                    2_001,
                )
                .unwrap();
            assert_eq!(record.phase, Phase::Restored);
        }
    }
    #[test]
    fn valid_external_edits_are_respected_but_never_reported_as_original_restored() {
        let mut record = fixture(Some(original()));
        let owner = record.owner.clone();
        let id = record.original_service.identity.clone();
        let external = b"{\"mode\":\"bios\",\"schema\":1}";
        assert!(record.maximum_action(&owner, &id, Some(external)).is_err());
        record.begin_restore(&owner, &id, ready(), 2_000).unwrap();
        assert_eq!(
            record
                .restore_action(&owner, &id, ready(), Some(external))
                .unwrap(),
            RestoreAction::RespectExternal
        );
        record
            .confirm_restored(
                &owner,
                &id,
                ready(),
                Some(external),
                &ack(&record, Mode::Bios, 2_001),
                2_001,
            )
            .unwrap();
        assert_eq!(record.phase, Phase::Superseded);
    }
    #[test]
    fn unresolved_recovery_and_failed_writes_retain_responsibility() {
        let mut record = fixture(Some(original()));
        let owner = record.owner.clone();
        let id = record.original_service.identity.clone();
        for readiness in [
            RecoveryReadiness {
                service_returned: false,
                ..ready()
            },
            RecoveryReadiness {
                hardware_recovery_clear: false,
                ..ready()
            },
        ] {
            let before = record.clone();
            assert!(record.begin_restore(&owner, &id, readiness, 2_000).is_err());
            assert_eq!(record, before);
            assert_eq!(
                record
                    .restore_action(&owner, &id, readiness, Some(FORCED_MAXIMUM))
                    .unwrap(),
                RestoreAction::HoldResponsibility
            );
        }
        record.begin_restore(&owner, &id, ready(), 2_000).unwrap();
        let before = record.clone();
        assert!(
            record
                .confirm_restored(
                    &owner,
                    &id,
                    ready(),
                    Some(FORCED_MAXIMUM),
                    &ack(&record, Mode::Max, 2_001),
                    2_001
                )
                .is_err()
        );
        assert_eq!(record, before);
        assert_eq!(record.phase, Phase::RestorePending);
    }
    #[test]
    fn recovery_can_acknowledge_a_verified_restarted_helper_of_the_same_binary() {
        let mut record = fixture(None);
        let owner = record.owner.clone();
        let id = record.original_service.identity.clone();
        record.begin_restore(&owner, &id, ready(), 2_000).unwrap();
        let mut returned = ack(&record, Mode::Bios, 2_001);
        returned.helper.process_id = 31;
        returned.helper.process_started = 301;
        record
            .confirm_restored(&owner, &id, ready(), None, &returned, 2_001)
            .unwrap();
        assert_eq!(record.phase, Phase::Restored);
    }
    #[test]
    fn corrupt_oversized_or_forged_images_and_unknown_fields_are_rejected() {
        for bad in [
            b"{".to_vec(),
            vec![b'x'; MAX_PREFERENCE_BYTES + 1],
            b"{\"schema\":2,\"mode\":\"max\"}".to_vec(),
            b"{\"schema\":1,\"mode\":\"other\"}".to_vec(),
            b"{\"schema\":1,\"mode\":\"max\",\"command\":\"x\"}".to_vec(),
        ] {
            assert!(preference_mode(Some(&bad)).is_err());
        }
        let mut record = fixture(None);
        record.forced_maximum = original();
        assert!(record.validate().is_err());
        let mut raw = serde_json::to_value(fixture(None)).unwrap();
        raw["path"] = "unrelated".into();
        assert!(serde_json::from_value::<Checkpoint>(raw).is_err());
    }
    #[test]
    fn wrong_owner_controller_helper_and_future_or_old_ack_cannot_complete() {
        let mut record = fixture(Some(original()));
        let owner = record.owner.clone();
        let id = record.original_service.identity.clone();
        let mut other = owner.clone();
        other.session = "e".repeat(64);
        assert!(record.begin_restore(&other, &id, ready(), 2_000).is_err());
        let mut changed = id.clone();
        changed.configuration_sha256 = "e".repeat(64);
        assert!(
            record
                .begin_restore(&owner, &changed, ready(), 2_000)
                .is_err()
        );
        record.begin_restore(&owner, &id, ready(), 2_000).unwrap();
        let mut wrong = ack(&record, Mode::Curve, 2_001);
        wrong.helper.binary_sha256 = "f".repeat(64);
        assert!(
            record
                .confirm_restored(&owner, &id, ready(), Some(&original()), &wrong, 2_001)
                .is_err()
        );
        assert!(
            record
                .confirm_restored(
                    &owner,
                    &id,
                    ready(),
                    Some(&original()),
                    &ack(&record, Mode::Curve, 2_002),
                    2_001
                )
                .is_err()
        );
        assert!(
            record
                .confirm_restored(
                    &owner,
                    &id,
                    ready(),
                    Some(&original()),
                    &ack(&record, Mode::Curve, 2_001),
                    8_000
                )
                .is_err()
        );
        assert_eq!(record.phase, Phase::RestorePending);
    }
}
