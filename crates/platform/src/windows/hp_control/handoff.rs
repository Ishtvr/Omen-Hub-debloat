//! Explicit, bounded migration qualification. Not used by dashboard or logon.
use super::cooling::Cooling;
use super::*;
use omen_core::{
    cooling_override::{Checkpoint, RecoveryReadiness},
    ownership::{Backend, Journal, Record},
};

#[derive(Debug, Serialize)]
pub struct RecoveryReport {
    pub service: Option<Record>,
    pub cooling: Option<Checkpoint>,
}

/// Shared normal/crash return. Service restore is serialized with hardware
/// recovery, then the own helper acknowledges its previous preference.
pub(super) fn return_session(
    backend: &mut ServiceController,
    journal: &mut crate::ownership_store::OwnershipJournal,
    cooling: &Cooling,
    owner: &Owner,
    identity: &Identity,
) -> Result<RecoveryReport, String> {
    let duty = cooling.load()?.filter(|r| &r.owner == owner);
    if duty.is_some() {
        // Under the same lease as STOP: a late owner can no longer acquire
        // after its guardian has started returning the coupled session.
        cooling.request_return(owner, identity)?;
    }
    let helper_error = duty
        .as_ref()
        .filter(|r| !r.phase.complete())
        .and_then(|_| cooling.ensure_helper(owner, identity).err());
    let service = {
        // A synchronous SCM call can occupy the owner for up to 40 seconds.
        // This waits for that one operation; it never repeats a STOP.
        let _hardware = hardware_lease(Duration::from_secs(45))?;
        require_hardware_recovery_complete()?;
        let record = journal.load()?;
        match record {
            Some(record) if &record.owner == owner => Some(
                ownership::release(backend, journal, owner, identity)
                    .map_err(|e| serde_json::to_string(&e).unwrap_or(e.error))?,
            ),
            other => {
                if other.as_ref().is_some_and(|r| !r.phase.complete()) {
                    return Err("Another controller session blocks return".into());
                }
                let duty = duty.as_ref().ok_or("No duty belongs to this session")?;
                let current = backend.inspect()?;
                if &current.identity != identity || current.state != duty.original_service.state {
                    return Err(
                        "Unjournaled controller state changed; no service command authorized"
                            .into(),
                    );
                }
                None
            }
        }
    };
    if let Some(error) = helper_error {
        return Err(format!(
            "OEM returned; cooling responsibility retained: {error}"
        ));
    }
    let cooling = if let Some(duty) = duty {
        Some(cooling.restore(owner, identity, || {
            require_hardware_recovery_complete()?;
            let actual = backend.inspect()?;
            Ok(RecoveryReadiness {
                service_returned: &actual.identity == identity
                    && (actual.state == duty.original_service.state
                        || (duty.original_service.state == State::Stopped
                            && actual.state == State::Running)),
                hardware_recovery_clear: true,
            })
        })?)
    } else {
        None
    };
    Ok(RecoveryReport { service, cooling })
}

/// Explicit repair accepts only a dead owner. No STOP permission is opened.
pub fn recover_controller() -> Result<RecoveryReport, String> {
    let _lifetime = super::super::ControlLease::hp_ownership()?;
    let mut journal = protected_journal()?;
    let cooling = Cooling::open(true)?;
    let service = journal.load()?;
    let duty = cooling.load()?;
    let owner = duty
        .as_ref()
        .filter(|c| !c.phase.complete())
        .map(|c| &c.owner)
        .or_else(|| service.as_ref().map(|r| &r.owner))
        .or_else(|| duty.as_ref().map(|c| &c.owner))
        .ok_or("No interrupted controller/cooling session exists")?
        .clone();
    if owner.user != super::super::current_user_sid()? {
        return Err("Recovery belongs to another user".into());
    }
    let identity = duty
        .as_ref()
        .filter(|c| c.owner == owner)
        .map(|c| &c.original_service.identity)
        .or_else(|| {
            service
                .as_ref()
                .filter(|r| r.owner == owner)
                .map(|r| &r.original.identity)
        })
        .ok_or("Recovery identity is unavailable")?
        .clone();
    let observed = omen_core::guardian::Observation::coupled(
        &owner,
        &identity,
        service.as_ref(),
        duty.as_ref(),
    )?;
    if observed.as_ref().is_some_and(|o| o.complete) {
        return Ok(RecoveryReport {
            service,
            cooling: duty,
        });
    }
    if !matches!(liveness(&owner), ownership::OwnerLiveness::Dead) {
        return Err(
            "Controller owner is alive or inaccessible; no recovery command was sent".into(),
        );
    }
    let service = Service::open(READ_ACCESS | SERVICE_START)?
        .ok_or("Original controller is absent; recovery retained")?;
    let (snapshot, _, image) = audit(&service)?;
    if snapshot.identity != identity {
        return Err("Recovery service identity changed".into());
    }
    let mut backend = ServiceController {
        service,
        identity: identity.clone(),
        _image: image,
        guardian: None,
        cooling_owner: None,
        start_requests: 0,
    };
    return_session(&mut backend, &mut journal, &cooling, &owner, &identity)
}

pub struct ScopedSession {
    backend: ServiceController,
    journal: crate::ownership_store::OwnershipJournal,
    cooling: Cooling,
    owner: Owner,
    identity: Identity,
    began: Instant,
    finished: bool,
    auto_restore: bool,
    _lifetime: super::super::ControlLease,
}
impl ScopedSession {
    pub fn acquire_for_qualification(
        inventory: &Inventory,
        consent: ownership::Consent,
    ) -> Result<Self, String> {
        let lifetime = super::super::ControlLease::hp_ownership()?;
        if !known_controllers()?.is_empty() {
            return Err(
                "Close the original Hub and other tuning controllers before handover".into(),
            );
        }
        check_sensors(inventory, false)?;
        let installation = super::super::startup::inspect_installed_binary(true)?;
        let mut backend = ServiceController::prepare_explicit_session(inventory)?;
        let identity = backend.identity().clone();
        let owner = current_owner_identity()?;
        let journal = protected_journal()?;
        let cooling = Cooling::open(true)?;
        {
            let _hardware = hardware_lease(Duration::from_secs(2))?;
            require_hardware_recovery_complete()?;
            if journal.load()?.is_some_and(|r| !r.phase.complete()) {
                return Err("Pending service recovery blocks qualification".into());
            }
            let original = backend.inspect()?;
            if original.state != State::Running || !original.can_stop || original.has_dependents {
                return Err("This temporary migration trial requires the original running/stoppable service".into());
            }
            cooling.prepare(owner.clone(), original, &installation.sha256)?;
        }
        let mut scope = Self {
            backend,
            journal,
            cooling,
            owner,
            identity,
            began: Instant::now(),
            finished: false,
            auto_restore: true,
            _lifetime: lifetime,
        };
        let acquire = (|| {
            let request = super::super::guardian::Request {
                schema: 1,
                owner: scope.owner.clone(),
                identity: scope.identity.clone(),
            };
            scope.backend.guardian = Some(super::super::guardian::GuardianProcess::spawn_self(
                &request,
                &["hp-control-guardian".into()],
            )?);
            scope.cooling.maximum(&scope.owner, &scope.identity)?;
            await_maximum_rpm(inventory)?;
            let _hardware = hardware_lease(Duration::from_secs(2))?;
            require_hardware_recovery_complete()?;
            scope.guard_alive()?;
            if scope.began.elapsed() >= Duration::from_secs(35) {
                return Err("Insufficient qualification time remains for service handover".into());
            }
            if !known_controllers()?.is_empty() {
                return Err("Another tuning controller appeared before handover".into());
            }
            let original = scope
                .cooling
                .load()?
                .ok_or("Cooling checkpoint disappeared")?
                .original_service;
            if scope.backend.inspect()? != original {
                return Err("Original service changed since cooling preparation".into());
            }
            scope.backend.cooling_owner = Some(scope.owner.clone());
            ownership::acquire(
                &mut scope.backend,
                &mut scope.journal,
                scope.owner.clone(),
                &scope.identity,
                consent,
            )
            .map_err(|e| serde_json::to_string(&e).unwrap_or(e.error))?;
            Ok(())
        })();
        let acquire = acquire.and_then(|()| {
            scope
                .cooling
                .await_maximum_ack(&scope.owner, &scope.identity)?;
            scope.verify()
        });
        if let Err(error) = acquire {
            let recovery = scope.release();
            return Err(match recovery {
                Ok(_) => error,
                Err(restore) => format!("{error}; recovery pending: {restore}"),
            });
        }
        Ok(scope)
    }
    fn guard_alive(&mut self) -> Result<(), String> {
        self.backend
            .guardian
            .as_mut()
            .ok_or("Controller guardian is absent")?
            .ensure_running()
    }
    pub fn verify(&mut self) -> Result<(), String> {
        if self.finished || self.began.elapsed() >= Duration::from_secs(50) {
            return Err("Bounded controller session ended or reached its return deadline".into());
        }
        self.guard_alive()?;
        if !known_controllers()?.is_empty() {
            return Err("Another tuning controller became active".into());
        }
        self.cooling.verify_maximum(&self.owner, &self.identity)?;
        let _hardware = hardware_lease(Duration::from_secs(2))?;
        ownership::verify_owned(
            &mut self.backend,
            &self.journal,
            &self.owner,
            &self.identity,
        )
        .map(|_| ())
        .map_err(|e| serde_json::to_string(&e).unwrap_or(e.error))
    }
    pub fn release(&mut self) -> Result<RecoveryReport, String> {
        self.auto_restore = false;
        let result = return_session(
            &mut self.backend,
            &mut self.journal,
            &self.cooling,
            &self.owner,
            &self.identity,
        );
        self.finished = result.is_ok();
        if self.finished {
            if let Some(guardian) = self.backend.guardian.as_mut() {
                guardian.disconnect();
                if !guardian.wait_for_exit(Duration::from_secs(3))? {
                    return Err(
                        "Cooling/service returned but guardian exit is still pending".into(),
                    );
                }
            }
        }
        result
    }
}
impl Drop for ScopedSession {
    fn drop(&mut self) {
        if !self.finished && self.auto_restore {
            let _ = self.release();
        }
        // Drop closes the private guardian lease, never kills the guardian.
    }
}

fn check_sensors(
    inventory: &Inventory,
    maximum: bool,
) -> Result<super::super::CoolingSnapshot, String> {
    let sample = super::super::read_cooling_snapshot(inventory)?;
    let safe = sample.temperature_batch_valid
        && sample.power_before.ac_connected == Some(true)
        && sample.power_after.ac_connected == Some(true)
        && sample
            .cpu
            .result
            .as_ref()
            .ok()
            .and_then(|v| v.temperature_c)
            .is_some_and(|v| v.is_finite() && (1.0..80.0).contains(&v))
        && sample
            .gpu
            .result
            .as_ref()
            .ok()
            .and_then(|v| v.temperature_c)
            .is_some_and(|v| (1..75).contains(&v))
        && sample.fans.result.as_ref().is_ok_and(|v| {
            v.ir_temperature.as_ref().is_ok_and(|t| t.celsius < 50)
                && (!maximum
                    || (v.speeds.cpu_rpm.is_some_and(|v| v >= 4000)
                        && v.speeds.gpu_rpm.is_some_and(|v| v >= 4000)))
        });
    if !safe {
        return Err("Qualification requires AC, fresh CPU <80 C / GPU <75 C / IR <50 C and maximum RPM when owned".into());
    }
    Ok(sample)
}
fn await_maximum_rpm(inventory: &Inventory) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        // Temperature/AC faults abort immediately. Only mechanical fan ramp-up
        // gets a short observation budget after a successful maximum command.
        let sample = check_sensors(inventory, false)?;
        if sample.fans.result.as_ref().is_ok_and(|v| {
            v.speeds.cpu_rpm.is_some_and(|v| v >= 4000)
                && v.speeds.gpu_rpm.is_some_and(|v| v >= 4000)
        }) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("Both fans did not reach the qualification RPM floor".into());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
#[derive(Debug, Serialize)]
pub struct Qualification {
    pub schema: u32,
    pub samples_while_hp_stopped: Vec<super::super::CoolingSnapshot>,
    pub observation: Result<(), String>,
    pub recovery: Result<RecoveryReport, String>,
    pub performance_settings_written: bool,
    pub complete_replacement_qualified: bool,
}
/// Development-only opt-in command: one service stop, six bounded read batches,
/// continuous own maximum cooling, then exact original return. No tuning SET.
pub fn qualify(inventory: &Inventory) -> Result<Qualification, String> {
    let mut session =
        ScopedSession::acquire_for_qualification(inventory, ownership::Consent::ExplicitSession)?;
    let mut samples = Vec::new();
    let observation = (|| {
        for _ in 0..6 {
            session.verify()?;
            samples.push(check_sensors(inventory, true)?);
            std::thread::sleep(Duration::from_secs(2));
        }
        session.verify()
    })();
    let recovery = session.release();
    Ok(Qualification {
        schema: 1,
        samples_while_hp_stopped: samples,
        observation,
        recovery,
        performance_settings_written: false,
        complete_replacement_qualified: false,
    })
}
