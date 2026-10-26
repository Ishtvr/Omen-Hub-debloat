//! Fixed own cooling-task inspection/resumption. Never installs a task, accepts
//! a supplied command, or stops a process. Only its existing protected action.
use super::startup;
use omen_core::cooling_override::{Acknowledgement, HelperIdentity, Mode};
use serde::Deserialize;
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::windows::fs::OpenOptionsExt,
    path::PathBuf,
    ptr,
    time::{Duration, Instant},
};
use windows::{
    Win32::System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        TaskScheduler::{
            IExecAction, ITaskService, TASK_INSTANCES_IGNORE_NEW, TASK_LOGON_INTERACTIVE_TOKEN,
            TASK_RUNLEVEL_HIGHEST, TaskScheduler,
        },
        Variant::VARIANT,
    },
    core::{BSTR, Interface},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_NO_MORE_FILES, HANDLE, INVALID_HANDLE_VALUE, LocalFree, WAIT_TIMEOUT,
    },
    Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation},
    Storage::FileSystem::FILE_SHARE_READ,
    System::{
        Diagnostics::ToolHelp::*,
        RemoteDesktop::ProcessIdToSessionId,
        Threading::{
            GetCurrentProcessId, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW, WaitForSingleObject,
        },
    },
    UI::Shell::CommandLineToArgvW,
};

struct Apartment(bool);
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                CoUninitialize();
            }
        }
    }
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub(super) fn image_path() -> Result<PathBuf, String> {
    Ok(startup::installation_root()?.join("Omen Hub++ Fan.exe"))
}
pub(super) fn verify(expected_hash: &str, start: bool) -> Result<(), String> {
    let root = startup::installation_root()?;
    let sid = startup::current_user_sid()?;
    let mut bytes = vec![];
    File::open(root.join(format!("fan-service-{sid}.json")))
        .map_err(|e| e.to_string())?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Cooling installation receipt exceeds its bound".into());
    }
    let receipt: startup::StartupInstallation =
        serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes))
            .map_err(|e| e.to_string())?;
    let name = format!("OmenHubPP-Fan-{sid}");
    let path = image_path()?;
    if receipt.schema != 1
        || receipt.user_sid != sid
        || receipt.task_name != name
        || receipt.executable != path
        || !receipt.sha256.eq_ignore_ascii_case(expected_hash)
        || !startup::file_hash(&path)?.eq_ignore_ascii_case(expected_hash)
    {
        return Err("Cooling task/binary receipt differs from the captured helper".into());
    }
    let init = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if init.is_err() && init.0 as u32 != 0x80010106 {
        return Err(init.to_string());
    }
    let _apartment = Apartment(init.is_ok());
    let operation = || -> windows::core::Result<()> {
        unsafe {
            let service: ITaskService =
                CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
            let empty = VARIANT::default();
            service.Connect(&empty, &empty, &empty, &empty)?;
            let task = service
                .GetFolder(&BSTR::from("\\"))?
                .GetTask(&BSTR::from(name.as_str()))?;
            let reject = |message: &str| {
                windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), message)
            };
            if !task.Enabled()?.as_bool() {
                return Err(reject("Original cooling task is disabled"));
            }
            startup::verify_principal_xml(&task.Xml()?.to_string(), &sid)
                .map_err(|e| reject(&e))?;
            let definition = task.Definition()?;
            let settings = definition.Settings()?;
            let mut instances = Default::default();
            let mut stop_on_battery = Default::default();
            let mut disallow_battery = Default::default();
            let mut limit = BSTR::new();
            settings.MultipleInstances(&mut instances)?;
            settings.StopIfGoingOnBatteries(&mut stop_on_battery)?;
            settings.DisallowStartIfOnBatteries(&mut disallow_battery)?;
            settings.ExecutionTimeLimit(&mut limit)?;
            if instances != TASK_INSTANCES_IGNORE_NEW
                || stop_on_battery.as_bool()
                || disallow_battery.as_bool()
                || limit != "PT0S"
            {
                return Err(reject("Cooling task lifetime/battery settings differ"));
            }
            let principal = definition.Principal()?;
            let mut logon = Default::default();
            let mut level = Default::default();
            principal.LogonType(&mut logon)?;
            principal.RunLevel(&mut level)?;
            if logon != TASK_LOGON_INTERACTIVE_TOKEN || level != TASK_RUNLEVEL_HIGHEST {
                return Err(reject("Cooling task principal differs"));
            }
            let actions = definition.Actions()?;
            let mut count = 0;
            actions.Count(&mut count)?;
            if count != 1 {
                return Err(reject("Cooling task action count differs"));
            }
            let action: IExecAction = actions.get_Item(1)?.cast()?;
            let mut exe = BSTR::new();
            let mut args = BSTR::new();
            let mut directory = BSTR::new();
            action.Path(&mut exe)?;
            action.Arguments(&mut args)?;
            action.WorkingDirectory(&mut directory)?;
            if std::path::Path::new(&exe.to_string()) != path
                || args != "fan-service"
                || std::path::Path::new(&directory.to_string()) != root
            {
                return Err(reject("Cooling task action differs"));
            }
            if start {
                let _ = task.Run(&empty)?;
            }
            Ok(())
        }
    };
    operation().map_err(|e| e.to_string())
}
struct LiveHelper {
    process: Handle,
    identity: HelperIdentity,
    _image: File,
}
impl LiveHelper {
    fn ensure_alive(&self) -> Result<(), String> {
        if unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT {
            return Err("Cooling helper exited while reading its acknowledgement".into());
        }
        Ok(())
    }
}
fn running(expected_hash: &str) -> Result<Option<LiveHelper>, String> {
    let expected = image_path()?;
    let image = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&expected)
        .map_err(|e| e.to_string())?;
    verify(expected_hash, false)?;
    let sid = startup::current_user_sid()?;
    let mut own_session = 0;
    if unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut own_session) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let snap = Handle(snap);
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut ok = unsafe { Process32FirstW(snap.0, &mut entry) } != 0;
    let mut found = None;
    loop {
        if !ok {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
                return Err(error.to_string());
            }
            break;
        }
        let end = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .ok_or("Cooling process name is unterminated")?;
        if String::from_utf16_lossy(&entry.szExeFile[..end])
            .eq_ignore_ascii_case("Omen Hub++ Fan.exe")
        {
            let h = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    0,
                    entry.th32ProcessID,
                )
            };
            if h.is_null() {
                return Err("Cooling helper process identity is unavailable".into());
            }
            let h = Handle(h);
            let mut file = [0u16; 32768];
            let mut size = file.len() as u32;
            if unsafe { QueryFullProcessImageNameW(h.0, 0, file.as_mut_ptr(), &mut size) } == 0
                || size as usize >= file.len()
            {
                return Err("Cooling helper image is unavailable".into());
            }
            if String::from_utf16(&file[..size as usize])
                .map_err(|e| e.to_string())?
                .eq_ignore_ascii_case(&expected.to_string_lossy())
            {
                let mut session = 0;
                if unsafe { ProcessIdToSessionId(entry.th32ProcessID, &mut session) } == 0
                    || session != own_session
                    || unsafe { startup::process_user_sid(h.0) }? != sid
                {
                    return Err("Cooling helper belongs to another user/session".into());
                }
                if found.is_some() {
                    return Err("Multiple protected cooling helpers are active".into());
                }
                verify_invocation(h.0, entry.th32ProcessID, &expected)?;
                let identity = HelperIdentity {
                    binary_sha256: expected_hash.to_ascii_lowercase(),
                    process_id: entry.th32ProcessID,
                    process_started: unsafe { super::guardian::creation(h.0) }?,
                };
                found = Some((h, identity));
            }
        }
        ok = unsafe { Process32NextW(snap.0, &mut entry) } != 0;
    }
    found
        .map(|(process, identity)| {
            let live = LiveHelper {
                process,
                identity,
                _image: image,
            };
            live.ensure_alive()?;
            Ok(live)
        })
        .transpose()
}
pub(super) fn ensure_running(expected_hash: &str) -> Result<HelperIdentity, String> {
    verify(expected_hash, false)?;
    if let Some(helper) = running(expected_hash)? {
        return Ok(helper.identity);
    }
    verify(expected_hash, true)?;
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(helper) = running(expected_hash)? {
            return Ok(helper.identity);
        }
        if Instant::now() >= end {
            return Err("Original cooling helper did not resume".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
pub(super) fn acknowledgement(expected_hash: &str) -> Result<Acknowledgement, String> {
    let live = running(expected_hash)?.ok_or("Cooling helper is not running")?;
    let helper = &live.identity;
    let path = super::profile_directory()?.join("fan-service-status.json");
    let mut bytes = vec![];
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8192 {
        return Err("Cooling acknowledgement exceeds its bound".into());
    }
    let status: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if status["schema"] != 1
        || status["process_id"].as_u64() != Some(u64::from(helper.process_id))
        || status["process_started"].as_u64() != Some(helper.process_started)
    {
        return Err("Cooling status does not belong to the verified live helper".into());
    }
    let mode = match status["state"].as_str() {
        Some("max") => Mode::Max,
        // Emergency maximum still acknowledges an active curve controller.
        Some("curve" | "full_cooling") => Mode::Curve,
        // BIOS ownership has no qualified acknowledgement.
        _ => return Err("Cooling helper has not acknowledged an active mode".into()),
    };
    live.ensure_alive()?;
    Ok(Acknowledgement {
        helper: helper.clone(),
        mode,
        updated_unix_ms: status["updated_unix_ms"]
            .as_u64()
            .ok_or("Fresh cooling acknowledgement timestamp is absent")?,
        last_heartbeat_unix_ms: status["last_heartbeat_unix_ms"].as_u64(),
    })
}

fn verify_invocation(process: HANDLE, pid: u32, expected: &std::path::Path) -> Result<(), String> {
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let token = Handle(token);
    let mut elevation: TOKEN_ELEVATION = unsafe { std::mem::zeroed() };
    let mut count = 0;
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut count,
        )
    } == 0
        || elevation.TokenIsElevated == 0
    {
        return Err("Cooling helper is not running with its required elevated token".into());
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Invocation {
        command_line: Option<String>,
    }
    let connection = wmi::WMIConnection::new().map_err(|e| e.to_string())?;
    let rows: Vec<Invocation> = connection
        .raw_query(format!(
            "SELECT CommandLine FROM Win32_Process WHERE ProcessId = {pid}"
        ))
        .map_err(|e| e.to_string())?;
    if rows.len() != 1 {
        return Err("Cooling helper invocation is unavailable".into());
    }
    let command = rows[0]
        .command_line
        .as_deref()
        .ok_or("Cooling command line is unavailable")?;
    verify_command(command, expected)
}
fn verify_command(command: &str, expected: &std::path::Path) -> Result<(), String> {
    if command.len() > 32768 || command.contains('\0') {
        return Err("Cooling command line exceeds its bound".into());
    }
    let wide: Vec<u16> = command.encode_utf16().chain(Some(0)).collect();
    let mut count = 0;
    let argv = unsafe { CommandLineToArgvW(wide.as_ptr(), &mut count) };
    if argv.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let result = (|| {
        if count != 2 {
            return Err("Cooling process is not the fixed fan-service action".into());
        }
        let mut args = Vec::new();
        for index in 0..2 {
            let value = unsafe { *argv.add(index) };
            let mut length = 0;
            while length < 32768 && unsafe { *value.add(length) } != 0 {
                length += 1;
            }
            if length == 32768 {
                return Err("Cooling argument exceeds its bound".into());
            }
            args.push(
                String::from_utf16(unsafe { std::slice::from_raw_parts(value, length) })
                    .map_err(|e| e.to_string())?,
            );
        }
        if !args[0].eq_ignore_ascii_case(&expected.to_string_lossy()) || args[1] != "fan-service" {
            return Err("Cooling process is not the fixed fan-service action".into());
        }
        Ok(())
    })();
    unsafe {
        LocalFree(argv.cast());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_fixed_fan_service_invocation_can_acknowledge_cooling() {
        let exe = std::path::Path::new(r"C:\Program Files\OmenHubPlusPlus\Omen Hub++ Fan.exe");
        verify_command(&format!("\"{}\" fan-service", exe.display()), exe).unwrap();
        for command in [
            format!("\"{}\" --tray", exe.display()),
            format!("\"{}\" fan-service extra", exe.display()),
            "C:\\other.exe fan-service".into(),
            String::new(),
        ] {
            assert!(verify_command(&command, exe).is_err());
        }
    }
}
