//! Read-only CPU helper. A same-user, local, PID-bound pipe carries fixed
//! sample/pause/stop requests; no profiles, register IDs or driver blobs in IPC.
use crate::native_ui::{token, wide};
use omen_core::CpuHardwareSample;
use omen_platform::CpuHardwareReader;
use serde::{Deserialize, Serialize};
use std::{
    io::Read as _,
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    ptr, thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::{
        Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW, SECURITY_ATTRIBUTES,
    },
    Storage::FileSystem::*,
    System::{Pipes::*, Threading::*},
    UI::{Shell::*, WindowsAndMessaging::SW_HIDE},
};

const MAX_FRAME: usize = 4096;
const SAMPLE: u8 = 1;
const PAUSE: u8 = 2;
const STOP: u8 = 3;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LaunchFrame {
    schema: u32,
    nonce: String,
    result: Result<u32, String>,
}
fn approved_pid(bytes: &[u8], nonce: &str) -> Result<u32, String> {
    if bytes.len() > MAX_FRAME {
        return Err("CPU launch response exceeds its bounded frame".into());
    }
    let frame: LaunchFrame = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if frame.schema != 1 || frame.nonce != nonce {
        return Err("CPU launch returned a stale or unrelated response".into());
    }
    let pid = frame.result?;
    if pid == 0 {
        return Err("CPU reader process ID is missing".into());
    }
    Ok(pid)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    schema: u32,
    nonce: String,
    sequence: u64,
    result: Result<Option<CpuHardwareSample>, String>,
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn validate_nonce(nonce: &str) -> Result<(), String> {
    if nonce.is_empty() || nonce.len() > 64 || !nonce.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err("Invalid CPU session token".into());
    }
    Ok(())
}
fn name(nonce: &str) -> Vec<u16> {
    wide(&format!("\\\\.\\pipe\\OmenHubPP.CpuRead.{nonce}"))
}

pub struct CpuSensorBridge {
    pipe: Handle,
    _child: Handle,
    nonce: String,
    sequence: u64,
}
impl CpuSensorBridge {
    pub fn start() -> Result<Self, String> {
        let nonce = token();
        validate_nonce(&nonce)?;
        let sid = omen_platform::current_user_sid()?;
        // No inherited default DACL: only this user and LocalSystem. The
        // expected child PID is checked separately after the UAC launch.
        let sddl = wide(&format!("D:P(A;;GA;;;SY)(A;;GA;;;{sid})"));
        let mut descriptor = ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let pipe = unsafe {
            CreateNamedPipeW(
                name(&nonce).as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE
                    | PIPE_READMODE_MESSAGE
                    | PIPE_NOWAIT
                    | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                MAX_FRAME as u32,
                MAX_FRAME as u32,
                100,
                &attributes,
            )
        };
        unsafe {
            LocalFree(descriptor);
        }
        if pipe == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let pipe = Handle(pipe);
        // ShellExecute loads COM/shell extensions. Keep that transient state
        // out of both persistent processes. This exact same-executable child
        // returns the approved PID through its private inherited stdout pipe.
        let mut launcher = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .args(["cpu-sensor-launch", &nonce, &std::process::id().to_string()])
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("CPU reader launch: {e}"))?;
        let mut response = Vec::new();
        launcher
            .stdout
            .take()
            .ok_or("CPU launch pipe missing")?
            .take((MAX_FRAME + 1) as u64)
            .read_to_end(&mut response)
            .map_err(|e| e.to_string())?;
        let status = launcher.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("CPU reader launch process failed".into());
        }
        let expected = approved_pid(&response, &nonce)?;
        let process =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, expected) };
        if process.is_null() {
            return Err(format!(
                "CPU reader process access: {}",
                std::io::Error::last_os_error()
            ));
        }
        let child = Handle(process);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let connected = unsafe { ConnectNamedPipe(pipe.0, ptr::null_mut()) };
            let error = unsafe { GetLastError() };
            if connected != 0 || error == ERROR_PIPE_CONNECTED {
                let mut actual = 0;
                if unsafe { GetNamedPipeClientProcessId(pipe.0, &mut actual) } != 0 {
                    if actual != expected {
                        return Err("CPU pipe peer differs from the approved process".into());
                    }
                    break;
                }
                if unsafe { GetLastError() } != ERROR_PIPE_NOT_CONNECTED {
                    return Err(std::io::Error::last_os_error().to_string());
                }
            }
            if (connected == 0 && error != ERROR_PIPE_LISTENING && error != ERROR_PIPE_CONNECTED)
                || Instant::now() >= deadline
                || unsafe { WaitForSingleObject(child.0, 0) } != WAIT_TIMEOUT
            {
                return Err("CPU reader failed to connect within its deadline".into());
            }
            thread::sleep(Duration::from_millis(20));
        }
        Ok(Self {
            pipe,
            _child: child,
            nonce,
            sequence: 0,
        })
    }
    fn request(&mut self, command: u8) -> Result<Option<CpuHardwareSample>, String> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("CPU sequence exhausted")?;
        let mut written = 0;
        if unsafe {
            WriteFile(
                self.pipe.0,
                (&command as *const u8).cast(),
                1,
                &mut written,
                ptr::null_mut(),
            )
        } == 0
            || written != 1
        {
            return Err("CPU reader request pipe closed".into());
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut available = 0;
            let mut remaining = 0;
            if unsafe {
                PeekNamedPipe(
                    self.pipe.0,
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    &mut available,
                    &mut remaining,
                )
            } == 0
            {
                return Err("CPU reader disconnected".into());
            }
            if remaining as usize > MAX_FRAME || available as usize > MAX_FRAME {
                return Err("CPU reader response exceeds its bounded frame".into());
            }
            if available > 0 {
                let mut frame = [0; MAX_FRAME];
                let mut bytes = 0;
                if unsafe {
                    ReadFile(
                        self.pipe.0,
                        frame.as_mut_ptr().cast(),
                        MAX_FRAME as u32,
                        &mut bytes,
                        ptr::null_mut(),
                    )
                } == 0
                {
                    return Err("CPU reader returned an incomplete message".into());
                }
                let frame: Frame =
                    serde_json::from_slice(&frame[..bytes as usize]).map_err(|e| e.to_string())?;
                if frame.schema != 1 || frame.nonce != self.nonce || frame.sequence != self.sequence
                {
                    return Err("CPU reader returned a stale or unrelated response".into());
                }
                let sample = frame.result?;
                if let Some(sample) = &sample {
                    sample.validate()?;
                }
                if (command == SAMPLE) != sample.is_some() {
                    return Err("CPU response does not match its fixed operation".into());
                }
                return Ok(sample);
            }
            if Instant::now() >= deadline {
                return Err("CPU reader timed out; old sensor values were discarded".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn sample(&mut self) -> Result<CpuHardwareSample, String> {
        self.request(SAMPLE)?.ok_or("No CPU sample".into())
    }
    pub fn pause(&mut self) -> Result<(), String> {
        self.request(PAUSE).map(|_| ())
    }
    pub fn process_id(&self) -> u32 {
        unsafe { GetProcessId(self._child.0) }
    }
}

pub fn launch(nonce: &str, parent: u32) -> Result<(), String> {
    validate_nonce(nonce)?;
    if parent == 0 {
        return Err("Invalid CPU parent process".into());
    }
    let result = (|| {
        let file = wide(
            &std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy(),
        );
        let params = wide(&format!("cpu-sensor-child {nonce} {parent}"));
        let verb = wide("runas");
        let mut launch: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        launch.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
        launch.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        launch.lpFile = file.as_ptr();
        launch.lpParameters = params.as_ptr();
        launch.lpVerb = verb.as_ptr();
        launch.nShow = SW_HIDE;
        if unsafe { ShellExecuteExW(&mut launch) } == 0 {
            return Err(format!(
                "CPU-Sensorfreigabe nicht erfolgt: {}",
                std::io::Error::last_os_error()
            ));
        }
        if launch.hProcess.is_null() {
            return Err("CPU reader process handle is missing".into());
        }
        let child = Handle(launch.hProcess);
        let pid = unsafe { GetProcessId(child.0) };
        if pid == 0 {
            return Err("CPU reader process ID is missing".into());
        }
        Ok(pid)
    })();
    println!(
        "{}",
        serde_json::to_string(&LaunchFrame {
            schema: 1,
            nonce: nonce.into(),
            result
        })
        .map_err(|e| e.to_string())?
    );
    Ok(())
}
impl Drop for CpuSensorBridge {
    fn drop(&mut self) {
        // Best effort stop, then close the peer. Never stop/unload the driver.
        let command = STOP;
        let mut bytes = 0;
        unsafe {
            WriteFile(
                self.pipe.0,
                (&command as *const u8).cast(),
                1,
                &mut bytes,
                ptr::null_mut(),
            );
            DisconnectNamedPipe(self.pipe.0);
        }
    }
}

pub fn child(nonce: &str, parent: u32) -> Result<(), String> {
    validate_nonce(nonce)?;
    if parent == 0 {
        return Err("Invalid CPU parent process".into());
    }
    let file = unsafe {
        CreateFileW(
            name(nonce).as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };
    if file == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let pipe = Handle(file);
    let mut server = 0;
    if unsafe { GetNamedPipeServerProcessId(pipe.0, &mut server) } == 0 || server != parent {
        return Err("CPU reader server PID differs from the specified parent".into());
    }
    let mut reader = None;
    let mut sequence = 0u64;
    loop {
        let mut command = 0u8;
        let mut read = 0;
        if unsafe {
            ReadFile(
                pipe.0,
                (&mut command as *mut u8).cast(),
                1,
                &mut read,
                ptr::null_mut(),
            )
        } == 0
            || read != 1
        {
            break;
        }
        sequence = sequence.checked_add(1).ok_or("CPU sequence exhausted")?;
        let result = match command {
            SAMPLE => (|| {
                if reader.is_none() {
                    // WMI lives only in our bounded, disposable inventory
                    // process, never in the long-lived sensor helper.
                    reader = Some(
                        CpuHardwareReader::new(&crate::inventory::read())?.with_clocks_and_ccds(),
                    );
                }
                reader
                    .as_mut()
                    .ok_or("CPU reader missing")?
                    .sample()
                    .map(Some)
            })(),
            PAUSE => {
                reader = None;
                Ok(None)
            }
            STOP => break,
            _ => return Err("Unknown CPU sensor request; no hardware operation dispatched".into()),
        };
        if result.is_err() {
            reader = None;
        }
        let frame = serde_json::to_vec(&Frame {
            schema: 1,
            nonce: nonce.into(),
            sequence,
            result,
        })
        .map_err(|e| e.to_string())?;
        if frame.len() > MAX_FRAME {
            return Err("CPU response exceeds its bound".into());
        }
        let mut written = 0;
        if unsafe {
            WriteFile(
                pipe.0,
                frame.as_ptr().cast(),
                frame.len() as u32,
                &mut written,
                ptr::null_mut(),
            )
        } == 0
            || written as usize != frame.len()
        {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_reply_binds_approval_to_session_and_preserves_cancelled_uac() {
        let frame = |nonce: &str, result| {
            serde_json::to_vec(&LaunchFrame {
                schema: 1,
                nonce: nonce.into(),
                result,
            })
            .unwrap()
        };
        assert_eq!(approved_pid(&frame("session", Ok(42)), "session"), Ok(42));
        assert!(approved_pid(&frame("old", Ok(42)), "session").is_err());
        assert!(approved_pid(&frame("session", Ok(0)), "session").is_err());
        assert!(approved_pid(&vec![b' '; MAX_FRAME + 1], "session").is_err());
        let cancelled = "CPU-Sensorfreigabe nicht erfolgt: OS error 1223".to_string();
        assert_eq!(
            approved_pid(&frame("session", Err(cancelled.clone())), "session"),
            Err(cancelled)
        );
        assert!(
            approved_pid(
                br#"{"schema":2,"nonce":"session","result":{"Ok":42}}"#,
                "session"
            )
            .is_err()
        );
    }
}
