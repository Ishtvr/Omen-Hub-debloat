//! Read-only CPU helper. A same-user, local, PID-bound pipe carries fixed
//! prepare/sample/pause/stop requests; no profiles, register IDs or driver blobs
//! in IPC. Preparation has its own budget; samples never run inventory queries.
mod session;
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

// At most 64 clock rows plus eight bounded warning strings (including JSON
// escaping). This remains a local, PID/nonce/sequence-bound measurement frame.
const MAX_FRAME: usize = 32_768;
const MAX_LAUNCH_FRAME: usize = 4096;
pub(crate) const APPROVAL_FAILED: &str = "CPU_SENSOR_APPROVAL_FAILED";
const SAMPLE: u8 = 1;
const PAUSE: u8 = 2;
const STOP: u8 = 3;
const PREPARE: u8 = 4;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
// Only preparation may run the isolated inventory child (bounded separately at
// five seconds). Never nest that full allowance inside a five-second sample.
const PREPARE_TIMEOUT: Duration = crate::read_process::READ_TIMEOUT.saturating_add(REQUEST_TIMEOUT);
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LaunchFrame {
    schema: u32,
    nonce: String,
    result: Result<u32, String>,
}
fn approved_pid(bytes: &[u8], nonce: &str) -> Result<u32, String> {
    if bytes.len() > MAX_LAUNCH_FRAME {
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
    prepared: bool,
    pending: Option<(u64, u8)>,
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
            .take((MAX_LAUNCH_FRAME + 1) as u64)
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
        let mut bridge = Self {
            pipe,
            _child: child,
            nonce,
            sequence: 0,
            prepared: false,
            pending: None,
        };
        bridge.prepare()?;
        Ok(bridge)
    }
    fn request(&mut self, command: u8) -> Result<Option<CpuHardwareSample>, String> {
        if self.pending.is_some() {
            return Err("CPU request still outstanding; pause must drain it first".into());
        }
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
        self.pending = Some((self.sequence, command));
        self.receive(self.sequence, command)
    }
    fn receive(&mut self, sequence: u64, command: u8) -> Result<Option<CpuHardwareSample>, String> {
        let deadline = Instant::now()
            + if command == PREPARE {
                PREPARE_TIMEOUT
            } else {
                REQUEST_TIMEOUT
            };
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
                self.pending = None;
                return response(&frame[..bytes as usize], &self.nonce, sequence, command);
            }
            if unsafe { WaitForSingleObject(self._child.0, 0) } != WAIT_TIMEOUT {
                return Err("CPU reader disconnected before replying".into());
            }
            if Instant::now() >= deadline {
                return Err(if command == PREPARE {
                    "CPU reader preparation timed out; no sample was accepted"
                } else {
                    "CPU reader timed out; old sensor values were discarded"
                }
                .into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn sample(&mut self) -> Result<CpuHardwareSample, String> {
        if !self.prepared {
            self.prepare()?;
        }
        self.request(SAMPLE)?.ok_or("No CPU sample".into())
    }
    pub fn pause(&mut self) -> Result<(), String> {
        self.prepared = false;
        if let Some((sequence, command)) = self.pending {
            // Windows may have frozen both peers during an in-flight sample.
            // Accept only its known nonce/sequence/operation, discard its data,
            // then send PAUSE. Never relabel that late sample as an ack.
            self.receive(sequence, command)?;
        }
        self.request(PAUSE).map(|_| ())
    }
    fn prepare(&mut self) -> Result<(), String> {
        self.prepared = false;
        self.request(PREPARE)?;
        self.prepared = true;
        Ok(())
    }
    pub fn process_id(&self) -> u32 {
        unsafe { GetProcessId(self._child.0) }
    }
}

fn response(
    bytes: &[u8],
    nonce: &str,
    sequence: u64,
    command: u8,
) -> Result<Option<CpuHardwareSample>, String> {
    if bytes.len() > MAX_FRAME {
        return Err("CPU reader response exceeds its bounded frame".into());
    }
    let frame: Frame = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if frame.schema != 1 || frame.nonce != nonce || frame.sequence != sequence {
        return Err("CPU reader returned a stale or unrelated response".into());
    }
    let sample = frame.result?;
    if let Some(sample) = &sample {
        sample.validate()?;
    }
    if !matches!(command, PREPARE | SAMPLE | PAUSE) || (command == SAMPLE) != sample.is_some() {
        return Err("CPU response does not match its fixed operation".into());
    }
    Ok(sample)
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
                "{APPROVAL_FAILED}: {}",
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
    let mut session = session::ReaderSession::new();
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
            PREPARE => session
                .prepare(crate::inventory::try_read, CpuHardwareReader::new)
                .map(|()| None),
            SAMPLE => session.sample(CpuHardwareReader::sample).map(Some),
            PAUSE => {
                session.pause();
                Ok(None)
            }
            STOP => break,
            _ => return Err("Unknown CPU sensor request; no hardware operation dispatched".into()),
        };
        if result.is_err() {
            session.pause();
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
    fn pause_drains_only_the_exact_outstanding_sample_before_its_own_ack() {
        for stale in [false, true] {
            let nonce = token();
            let handle = unsafe {
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
                    ptr::null(),
                )
            };
            assert_ne!(handle, INVALID_HANDLE_VALUE);
            let pipe = Handle(handle);
            let peer_nonce = nonce.clone();
            // A local test peer exercises the real message pipe, never UAC,
            // inventory, a device, a driver, or a hardware sampling operation.
            let peer = thread::spawn(move || {
                let handle = unsafe {
                    CreateFileW(
                        name(&peer_nonce).as_ptr(),
                        GENERIC_READ | GENERIC_WRITE,
                        0,
                        ptr::null(),
                        OPEN_EXISTING,
                        0,
                        ptr::null_mut(),
                    )
                };
                assert_ne!(handle, INVALID_HANDLE_VALUE);
                let peer = Handle(handle);
                let mut command = 0u8;
                let mut count = 0;
                assert_ne!(
                    unsafe {
                        ReadFile(
                            peer.0,
                            (&mut command as *mut u8).cast(),
                            1,
                            &mut count,
                            ptr::null_mut(),
                        )
                    },
                    0
                );
                assert_eq!(command, SAMPLE);
                thread::sleep(Duration::from_millis(20));
                let frame = Frame {
                    schema: 1,
                    nonce: if stale {
                        "unrelated".into()
                    } else {
                        peer_nonce.clone()
                    },
                    sequence: 1,
                    result: Ok(Some(CpuHardwareSample {
                        schema: 1,
                        source: omen_core::CpuHardwareSource::PawnioAmdRapl,
                        temperature_kind: omen_core::CpuTemperatureKind::AmdTctl,
                        package_power_w: None,
                        temperature_c: Some(50.0),
                        power_interval_ms: None,
                        effective_clocks: None,
                        warnings: vec![],
                    })),
                };
                let reply = serde_json::to_vec(&frame).unwrap();
                assert_ne!(
                    unsafe {
                        WriteFile(
                            peer.0,
                            reply.as_ptr().cast(),
                            reply.len() as u32,
                            &mut count,
                            ptr::null_mut(),
                        )
                    },
                    0
                );
                if stale {
                    return;
                }
                assert_ne!(
                    unsafe {
                        ReadFile(
                            peer.0,
                            (&mut command as *mut u8).cast(),
                            1,
                            &mut count,
                            ptr::null_mut(),
                        )
                    },
                    0
                );
                assert_eq!(command, PAUSE);
                let ack = serde_json::to_vec(&Frame {
                    schema: 1,
                    nonce: peer_nonce,
                    sequence: 2,
                    result: Ok(None),
                })
                .unwrap();
                assert_ne!(
                    unsafe {
                        WriteFile(
                            peer.0,
                            ack.as_ptr().cast(),
                            ack.len() as u32,
                            &mut count,
                            ptr::null_mut(),
                        )
                    },
                    0
                );
                // Keep the writer alive until the parent consumes its ack.
                unsafe {
                    FlushFileBuffers(peer.0);
                }
            });
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let connected = unsafe { ConnectNamedPipe(pipe.0, ptr::null_mut()) };
                let error = unsafe { GetLastError() };
                if connected != 0 || error == ERROR_PIPE_CONNECTED {
                    break;
                }
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(5));
            }
            let process = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE,
                    0,
                    std::process::id(),
                )
            };
            assert!(!process.is_null());
            let mut bridge = CpuSensorBridge {
                pipe,
                _child: Handle(process),
                nonce,
                sequence: 1,
                prepared: true,
                pending: Some((1, SAMPLE)),
            };
            let mut written = 0;
            let command = SAMPLE;
            assert_ne!(
                unsafe {
                    WriteFile(
                        bridge.pipe.0,
                        (&command as *const u8).cast(),
                        1,
                        &mut written,
                        ptr::null_mut(),
                    )
                },
                0
            );
            let result = bridge.pause();
            if stale {
                assert!(result.unwrap_err().contains("stale or unrelated"));
            } else {
                assert!(result.is_ok());
                assert_eq!(bridge.sequence, 2);
                assert!(bridge.pending.is_none());
            }
            assert!(!bridge.prepared);
            drop(bridge);
            peer.join().unwrap();
        }
    }

    #[test]
    fn preparation_acknowledgement_cannot_become_a_sample_or_cross_a_sequence() {
        let ack = serde_json::to_vec(&Frame {
            schema: 1,
            nonce: "current".into(),
            sequence: 4,
            result: Ok(None),
        })
        .unwrap();
        assert!(response(&ack, "current", 4, PREPARE).unwrap().is_none());
        assert!(response(&ack, "current", 4, PAUSE).unwrap().is_none());
        assert!(response(&ack, "current", 4, SAMPLE).is_err());
        assert!(response(&ack, "old", 4, PREPARE).is_err());
        assert!(response(&ack, "current", 5, PREPARE).is_err());
        assert!(response(&ack, "current", 4, STOP).is_err());

        let sample = serde_json::to_vec(&Frame {
            schema: 1,
            nonce: "current".into(),
            sequence: 5,
            result: Ok(Some(CpuHardwareSample {
                schema: 1,
                source: omen_core::CpuHardwareSource::PawnioAmdRapl,
                temperature_kind: omen_core::CpuTemperatureKind::AmdTctl,
                package_power_w: None,
                temperature_c: Some(50.0),
                power_interval_ms: None,
                effective_clocks: None,
                warnings: vec![],
            })),
        })
        .unwrap();
        assert!(response(&sample, "current", 5, SAMPLE).unwrap().is_some());
        assert!(response(&sample, "current", 5, PREPARE).is_err());
        assert!(response(&sample, "current", 5, PAUSE).is_err());
        assert!(response(&sample, "current", 4, SAMPLE).is_err());
    }

    #[test]
    fn preparation_budget_includes_both_bounded_phases_and_preserves_errors() {
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(5));
        assert!(PREPARE_TIMEOUT >= crate::read_process::READ_TIMEOUT + REQUEST_TIMEOUT);
        let error = "Isolated read exceeded the 5-second deadline";
        let bytes = serde_json::to_vec(&Frame {
            schema: 1,
            nonce: "current".into(),
            sequence: 1,
            result: Err(error.into()),
        })
        .unwrap();
        assert_eq!(response(&bytes, "current", 1, PREPARE).unwrap_err(), error);
        assert!(response(&vec![b' '; MAX_FRAME + 1], "current", 1, PREPARE).is_err());
    }

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
        assert!(approved_pid(&vec![b' '; MAX_LAUNCH_FRAME + 1], "session").is_err());
        let cancelled = format!("{APPROVAL_FAILED}: OS error 1223");
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

    #[test]
    fn bounded_frame_carries_all_clocks_and_worst_case_escaped_warnings() {
        use omen_core::{CpuHardwareSource, CpuTemperatureKind, cpu::*};
        let sample = CpuHardwareSample {
            schema: 1,
            source: CpuHardwareSource::PawnioAmdRapl,
            temperature_kind: CpuTemperatureKind::AmdTctl,
            package_power_w: Some(0.1234567890123456),
            temperature_c: Some(83.375),
            power_interval_ms: Some(10_000),
            effective_clocks: Some(CpuEffectiveClocks {
                source: CpuClockSource::AmdAperfReadOnly,
                logical_processor_count: 64,
                processors: (0..64)
                    .map(|logical_processor| CpuLogicalClock {
                        logical_processor,
                        effective_mhz: 0.1234567890123456,
                        interval_ms: 10_000,
                    })
                    .collect(),
            }),
            warnings: vec!["\u{0001}".repeat(512); 8],
        };
        sample.validate().unwrap();
        let bytes = serde_json::to_vec(&Frame {
            schema: 1,
            nonce: "a".repeat(64),
            sequence: u64::MAX,
            result: Ok(Some(sample)),
        })
        .unwrap();
        assert!(
            bytes.len() < MAX_FRAME,
            "{} bytes exceeded bounded frame",
            bytes.len()
        );
        let decoded: Frame = serde_json::from_slice(&bytes).unwrap();
        decoded.result.unwrap().unwrap().validate().unwrap();
    }

    #[test]
    fn legacy_hardware_samples_remain_readable_without_effective_clocks() {
        let sample: CpuHardwareSample = serde_json::from_str(
            r#"{"schema":1,"source":"pawnio_amd_rapl","temperature_kind":"amd_tctl",
                "package_power_w":null,"temperature_c":50.0,"power_interval_ms":null,"warnings":[]}"#,
        ).unwrap();
        assert!(sample.effective_clocks.is_none());
        assert!(sample.validate().is_ok());
    }
}
