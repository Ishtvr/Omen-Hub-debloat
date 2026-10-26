//! Real guardian consumer. Readiness requires the protected same build and a
//! START-only handle to the exact qualified OEM service. No STOP path exists.
use super::*;
use crate::windows::guardian::{self, Outcome, ParentProcess, Request, Target};
use omen_core::{
    guardian::{Observation, ParentState, Trigger},
    ownership::{Journal, Phase, Record},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    schema: u32,
    owner: Owner,
    guardian_pid: u32,
    recorded_at_unix_seconds: u64,
    start_requested: bool,
    #[serde(default)]
    cooling_phase: Option<omen_core::cooling_override::Phase>,
    #[serde(default)]
    cooling_error: Option<String>,
    result: Result<Outcome, String>,
}
struct ReturnTarget {
    journal: crate::ownership_store::OwnershipJournal,
    backend: ServiceController,
    owner: Owner,
    expected: Identity,
    cooling: cooling::Cooling,
    last_maintenance: Option<Instant>,
    awaiting_cooling_after_lock: bool,
    cooling_error: Option<String>,
}
impl Target for ReturnTarget {
    fn record(&mut self) -> Result<Option<Record>, String> {
        self.journal.load()
    }
    fn observation(&mut self) -> Result<Option<Observation>, String> {
        let service = self.journal.load()?;
        let cooling = self.cooling.load()?;
        Observation::coupled(
            &self.owner,
            &self.expected,
            service.as_ref(),
            cooling.as_ref(),
        )
    }
    fn maintain(&mut self) -> Result<(), String> {
        if self
            .last_maintenance
            .is_some_and(|v| v.elapsed() < Duration::from_secs(2))
        {
            return Ok(());
        }
        self.last_maintenance = Some(Instant::now());
        // A service operation can temporarily own the hardware lease. The fan
        // worker reports that wait explicitly and renews maximum afterwards.
        let _hardware = match super::super::ControlLease::acquire() {
            Ok(lease) => lease,
            Err(error) if error == "Another Omen Hub++ hardware transaction is running" => {
                self.awaiting_cooling_after_lock = true;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        drop(_hardware);
        if self.awaiting_cooling_after_lock {
            self.awaiting_cooling_after_lock = false;
            return Ok(()); // one normal two-second helper cycle to acknowledge
        }
        let result = self.cooling.maintain(&self.owner, &self.expected);
        self.cooling_error = result.as_ref().err().cloned();
        result
    }
    fn recover(&mut self, _trigger: Trigger, parent: ParentState) -> Result<Phase, String> {
        // The parent's lifetime mutex is abandoned only after actual exit.
        // Pipe loss/qualification expiry may revoke a still-live bounded scope.
        let _lifetime = if parent == ParentState::Exited {
            Some(super::super::ControlLease::hp_ownership()?)
        } else {
            None
        };
        handoff::return_session(
            &mut self.backend,
            &mut self.journal,
            &self.cooling,
            &self.owner,
            &self.expected,
        )
        .map(|r| r.service.map_or(Phase::Restored, |s| s.phase))
    }
}
pub(super) fn run() -> Result<(), String> {
    let input = std::io::stdin();
    let request: Request = guardian::receive_request(&input)?;
    let parent = ParentProcess::authenticate(&request)?;
    let installation = super::super::startup::inspect_installed_binary(true)?;
    let directory = installation
        .executable
        .parent()
        .ok_or("Protected guardian directory is missing")?;
    let journal = crate::ownership_store::OwnershipJournal::at(directory.to_path_buf())?;
    // Decode any existing record before claiming readiness; never treat a bad
    // recovery ledger as an empty one and let the parent issue STOP.
    let _ = journal.load()?;
    let cooling = cooling::Cooling::open(true)?;
    let duty = cooling
        .load()?
        .ok_or("Guardian needs a durable cooling checkpoint before readiness")?;
    if duty.owner != request.owner
        || duty.original_service.identity != request.identity
        || duty.phase != omen_core::cooling_override::Phase::PendingMaximum
        || !duty
            .helper
            .binary_sha256
            .eq_ignore_ascii_case(&installation.sha256)
    {
        return Err("Guardian cooling readiness belongs to a different session/build".into());
    }
    super::super::fan_task::verify(&duty.helper.binary_sha256, false)?;
    let service = Service::open(READ_ACCESS | SERVICE_START)?
        .ok_or("Guardian cannot open the original HPOmenCap")?;
    let (snapshot, _, image) = audit(&service)?;
    if snapshot.identity != request.identity {
        return Err("Guardian controller identity differs from its owner".into());
    }
    let backend = ServiceController {
        service,
        identity: snapshot.identity,
        _image: image,
        guardian: None,
        cooling_owner: None,
        start_requests: 0,
    };
    let mut target = ReturnTarget {
        journal,
        backend,
        owner: request.owner.clone(),
        expected: request.identity.clone(),
        cooling,
        last_maintenance: None,
        awaiting_cooling_after_lock: false,
        cooling_error: None,
    };
    guardian::write_frame(&mut std::io::stdout(), &guardian::ready(&request)?)?;
    let result = guardian::monitor(&request, &parent, &input, &mut target);
    let report = Report {
        schema: 1,
        owner: request.owner,
        guardian_pid: std::process::id(),
        recorded_at_unix_seconds: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |t| t.as_secs()),
        start_requested: target.backend.start_requests != 0,
        cooling_phase: target
            .cooling
            .load()
            .ok()
            .flatten()
            .filter(|r| r.owner == target.owner)
            .map(|r| r.phase),
        cooling_error: target.cooling_error,
        result,
    };
    crate::profile_store::atomic_json(&directory.join("controller-guardian-last.json"), &report)?;
    report.result.map(|_| ())
}
pub(super) fn last_report() -> Result<Option<Report>, String> {
    let installation = super::super::startup::inspect_installed_binary(false)?;
    let path = installation
        .executable
        .parent()
        .ok_or("Protected guardian directory is absent")?
        .join("controller-guardian-last.json");
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = vec![];
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16384 {
        return Err("Guardian result exceeds its bound".into());
    }
    let report: Report = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if report.schema != 1 || report.guardian_pid == 0 {
        return Err("Guardian result identity is invalid".into());
    }
    report.owner.validate()?;
    Ok(Some(report))
}
