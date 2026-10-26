//! Fixed HPOmenCap inspection and explicit session backend. No driver/service
//! configuration is changed. The dashboard never acquires this backend itself.
mod cooling;
mod guard;
mod handoff;
pub use guard::Report as GuardianReport;
pub use handoff::{ScopedSession, qualify, recover_controller};
use omen_core::{
    Inventory,
    ownership::{self, Identity, Owner, Snapshot, State, Subject},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::OpenOptions,
    io::Read,
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_INVALID_PARAMETER, ERROR_MORE_DATA, ERROR_SERVICE_DOES_NOT_EXIST,
        FILETIME, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Storage::FileSystem::FILE_SHARE_READ,
    System::{
        Services::*,
        SystemInformation::GetWindowsDirectoryW,
        Threading::{
            GetCurrentProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW, WaitForSingleObject,
        },
    },
};

pub const SERVICE_NAME: &str = "HPOmenCap";
const IMAGE_SHA256: &str = "9eeccbdd7beaa9c33930551853a572ab1e56e2e9a72f6453dc53e190284fe0c8";
const IMAGE_RELATIVE: &str = "System32\\DriverStore\\FileRepository\\hpomencustomcapcomp.inf_amd64_38ec91e432e20182\\x64\\OmenCap\\OmenCap.exe";
const READ_ACCESS: u32 = SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG | SERVICE_ENUMERATE_DEPENDENTS;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
struct Service(SC_HANDLE);
impl Drop for Service {
    fn drop(&mut self) {
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}
impl Service {
    fn open(access: u32) -> Result<Option<Self>, String> {
        // SAFETY: Local machine/database only, no create/change permissions.
        let manager = unsafe { OpenSCManagerW(ptr::null(), ptr::null(), SC_MANAGER_CONNECT) };
        if manager.is_null() {
            return Err(format!(
                "OpenSCManager(local, connect): {}",
                std::io::Error::last_os_error()
            ));
        }
        let manager = Service(manager);
        let service = unsafe { OpenServiceW(manager.0, wide(SERVICE_NAME).as_ptr(), access) };
        if service.is_null() {
            let error = std::io::Error::last_os_error();
            return if error.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST as i32) {
                Ok(None)
            } else {
                Err(format!(
                    "OpenService(HPOmenCap, access 0x{access:x}): {error}"
                ))
            };
        }
        Ok(Some(Self(service)))
    }
    fn status(&self) -> Result<SERVICE_STATUS_PROCESS, String> {
        let mut status: SERVICE_STATUS_PROCESS = unsafe { std::mem::zeroed() };
        let mut needed = 0;
        // SAFETY: Exact public status structure, fixed information class.
        if unsafe {
            QueryServiceStatusEx(
                self.0,
                SC_STATUS_PROCESS_INFO,
                (&mut status as *mut SERVICE_STATUS_PROCESS).cast(),
                size_of_val(&status) as u32,
                &mut needed,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if status.dwServiceType != SERVICE_WIN32_OWN_PROCESS {
            return Err("HPOmenCap is not the qualified user-mode service type".into());
        }
        Ok(status)
    }
}
fn state(raw: u32) -> State {
    match raw {
        SERVICE_RUNNING => State::Running,
        SERVICE_STOPPED => State::Stopped,
        _ => State::Transitioning,
    }
}
/// Fast query-only guard. Only ERROR_SERVICE_DOES_NOT_EXIST means absence.
pub fn service_state() -> Result<Option<State>, String> {
    Service::open(SERVICE_QUERY_STATUS)?
        .map(|service| service.status().map(|s| state(s.dwCurrentState)))
        .transpose()
}
fn expected_path() -> Result<PathBuf, String> {
    let mut directory = [0u16; 32768];
    let count =
        unsafe { GetWindowsDirectoryW(directory.as_mut_ptr(), directory.len() as u32) } as usize;
    if count == 0 || count >= directory.len() {
        return Err("Windows directory is unavailable".into());
    }
    Ok(
        PathBuf::from(String::from_utf16(&directory[..count]).map_err(|e| e.to_string())?)
            .join(IMAGE_RELATIVE),
    )
}

/// The returned pointer must stay inside the live, aligned configuration buffer.
fn utf16_tail(buffer: &[usize], pointer: *const u16) -> Result<&[u16], String> {
    let start = buffer.as_ptr() as usize;
    let end = start + size_of_val(buffer);
    let address = pointer as usize;
    if address < start || address >= end || address % align_of::<u16>() != 0 {
        return Err("Service configuration pointer is outside its buffer".into());
    }
    // SAFETY: Validated address/alignment/remaining length in owned storage.
    Ok(unsafe { std::slice::from_raw_parts(pointer, (end - address) / 2) })
}
fn text(buffer: &[usize], pointer: *const u16) -> Result<String, String> {
    if pointer.is_null() {
        return Ok(String::new());
    }
    let cells = utf16_tail(buffer, pointer)?;
    let end = cells
        .iter()
        .position(|c| *c == 0)
        .ok_or("Unterminated service configuration text")?;
    String::from_utf16(&cells[..end]).map_err(|e| e.to_string())
}
fn dependencies(buffer: &[usize], pointer: *const u16) -> Result<Vec<String>, String> {
    if pointer.is_null() {
        return Ok(vec![]);
    }
    let mut rest = utf16_tail(buffer, pointer)?;
    let mut names = vec![];
    loop {
        let end = rest
            .iter()
            .position(|c| *c == 0)
            .ok_or("Unterminated service dependency list")?;
        if end == 0 {
            // QUERY_SERVICE_CONFIG permits an empty string when there are no
            // dependencies. Do not read into the following configuration field.
            return Ok(names);
        }
        if names.len() >= 32 {
            return Err("Too many service dependencies".into());
        }
        names.push(String::from_utf16(&rest[..end]).map_err(|e| e.to_string())?);
        rest = &rest[end + 1..];
    }
}

#[derive(Debug, Serialize)]
pub struct Configuration {
    pub executable: String,
    pub account: String,
    pub start_type: u32,
    pub service_type: u32,
    pub dependencies: Vec<String>,
    pub load_group: String,
    pub error_control: u32,
}
impl Service {
    fn configuration(&self) -> Result<Configuration, String> {
        // Windows documents an 8-KiB maximum; usize preserves native alignment.
        let mut buffer = [0usize; 8192 / size_of::<usize>()];
        let mut needed = 0;
        if unsafe {
            QueryServiceConfigW(
                self.0,
                buffer.as_mut_ptr().cast(),
                size_of_val(&buffer) as u32,
                &mut needed,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let config = unsafe { &*buffer.as_ptr().cast::<QUERY_SERVICE_CONFIGW>() };
        Ok(Configuration {
            executable: text(&buffer, config.lpBinaryPathName)?,
            account: text(&buffer, config.lpServiceStartName)?,
            start_type: config.dwStartType,
            service_type: config.dwServiceType,
            dependencies: dependencies(&buffer, config.lpDependencies)?,
            load_group: text(&buffer, config.lpLoadOrderGroup)?,
            error_control: config.dwErrorControl,
        })
    }
    fn has_dependents(&self) -> Result<bool, String> {
        let mut needed = 0;
        let mut count = 0;
        if unsafe {
            EnumDependentServicesW(
                self.0,
                SERVICE_STATE_ALL,
                ptr::null_mut(),
                0,
                &mut needed,
                &mut count,
            )
        } != 0
        {
            return Ok(count != 0);
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_MORE_DATA as i32) {
            Ok(true)
        } else {
            Err(error.to_string())
        }
    }
}
fn birth(handle: HANDLE) -> Result<u64, String> {
    let mut created: FILETIME = unsafe { std::mem::zeroed() };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    if unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}
struct Process(HANDLE);
impl Drop for Process {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn process_birth(pid: u32, expected: &std::path::Path) -> Result<u64, String> {
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let process = Process(handle);
    let mut path = [0u16; 32768];
    let mut count = path.len() as u32;
    if unsafe { QueryFullProcessImageNameW(process.0, 0, path.as_mut_ptr(), &mut count) } == 0
        || count as usize >= path.len()
    {
        return Err("HPOmenCap process image is unavailable".into());
    }
    if !String::from_utf16(&path[..count as usize])
        .map_err(|e| e.to_string())?
        .eq_ignore_ascii_case(&expected.to_string_lossy())
    {
        return Err("Running HPOmenCap process image differs from its configuration".into());
    }
    birth(process.0)
}

fn model_supported(inventory: &Inventory) -> bool {
    inventory.manufacturer == "HP"
        && inventory.board == "8E35"
        && inventory.bios == "F.13"
        && inventory.cpus == ["AMD Ryzen 9 8940HX with Radeon Graphics"]
}

fn audit(service: &Service) -> Result<(Snapshot, Configuration, std::fs::File), String> {
    let before = service.status()?;
    let mut config = service.configuration()?;
    let path = expected_path()?;
    validate_configuration(&config, &path)?;
    // Keep the image pinned without write/delete sharing during inspection or
    // ownership. A concurrent replacement must not retarget a checked command.
    let mut image = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let length = image.metadata().map_err(|e| e.to_string())?.len();
    if length == 0 || length > 16 * 1024 * 1024 {
        return Err("HPOmenCap image exceeds its inspection bound".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 8192];
    let mut count = 0u64;
    loop {
        let n = image.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        };
        count += n as u64;
        if count > 16 * 1024 * 1024 {
            return Err("HPOmenCap image changed during inspection".into());
        }
        hash.update(&buffer[..n]);
    }
    let image_sha256 = format!("{:x}", hash.finalize());
    if image_sha256 != IMAGE_SHA256 || count != length {
        return Err("HPOmenCap image hash differs from the qualified version".into());
    }
    config.executable = path.to_string_lossy().to_ascii_lowercase();
    config.account.make_ascii_lowercase();
    config.load_group.make_ascii_lowercase();
    let identity = Identity {
        subject: Subject::HpOmenCap,
        image_sha256,
        configuration_sha256: format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&config).map_err(|e| e.to_string())?)
        ),
    };
    let stopped = before.dwCurrentState == SERVICE_STOPPED;
    let running = before.dwCurrentState == SERVICE_RUNNING;
    let started = if running {
        process_birth(before.dwProcessId, &path)?
    } else {
        0
    };
    let after = service.status()?;
    if before.dwCurrentState != after.dwCurrentState
        || (running && before.dwProcessId != after.dwProcessId)
    {
        return Err("HPOmenCap changed state during inspection".into());
    }
    Ok((
        Snapshot {
            identity,
            state: state(after.dwCurrentState),
            process_id: if stopped { 0 } else { after.dwProcessId },
            process_started: started,
            can_stop: after.dwControlsAccepted & SERVICE_ACCEPT_STOP != 0,
            has_dependents: service.has_dependents()?,
        },
        config,
        image,
    ))
}
fn validate_configuration(config: &Configuration, path: &std::path::Path) -> Result<(), String> {
    let configured = config.executable.trim();
    let configured = configured
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(configured);
    if !configured.eq_ignore_ascii_case(&path.to_string_lossy())
        || config.service_type != SERVICE_WIN32_OWN_PROCESS
        || config.start_type != SERVICE_AUTO_START
        || !config.account.eq_ignore_ascii_case("LocalSystem")
        || !config.dependencies.is_empty()
    {
        return Err("HPOmenCap configuration differs from the measured fixed contract".into());
    }
    Ok(())
}

/// Native backend for the explicit handover engine. Not constructed by profile
/// Apply, dashboard startup, or status/plan reads. No arbitrary service target.
struct ServiceController {
    service: Service,
    identity: Identity,
    _image: std::fs::File,
    guardian: Option<super::guardian::GuardianProcess>,
    cooling_owner: Option<Owner>,
    start_requests: u32,
}
impl ServiceController {
    pub fn prepare_explicit_session(inventory: &Inventory) -> Result<Self, String> {
        if !model_supported(inventory) {
            return Err("HPOmenCap handover is not qualified for this device".into());
        }
        let service = Service::open(READ_ACCESS | SERVICE_STOP | SERVICE_START)?
            .ok_or("HPOmenCap is not installed")?;
        let (snapshot, _, image) = audit(&service)?;
        Ok(Self {
            service,
            identity: snapshot.identity,
            _image: image,
            guardian: None,
            cooling_owner: None,
            start_requests: 0,
        })
    }
    pub fn identity(&self) -> &Identity {
        &self.identity
    }
    fn wait(&self, target: State) -> Result<(), String> {
        // This bounds status observation, not the synchronous SCM API itself:
        // Windows may spend up to 30 s delivering a service control request.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if state(self.service.status()?.dwCurrentState) == target {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("HPOmenCap state did not settle within its observation budget".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
impl ownership::Backend for ServiceController {
    fn inspect(&mut self) -> Result<Snapshot, String> {
        let snapshot = audit(&self.service)?.0;
        if snapshot.identity != self.identity {
            return Err("HPOmenCap identity/configuration changed".into());
        }
        Ok(snapshot)
    }
    fn stop(&mut self, expected: &Snapshot) -> Result<(), String> {
        let owner = self
            .cooling_owner
            .as_ref()
            .ok_or("Durable cooling responsibility is required before STOP")?;
        cooling::Cooling::open(true)?.verify_maximum(owner, &expected.identity)?;
        self.guardian
            .as_mut()
            .ok_or("A ready independent guardian is required before STOP")?
            .ensure_running()?;
        let current = self.inspect()?;
        if &current != expected
            || current.state != State::Running
            || !current.can_stop
            || current.has_dependents
        {
            return Err("HPOmenCap STOP precondition changed".into());
        }
        let mut status: SERVICE_STATUS = unsafe { std::mem::zeroed() };
        self.guardian.as_mut().unwrap().ensure_running()?;
        // SAFETY: Fixed verified user-mode service handle; exact STOP only.
        if unsafe { ControlService(self.service.0, SERVICE_CONTROL_STOP, &mut status) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        self.wait(State::Stopped)?;
        self.guardian.as_mut().unwrap().ensure_running()
    }
    fn start(&mut self, expected: &Identity) -> Result<(), String> {
        let current = self.inspect()?;
        if &current.identity != expected || current.state != State::Stopped {
            return Err("HPOmenCap START precondition changed".into());
        }
        // SAFETY: Fixed verified user-mode service, no service arguments.
        self.start_requests += 1;
        if unsafe { StartServiceW(self.service.0, 0, ptr::null()) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        self.wait(State::Running)
    }
}

#[derive(Debug, Serialize)]
pub struct Inspection {
    pub schema: u32,
    pub service_name: &'static str,
    pub state: Result<Option<State>, String>,
    pub model_supported: bool,
    pub audit: Result<Option<(Snapshot, Configuration)>, String>,
    pub known_controller_processes: Result<Vec<String>, String>,
    pub recovery: Result<Option<ownership::Record>, String>,
    pub cooling_recovery: Result<Option<omen_core::cooling_override::Checkpoint>, String>,
    pub last_guardian: Result<Option<GuardianReport>, String>,
    pub automatic_takeover_enabled: bool,
    pub services_changed: bool,
    pub hardware_applied: bool,
}
/// Read rights only: neither START nor STOP access is opened by this function.
pub fn inspect(inventory: &Inventory) -> Inspection {
    let state = service_state();
    let audit = Service::open(READ_ACCESS).and_then(|service| {
        service
            .map(|service| audit(&service).map(|(snapshot, config, _)| (snapshot, config)))
            .transpose()
    });
    Inspection {
        schema: 1,
        service_name: SERVICE_NAME,
        state,
        model_supported: model_supported(inventory),
        audit,
        known_controller_processes: known_controllers(),
        recovery: recovery_record(),
        cooling_recovery: cooling::Cooling::open(false).and_then(|c| c.load()),
        last_guardian: guard::last_report(),
        automatic_takeover_enabled: false,
        services_changed: false,
        hardware_applied: false,
    }
}

/// Enumeration failure is unknown, never an empty list. Process names are
/// conflict observations, not authorization to stop or terminate a process.
pub fn known_controllers() -> Result<Vec<String>, String> {
    use windows_sys::Win32::{
        Foundation::{ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::*,
    };
    const NAMES: &[&str] = &[
        "omencommandcenterbackground.exe",
        "hp.omen.omencommandcenter.exe",
        "omencommandcenter.exe",
        "hp.omen.application.background.tasks.container.exe",
        "universal x86 tuning utility.exe",
        "uxtu.exe",
        "msiafterburner.exe",
    ];
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let snapshot = Process(handle);
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
    let mut names = vec![];
    loop {
        if !ok {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
                return Err(error.to_string());
            }
            break;
        }
        let length = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .ok_or("Process name is unterminated")?;
        let name = String::from_utf16(&entry.szExeFile[..length])
            .map_err(|e| e.to_string())?
            .to_ascii_lowercase();
        if NAMES.contains(&name.as_str()) {
            names.push(name);
        }
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
    }
    names.sort();
    names.dedup();
    Ok(names)
}

pub fn current_owner_identity() -> Result<Owner, String> {
    let user = super::current_user_sid()?;
    let process_id = std::process::id();
    let process_started = birth(unsafe { GetCurrentProcess() })?;
    let boot = super::boot_unix_s();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let session = format!(
        "{:x}",
        Sha256::digest(format!(
            "{user}:{process_id}:{process_started}:{boot}:{now}"
        ))
    );
    Ok(Owner {
        user,
        process_id,
        process_started,
        boot,
        session,
    })
}

fn liveness(owner: &Owner) -> ownership::OwnerLiveness {
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            owner.process_id,
        )
    };
    if handle.is_null() {
        return if std::io::Error::last_os_error().raw_os_error()
            == Some(ERROR_INVALID_PARAMETER as i32)
        {
            ownership::OwnerLiveness::Dead
        } else {
            ownership::OwnerLiveness::Unknown
        };
    }
    let process = Process(handle);
    match unsafe { WaitForSingleObject(process.0, 0) } {
        WAIT_OBJECT_0 => ownership::OwnerLiveness::Dead,
        WAIT_TIMEOUT => match birth(process.0) {
            Ok(created) if created == owner.process_started => ownership::OwnerLiveness::Alive,
            Ok(_) => ownership::OwnerLiveness::Dead,
            Err(_) => ownership::OwnerLiveness::Unknown,
        },
        _ => ownership::OwnerLiveness::Unknown,
    }
}
fn protected_journal() -> Result<crate::ownership_store::OwnershipJournal, String> {
    let installation = super::startup::inspect_installed_binary(true)?;
    crate::ownership_store::OwnershipJournal::at(
        installation
            .executable
            .parent()
            .ok_or("Protected install directory is absent")?
            .to_path_buf(),
    )
}
pub fn recovery_record() -> Result<Option<ownership::Record>, String> {
    use ownership::Journal;
    let installation = super::startup::inspect_installed_binary(false)?;
    crate::ownership_store::OwnershipJournal::at(
        installation
            .executable
            .parent()
            .ok_or("Protected install directory is absent")?
            .to_path_buf(),
    )?
    .load()
}

pub fn run_guardian_process() -> Result<(), String> {
    guard::run()
}

/// Retry only mutex acquisition, never an ambiguous service control request.
fn hardware_lease(budget: Duration) -> Result<super::ControlLease, String> {
    let deadline = Instant::now() + budget;
    loop {
        match super::ControlLease::acquire() {
            Ok(lease) => return Ok(lease),
            Err(error)
                if error == "Another Omen Hub++ hardware transaction is running"
                    && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => return Err(error),
        }
    }
}
fn require_hardware_recovery_complete() -> Result<(), String> {
    hardware_recovery_gate(crate::profile_store::ProfileStore::user_data()?.pending())
}
fn hardware_recovery_gate(pending: Result<bool, String>) -> Result<(), String> {
    match pending {
        Ok(false) => Ok(()),
        Ok(true) => Err(
            "Restore the pending hardware transaction before changing controller ownership".into(),
        ),
        Err(error) => Err(format!(
            "Hardware recovery state is unknown; controller unchanged: {error}"
        )),
    }
}

/// Guard new CPU power writes only. Recovery intentionally does not use this
/// preflight: returning captured originals must remain possible with HP active.
pub fn check_cpu_power_settings(settings: &[omen_core::profiles::Setting]) -> Result<(), String> {
    use omen_core::features::Feature;
    if !settings.iter().any(|s| {
        matches!(
            s.feature,
            Feature::CpuPptLimitMw | Feature::CpuSlowPptLimitMw
        )
    }) {
        return Ok(());
    }
    check_cpu_power_state(service_state(), false)
}
pub(super) fn check_cpu_power_write(feature: omen_core::features::Feature) -> Result<(), String> {
    use omen_core::features::Feature;
    if !matches!(feature, Feature::CpuPptLimitMw | Feature::CpuSlowPptLimitMw) {
        return Ok(());
    }
    check_cpu_power_state(service_state(), true)
}
/// A briefly stopped auto-start OEM service is not a qualified logon handover.
/// Independent startup remains possible after the original service is absent.
pub(super) fn cpu_power_startup_safe(feature: omen_core::features::Feature) -> bool {
    use omen_core::features::Feature;
    if !matches!(feature, Feature::CpuPptLimitMw | Feature::CpuSlowPptLimitMw) {
        return true;
    }
    startup_safe_for_status(service_state())
}
fn startup_safe_for_status(status: Result<Option<State>, String>) -> bool {
    matches!(status, Ok(None))
}
fn check_cpu_power_state(
    current: Result<Option<State>, String>,
    writing: bool,
) -> Result<(), String> {
    match current {
        Ok(None | Some(State::Stopped)) => Ok(()),
        Ok(Some(_)) if writing => Err(omen_core::message::msg("hp_control.cpu_power_changed", &[])),
        Err(error) if writing => Err(omen_core::message::msg(
            "hp_control.write_unknown",
            &[&error],
        )),
        Ok(Some(State::Running | State::Transitioning)) => Err(omen_core::message::msg(
            "hp_control.cpu_power_conflict",
            &[],
        )),
        Err(error) => Err(omen_core::message::msg("hp_control.unknown", &[&error])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_string_and_multistring_boundaries_are_checked() {
        let mut buffer = [0usize; 8];
        // SAFETY: Aligned integer storage is initialized and exclusively owned.
        let cells = unsafe {
            std::slice::from_raw_parts_mut(
                buffer.as_mut_ptr().cast::<u16>(),
                size_of_val(&buffer) / 2,
            )
        };
        cells[..5].copy_from_slice(&[b'a' as u16, 0, b'b' as u16, 0, 0]);
        let pointer = buffer.as_ptr().cast::<u16>();
        assert_eq!(text(&buffer, pointer).unwrap(), "a");
        assert_eq!(dependencies(&buffer, pointer).unwrap(), ["a", "b"]);
        let outside = (buffer.as_ptr() as usize + size_of_val(&buffer)) as *const u16;
        assert!(text(&buffer, outside).is_err());
        assert!(text(&buffer, (pointer as usize + 1) as *const u16).is_err());
        let last = (buffer.as_ptr() as usize + size_of_val(&buffer) - 2) as *const u16;
        assert!(dependencies(&buffer, last).unwrap().is_empty());
        let unterminated = [usize::MAX; 8];
        assert!(text(&unterminated, unterminated.as_ptr().cast()).is_err());
    }
    #[test]
    fn fixed_service_config_rejects_arguments_drivers_and_changed_accounts() {
        let path = PathBuf::from("C:\\Windows").join(IMAGE_RELATIVE);
        let base = || Configuration {
            executable: format!("\"{}\"", path.display()),
            account: "LocalSystem".into(),
            start_type: SERVICE_AUTO_START,
            service_type: SERVICE_WIN32_OWN_PROCESS,
            dependencies: vec![],
            load_group: String::new(),
            error_control: SERVICE_ERROR_NORMAL,
        };
        validate_configuration(&base(), &path).unwrap();
        for case in 0..6 {
            let mut changed = base();
            match case {
                0 => changed.executable.push_str(" --extra"),
                1 => changed.service_type = SERVICE_KERNEL_DRIVER,
                2 => changed.account = "OtherUser".into(),
                3 => changed.start_type = SERVICE_DISABLED,
                4 => changed.dependencies.push("UnknownDriver".into()),
                _ => changed.executable = "C:\\unrelated.exe".into(),
            }
            assert!(validate_configuration(&changed, &path).is_err());
        }
    }
    #[test]
    fn cpu_power_guard_distinguishes_absence_stopped_and_unknown() {
        for writing in [false, true] {
            assert!(check_cpu_power_state(Ok(None), writing).is_ok());
            assert!(check_cpu_power_state(Ok(Some(State::Stopped)), writing).is_ok());
            assert!(check_cpu_power_state(Ok(Some(State::Running)), writing).is_err());
            assert!(check_cpu_power_state(Ok(Some(State::Transitioning)), writing).is_err());
            assert!(check_cpu_power_state(Err("access denied".into()), writing).is_err());
        }
        let before = check_cpu_power_state(Ok(Some(State::Running)), false).unwrap_err();
        let during = check_cpu_power_state(Ok(Some(State::Running)), true).unwrap_err();
        assert!(before.contains("cpu_power_conflict"));
        assert!(during.contains("cpu_power_changed"));
    }
    #[test]
    fn liveness_uses_kernel_creation_identity_and_never_treats_a_reused_pid_as_alive() {
        let owner = current_owner_identity().unwrap();
        owner.validate().unwrap();
        assert!(matches!(liveness(&owner), ownership::OwnerLiveness::Alive));
        let mut reused = owner;
        reused.process_started += 1;
        assert!(matches!(liveness(&reused), ownership::OwnerLiveness::Dead));
        reused.process_id = u32::MAX;
        assert!(matches!(liveness(&reused), ownership::OwnerLiveness::Dead));
    }
    #[test]
    fn an_unresolved_or_unreadable_hardware_transaction_blocks_oem_return() {
        assert!(hardware_recovery_gate(Ok(false)).is_ok());
        assert!(hardware_recovery_gate(Ok(true)).is_err());
        assert!(hardware_recovery_gate(Err("broken journal".into())).is_err());
    }
    #[test]
    fn a_temporarily_stopped_oem_is_not_a_qualified_startup_handover() {
        assert!(startup_safe_for_status(Ok(None)));
        for state in [State::Stopped, State::Running, State::Transitioning] {
            assert!(!startup_safe_for_status(Ok(Some(state))));
        }
        assert!(!startup_safe_for_status(Err("unavailable".into())));
    }
}
