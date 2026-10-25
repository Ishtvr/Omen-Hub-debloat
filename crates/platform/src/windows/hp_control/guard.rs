//! Real guardian consumer. Readiness requires the protected same build and a
//! START-only handle to the exact qualified OEM service. No STOP path exists.
use super::*;
use crate::windows::guardian::{self, Outcome, ParentProcess, Request, Target};
use omen_core::{
    guardian::{ParentState, Trigger},
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
    result: Result<Outcome, String>,
}
struct ReturnTarget {
    journal: crate::ownership_store::OwnershipJournal,
    backend: ServiceController,
    owner: Owner,
    expected: Identity,
}
impl Target for ReturnTarget {
    fn record(&mut self) -> Result<Option<Record>, String> {
        self.journal.load()
    }
    fn recover(&mut self, _trigger: Trigger, parent: ParentState) -> Result<Phase, String> {
        // The parent's lifetime mutex is abandoned only after actual exit.
        // Pipe loss/qualification expiry may revoke a still-live bounded scope.
        let _lifetime = if parent == ParentState::Exited {
            Some(super::super::ControlLease::hp_ownership()?)
        } else {
            None
        };
        let _hardware = super::super::ControlLease::acquire()?;
        require_hardware_recovery_complete()?;
        ownership::release(
            &mut self.backend,
            &mut self.journal,
            &self.owner,
            &self.expected,
        )
        .map(|r| r.phase)
        .map_err(|e| serde_json::to_string(&e).unwrap_or(e.error))
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
        start_requests: 0,
    };
    let mut target = ReturnTarget {
        journal,
        backend,
        owner: request.owner.clone(),
        expected: request.identity.clone(),
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
