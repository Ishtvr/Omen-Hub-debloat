//! Private same-image process protocol. No service/driver operations here.
use omen_core::{
    guardian::{Decision, Finish, LinkState, ParentState, Trigger, Watch},
    ownership::{Identity, Owner, Phase, Record},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle, process::CommandExt},
    process::{Child, Command, Stdio},
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED, FILETIME, HANDLE,
        INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Storage::FileSystem::{FILE_SHARE_READ, FILE_TYPE_PIPE, GetFileType, ReadFile},
    System::{Diagnostics::ToolHelp::*, Pipes::PeekNamedPipe, Threading::*},
};

pub const MAX_FRAME: usize = 2048;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: u32,
    pub owner: Owner,
    pub identity: Identity,
}
impl Request {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 {
            return Err("Guardian protocol schema differs".into());
        }
        self.owner.validate()?;
        self.identity.validate()
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ready {
    pub schema: u32,
    pub owner: Owner,
    pub identity: Identity,
    pub process_id: u32,
    pub process_started: u64,
}
pub fn write_frame(writer: &mut impl Write, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.is_empty() || bytes.len() > MAX_FRAME {
        return Err("Guardian frame exceeds its bound".into());
    }
    writer
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .and_then(|_| writer.write_all(&bytes))
        .and_then(|_| writer.flush())
        .map_err(|e| e.to_string())
}
pub fn read_frame<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T, String> {
    let mut size = [0u8; 4];
    reader.read_exact(&mut size).map_err(|e| e.to_string())?;
    let size = u32::from_le_bytes(size) as usize;
    if size == 0 || size > MAX_FRAME {
        return Err("Guardian frame size is invalid".into());
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
/// # Safety
/// `handle` must be a live process handle with query rights, borrowed for the
/// complete call. A valid GetCurrentProcess pseudo-handle is also accepted.
pub unsafe fn creation(handle: HANDLE) -> Result<u64, String> {
    let mut c: FILETIME = unsafe { std::mem::zeroed() };
    let mut x = c;
    let mut k = c;
    let mut u = c;
    if unsafe { GetProcessTimes(handle, &mut c, &mut x, &mut k, &mut u) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok((u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime))
}
fn available(handle: HANDLE) -> Result<u32, String> {
    let mut bytes = 0;
    if unsafe {
        PeekNamedPipe(
            handle,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            &mut bytes,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(bytes)
}
fn await_bytes(child: &mut Child, needed: u32, deadline: Instant) -> Result<(), String> {
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Err("Guardian exited before readiness".into());
        }
        let output = child.stdout.as_ref().ok_or("Guardian output pipe absent")?;
        if available(output.as_raw_handle().cast())? >= needed {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("Guardian readiness deadline exceeded; no takeover is allowed".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
/// The command arguments are chosen by the caller's code, never by a recovery
/// record. Production always launches the one fixed same-exe guardian command.
pub struct GuardianProcess {
    child: Child,
    _image: File,
}
impl GuardianProcess {
    pub fn spawn_self(request: &Request, arguments: &[std::ffi::OsString]) -> Result<Self, String> {
        request.validate()?;
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let image = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&executable)
            .map_err(|e| e.to_string())?;
        // A watcher inside a kill-on-close parent job would disappear with its
        // owner. Refuse takeover if Windows will not permit independent launch.
        let child = Command::new(executable)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | CREATE_BREAKAWAY_FROM_JOB)
            .spawn()
            .map_err(|e| format!("Independent guardian launch: {e}"))?;
        let mut result = Self {
            child,
            _image: image,
        };
        // Successful CREATE_BREAKAWAY_FROM_JOB leaves the owner's job. An
        // independent Windows compatibility job is not inherited ownership.
        write_frame(
            result
                .child
                .stdin
                .as_mut()
                .ok_or("Guardian input pipe absent")?,
            request,
        )?;
        let deadline = Instant::now() + Duration::from_secs(5);
        await_bytes(&mut result.child, 4, deadline)?;
        let mut size = [0u8; 4];
        result
            .child
            .stdout
            .as_mut()
            .unwrap()
            .read_exact(&mut size)
            .map_err(|e| e.to_string())?;
        let size = u32::from_le_bytes(size) as usize;
        if size == 0 || size > MAX_FRAME {
            return Err("Guardian ready frame is oversized".into());
        }
        await_bytes(&mut result.child, size as u32, deadline)?;
        let mut bytes = vec![0; size];
        result
            .child
            .stdout
            .as_mut()
            .unwrap()
            .read_exact(&mut bytes)
            .map_err(|e| e.to_string())?;
        let ready: Ready = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let started = unsafe { creation(result.child.as_raw_handle().cast()) }?;
        validate_ready(request, &ready, result.child.id(), started)?;
        result.ensure_running()?;
        Ok(result)
    }
    pub fn ensure_running(&mut self) -> Result<(), String> {
        if self.child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Err("Independent guardian is no longer running".into());
        }
        Ok(())
    }
    pub fn id(&self) -> u32 {
        self.child.id()
    }
    /// Normal scope exit closes only our private lease link, never kills the
    /// guardian. It independently checks the protected record before recovery.
    pub fn disconnect(&mut self) {
        self.child.stdin.take();
    }
    pub fn wait_for_exit(&mut self, budget: Duration) -> Result<bool, String> {
        let deadline = Instant::now() + budget;
        loop {
            if self.child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn validate_ready(request: &Request, ready: &Ready, pid: u32, started: u64) -> Result<(), String> {
    if ready.schema != 1
        || ready.owner != request.owner
        || ready.identity != request.identity
        || ready.process_id != pid
        || ready.process_started != started
    {
        return Err("Guardian readiness belongs to a different process or session".into());
    }
    Ok(())
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub struct ParentProcess {
    handle: Handle,
}
fn actual_parent() -> Result<u32, String> {
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let snapshot = Handle(handle);
    let mut row: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    row.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut row) } != 0;
    while ok {
        if row.th32ProcessID == std::process::id() {
            return Ok(row.th32ParentProcessID);
        }
        ok = unsafe { Process32NextW(snapshot.0, &mut row) } != 0;
    }
    Err("Guardian's creator process is unavailable".into())
}
fn image_path(handle: HANDLE) -> Result<String, String> {
    let mut path = [0u16; 32768];
    let mut size = path.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle, 0, path.as_mut_ptr(), &mut size) } == 0
        || size as usize >= path.len()
    {
        return Err("Guardian owner image is unavailable".into());
    }
    String::from_utf16(&path[..size as usize]).map_err(|e| e.to_string())
}
impl ParentProcess {
    pub fn authenticate(request: &Request) -> Result<Self, String> {
        request.validate()?;
        if request.owner.user != super::current_user_sid()?
            || request.owner.process_id != actual_parent()?
        {
            return Err("Guardian owner is not its actual same-user creator".into());
        }
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                request.owner.process_id,
            )
        };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let handle = Handle(handle);
        let current = std::env::current_exe().map_err(|e| e.to_string())?;
        if unsafe { creation(handle.0) }? != request.owner.process_started
            || !image_path(handle.0)?.eq_ignore_ascii_case(&current.to_string_lossy())
        {
            return Err("Guardian owner creation/image identity differs".into());
        }
        Ok(Self { handle })
    }
    pub fn state(&self) -> ParentState {
        match unsafe { WaitForSingleObject(self.handle.0, 0) } {
            WAIT_OBJECT_0 => ParentState::Exited,
            WAIT_TIMEOUT => ParentState::Alive,
            _ => ParentState::Unknown,
        }
    }
}
pub fn input_is_pipe(input: &impl AsRawHandle) -> bool {
    (unsafe { GetFileType(input.as_raw_handle().cast()) }) == FILE_TYPE_PIPE
}
/// No buffered stdin reader: additional bytes must remain visible to the lease
/// protocol instead of being prefetched behind PeekNamedPipe's back.
pub fn receive_request(input: &impl AsRawHandle) -> Result<Request, String> {
    if !input_is_pipe(input) {
        return Err("Guardian requires its private input pipe".into());
    }
    struct Pipe(HANDLE);
    impl Read for Pipe {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let mut count = 0;
            if unsafe {
                ReadFile(
                    self.0,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    &mut count,
                    ptr::null_mut(),
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(count as usize)
        }
    }
    let request: Request = read_frame(&mut Pipe(input.as_raw_handle().cast()))?;
    request.validate()?;
    Ok(request)
}
pub fn link_state(input: &impl AsRawHandle) -> LinkState {
    let mut bytes = 0;
    if unsafe {
        PeekNamedPipe(
            input.as_raw_handle().cast(),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            &mut bytes,
            ptr::null_mut(),
        )
    } != 0
    {
        return if bytes == 0 {
            LinkState::Open
        } else {
            LinkState::Invalid
        };
    }
    let code = std::io::Error::last_os_error().raw_os_error();
    if matches!(code,Some(x)if x==ERROR_BROKEN_PIPE as i32||x==ERROR_PIPE_NOT_CONNECTED as i32) {
        LinkState::Closed
    } else {
        LinkState::Invalid
    }
}
pub fn ready(request: &Request) -> Result<Ready, String> {
    Ok(Ready {
        schema: 1,
        owner: request.owner.clone(),
        identity: request.identity.clone(),
        process_id: std::process::id(),
        process_started: unsafe { creation(GetCurrentProcess()) }?,
    })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    Finished { reason: Finish },
    Recovered { trigger: Trigger, phase: Phase },
}
pub trait Target {
    fn record(&mut self) -> Result<Option<Record>, String>;
    fn recover(&mut self, trigger: Trigger, parent: ParentState) -> Result<Phase, String>;
}
/// The callback implementations define the fixed subject. This loop can only
/// request return of the authenticated session; it has no STOP operation.
pub fn monitor(
    request: &Request,
    parent: &ParentProcess,
    input: &impl AsRawHandle,
    target: &mut impl Target,
) -> Result<Outcome, String> {
    let mut watch = Watch::new(request.owner.clone(), request.identity.clone())?;
    let began = Instant::now();
    loop {
        let record = target.record()?;
        let state = parent.state();
        match watch.observe(began.elapsed(), state, link_state(input), record.as_ref())? {
            Decision::Wait => std::thread::sleep(Duration::from_millis(100)),
            Decision::Finish(reason) => return Ok(Outcome::Finished { reason }),
            Decision::Recover(trigger) => {
                return target
                    .recover(trigger, state)
                    .map(|phase| Outcome::Recovered { trigger, phase });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn request() -> Request {
        Request {
            schema: 1,
            owner: Owner {
                user: "user".into(),
                process_id: 10,
                process_started: 100,
                boot: 1,
                session: "a".repeat(64),
            },
            identity: Identity {
                subject: omen_core::ownership::Subject::HpOmenCap,
                image_sha256: "b".repeat(64),
                configuration_sha256: "c".repeat(64),
            },
        }
    }
    #[test]
    fn bounded_frames_reject_empty_oversized_truncated_and_foreign_fields() {
        for size in [0, MAX_FRAME as u32 + 1] {
            assert!(read_frame::<Request>(&mut Cursor::new(size.to_le_bytes())).is_err());
        }
        assert!(read_frame::<Request>(&mut Cursor::new(vec![20, 0, 0, 0, b'{'])).is_err());
        let mut raw = serde_json::to_value(request()).unwrap();
        raw["command"] = "unexpected".into();
        let mut bytes = vec![];
        write_frame(&mut bytes, &raw).unwrap();
        assert!(read_frame::<Request>(&mut Cursor::new(bytes)).is_err());
    }
    #[test]
    fn readiness_binds_the_child_kernel_identity_and_full_session() {
        let request = request();
        let mut frame = Ready {
            schema: 1,
            owner: request.owner.clone(),
            identity: request.identity.clone(),
            process_id: 20,
            process_started: 200,
        };
        validate_ready(&request, &frame, 20, 200).unwrap();
        assert!(validate_ready(&request, &frame, 21, 200).is_err());
        assert!(validate_ready(&request, &frame, 20, 201).is_err());
        frame.owner.session = "d".repeat(64);
        assert!(validate_ready(&request, &frame, 20, 200).is_err());
        frame.owner = request.owner.clone();
        frame.identity.configuration_sha256 = "e".repeat(64);
        assert!(validate_ready(&request, &frame, 20, 200).is_err());
    }
}
