//! Inspect the fixed logon task through public COM. Runtime never executes a shell.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::PathBuf, ptr};
use windows::{
    Win32::System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        TaskScheduler::{
            IExecAction, ILogonTrigger, ITaskService, TASK_LOGON_INTERACTIVE_TOKEN,
            TASK_RUNLEVEL_HIGHEST, TaskScheduler,
        },
        Variant::VARIANT,
    },
    core::{BSTR, Interface},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree},
    Security::{
        Authentication::Identity::{GetUserNameExW, NameSamCompatible},
        Authorization::ConvertSidToStringSidW,
        GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

fn sam_name() -> Result<String, String> {
    let mut buffer = [0_u16; 512];
    let mut count = buffer.len() as u32;
    // SAFETY: Fixed documented name format and bounded writable UTF-16 storage.
    if !unsafe { GetUserNameExW(NameSamCompatible, buffer.as_mut_ptr(), &mut count) }
        || count as usize >= buffer.len()
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    String::from_utf16(&buffer[..count as usize]).map_err(|e| e.to_string())
}

/// COM normalizes UserId to a name; native task XML retains the principal SID.
/// Comments, extra principals and SID text elsewhere must not satisfy this check.
fn verify_principal_xml(xml: &str, sid: &str) -> Result<(), String> {
    use quick_xml::{Reader, events::Event};
    if xml.len() > 65536 {
        return Err("Task XML exceeds its inspection bound".into());
    }
    let mut reader = Reader::from_str(xml);
    let mut path: Vec<Vec<u8>> = vec![];
    let mut principals = 0;
    let mut identities = vec![];
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(tag) => {
                path.push(tag.local_name().as_ref().to_vec());
                if path.len() > 32 {
                    return Err("Task XML exceeds its nesting bound".into());
                }
                if path.iter().map(Vec::as_slice).eq([
                    b"Task".as_slice(),
                    b"Principals",
                    b"Principal",
                ]) {
                    principals += 1;
                }
            }
            Event::End(_) => {
                path.pop();
            }
            Event::Text(value)
                if path.iter().map(Vec::as_slice).eq([
                    b"Task".as_slice(),
                    b"Principals",
                    b"Principal",
                    b"UserId",
                ]) =>
            {
                identities.push(value.decode().map_err(|e| e.to_string())?.into_owned())
            }
            Event::DocType(_) => return Err("Task XML must not contain a DTD".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    if principals != 1 || identities != [sid] || !path.is_empty() {
        return Err("Scheduled task principal SID differs from the current user".into());
    }
    Ok(())
}

fn task_error(message: impl AsRef<str>) -> windows::core::Error {
    windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), message)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartupInstallation {
    pub schema: u32,
    pub executable: PathBuf,
    pub sha256: String,
    pub task_name: String,
    pub user_sid: String,
    pub installed_at: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartupAuthorization {
    schema: u32,
    user_sid: String,
    profile_id: String,
    profile_digest: String,
    binding: omen_core::profiles::DeviceBinding,
    binary_sha256: String,
}

fn authorization_path(installation: &StartupInstallation) -> Result<PathBuf, String> {
    Ok(installation
        .executable
        .parent()
        .ok_or("Installation directory missing")?
        .join(format!("startup-authorized-{}.json", current_user_sid()?)))
}

/// Called only after applied-journal eligibility and live target readback. The
/// installer protects this directory: an unelevated process cannot forge a new
/// profile approval by editing its user-writable selection or journal files.
pub fn authorize_startup_profile(
    profile: &omen_core::profiles::PerformanceProfile,
) -> Result<(), String> {
    let installation = verify_startup_installation()?;
    let record = StartupAuthorization {
        schema: 1,
        user_sid: current_user_sid()?,
        profile_id: profile.id.clone(),
        profile_digest: crate::profile_store::profile_digest(profile)?,
        binding: profile.binding.clone(),
        binary_sha256: installation.sha256.clone(),
    };
    crate::profile_store::atomic_json(&authorization_path(&installation)?, &record)
        .map_err(|e| format!("Administrator approval for startup is required: {e}"))
}

pub fn verify_startup_profile(
    profile: &omen_core::profiles::PerformanceProfile,
) -> Result<(), String> {
    let installation = verify_startup_installation()?;
    let mut data = vec![];
    File::open(authorization_path(&installation)?)
        .map_err(|e| format!("No protected startup approval: {e}"))?
        .take(65537)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > 65536 {
        return Err("Startup approval file exceeds its bound".into());
    }
    let record: StartupAuthorization = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    if record.schema != 1
        || record.user_sid != current_user_sid()?
        || record.profile_id != profile.id
        || record.profile_digest != crate::profile_store::profile_digest(profile)?
        || record.binding != profile.binding
        || !record
            .binary_sha256
            .eq_ignore_ascii_case(&installation.sha256)
    {
        return Err("Profile/build differs from the protected startup approval".into());
    }
    Ok(())
}

pub fn current_user_sid() -> Result<String, String> {
    let mut token = ptr::null_mut();
    // SAFETY: Token output is initialized; query-only access to this process.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let result = (|| {
        let mut bytes = 0;
        // SAFETY: Size query has no output buffer and uses the fixed TokenUser class.
        unsafe {
            GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut bytes);
        }
        if bytes < size_of::<TOKEN_USER>() as u32 || bytes > 65536 {
            return Err("Invalid token user size".into());
        }
        // usize storage provides TOKEN_USER alignment as well as its trailing SID.
        let mut buffer = vec![0_usize; (bytes as usize).div_ceil(size_of::<usize>())];
        // SAFETY: Aligned storage has at least the size returned by the token query.
        if unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                bytes,
                &mut bytes,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        // SAFETY: A successful TokenUser query initialized this exact structure.
        let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        let mut sid = ptr::null_mut();
        // SAFETY: SID remains valid in buffer; conversion returns LocalAlloc storage.
        if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut length = 0;
        // SAFETY: Windows returns a terminated SID string, bounded here at 256 units.
        unsafe {
            while length < 256 && *sid.add(length) != 0 {
                length += 1;
            }
            let result = if length < 256 {
                String::from_utf16(std::slice::from_raw_parts(sid, length))
                    .map_err(|e| e.to_string())
            } else {
                Err("SID string exceeds bound".into())
            };
            LocalFree(sid.cast());
            result
        }
    })();
    // SAFETY: Every successful OpenProcessToken is paired, including error paths.
    unsafe {
        CloseHandle(token);
    }
    result
}

fn file_hash(path: &std::path::Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
struct Apartment(bool);
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: Paired on the same thread with successful CoInitializeEx.
            unsafe {
                CoUninitialize();
            }
        }
    }
}

/// Whether any build of the logon fan service is installed (its receipt
/// exists), even one that predates the current executable.
pub fn fan_service_present() -> bool {
    std::env::var_os("ProgramFiles").is_some_and(|files| {
        current_user_sid().is_ok_and(|sid| {
            PathBuf::from(files)
                .join("OmenHubPlusPlus")
                .join(format!("fan-service-{sid}.json"))
                .is_file()
        })
    })
}
/// Whether the logon fan service is installed from this exact build (receipt
/// written by scripts/install-fan-service.ps1 in the protected directory).
pub fn fan_service_installed() -> bool {
    (|| -> Result<bool, String> {
        let root =
            PathBuf::from(std::env::var_os("ProgramFiles").ok_or("Program Files unavailable")?)
                .join("OmenHubPlusPlus");
        let sid = current_user_sid()?;
        let mut bytes = vec![];
        File::open(root.join(format!("fan-service-{sid}.json")))
            .map_err(|e| e.to_string())?
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let receipt: serde_json::Value =
            serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes))
                .map_err(|e| e.to_string())?;
        let current = file_hash(&std::env::current_exe().map_err(|e| e.to_string())?)?;
        Ok(receipt["schema"] == 1
            && receipt["sha256"]
                .as_str()
                .is_some_and(|h| h.eq_ignore_ascii_case(&current)))
    })()
    .unwrap_or(false)
}

/// Requires the protected installed binary, the same current build and the exact
/// per-user interactive logon action. This is a read; installation is explicit.
pub fn verify_startup_installation() -> Result<StartupInstallation, String> {
    let root = PathBuf::from(std::env::var_os("ProgramFiles").ok_or("Program Files unavailable")?)
        .join("OmenHubPlusPlus");
    let file = File::open(root.join("installation.json")).map_err(|_| "Startup is not installed. Run scripts/install-startup.ps1 once with administrator rights.".to_owned())?;
    let mut bytes = vec![];
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Invalid installation receipt size".into());
    }
    let receipt: StartupInstallation =
        serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes))
            .map_err(|e| e.to_string())?;
    let sid = current_user_sid()?;
    let sam = sam_name()?;
    if receipt.schema != 1
        || receipt.user_sid != sid
        || receipt.task_name != format!("OmenHubPP-Profile-{sid}")
        || receipt.executable != root.join("Omen Hub++.exe")
    {
        return Err("Startup installation identity/path differs".into());
    }
    if !file_hash(&receipt.executable)?.eq_ignore_ascii_case(&receipt.sha256)
        || !file_hash(&std::env::current_exe().map_err(|e| e.to_string())?)?
            .eq_ignore_ascii_case(&receipt.sha256)
    {
        return Err(
            "Installed startup binary differs from this build; reinstall the current release"
                .into(),
        );
    }
    // SAFETY: Public COM initialization on the current thread, with paired lifetime.
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // RPC_E_CHANGED_MODE means the thread already owns a usable apartment.
    if initialized.is_err() && initialized.0 as u32 != 0x80010106 {
        return Err(initialized.to_string());
    }
    let _apartment = Apartment(initialized.is_ok());
    let query = || -> windows::core::Result<()> {
        // SAFETY: Local public Task Scheduler COM object; only fixed read operations.
        unsafe {
            let service: ITaskService =
                CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
            let empty = VARIANT::default();
            service.Connect(&empty, &empty, &empty, &empty)?;
            let task = service
                .GetFolder(&BSTR::from("\\"))?
                .GetTask(&BSTR::from(receipt.task_name.as_str()))?;
            if !task.Enabled()?.as_bool() {
                return Err(task_error("Scheduled task is disabled"));
            }
            verify_principal_xml(&task.Xml()?.to_string(), &sid).map_err(task_error)?;
            let definition = task.Definition()?;
            let triggers = definition.Triggers()?;
            let mut trigger_count = 0;
            triggers.Count(&mut trigger_count)?;
            if trigger_count != 1 {
                return Err(task_error("Scheduled task has a different trigger count"));
            }
            let trigger = triggers.get_Item(1)?;
            let mut enabled = Default::default();
            trigger.Enabled(&mut enabled)?;
            let logon_trigger: ILogonTrigger = trigger.cast()?;
            let mut trigger_user = BSTR::new();
            let mut delay = BSTR::new();
            logon_trigger.UserId(&mut trigger_user)?;
            logon_trigger.Delay(&mut delay)?;
            if !enabled.as_bool()
                || (trigger_user != sid && !trigger_user.to_string().eq_ignore_ascii_case(&sam))
                || delay != "PT15S"
            {
                return Err(task_error(
                    "Scheduled logon trigger identity/delay/enabled state differs",
                ));
            }
            let principal = definition.Principal()?;
            let mut user = BSTR::new();
            let mut logon = Default::default();
            let mut level = Default::default();
            principal.UserId(&mut user)?;
            principal.LogonType(&mut logon)?;
            principal.RunLevel(&mut level)?;
            if logon != TASK_LOGON_INTERACTIVE_TOKEN || level != TASK_RUNLEVEL_HIGHEST {
                return Err(task_error("Scheduled principal is not interactive/highest"));
            }
            let actions = definition.Actions()?;
            let mut count = 0;
            actions.Count(&mut count)?;
            if count != 1 {
                return Err(task_error("Scheduled task has a different action count"));
            }
            let action: IExecAction = actions.get_Item(1)?.cast()?;
            let mut path = BSTR::new();
            let mut arguments = BSTR::new();
            let mut directory = BSTR::new();
            action.Path(&mut path)?;
            action.Arguments(&mut arguments)?;
            action.WorkingDirectory(&mut directory)?;
            let action_path = PathBuf::from(path.to_string());
            let working_directory = PathBuf::from(directory.to_string());
            if action_path != receipt.executable
                || arguments != "profile-startup-apply"
                || working_directory != root
            {
                return Err(task_error(
                    "Scheduled executable/arguments/working directory differ",
                ));
            }
            Ok(())
        }
    };
    query().map_err(|e| format!("Startup task readback differs or is unavailable: {e}"))?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn task_sid_must_be_the_single_structural_principal() {
        let valid = "<Task><Principals><Principal><UserId>S-1-test</UserId></Principal></Principals></Task>";
        assert!(verify_principal_xml(valid, "S-1-test").is_ok());
        assert!(verify_principal_xml(valid, "S-2-other").is_err());
        assert!(verify_principal_xml("<Task><!-- S-1-test --><Principals><Principal><UserId>S-2-other</UserId></Principal></Principals></Task>", "S-1-test").is_err());
        assert!(verify_principal_xml("<Task><Principals><Principal><UserId>S-1-test</UserId></Principal><Principal><UserId>S-1-test</UserId></Principal></Principals></Task>", "S-1-test").is_err());
        assert!(verify_principal_xml("<!DOCTYPE Task><Task/>", "S-1-test").is_err());
    }
}
