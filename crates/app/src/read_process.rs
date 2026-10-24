//! Bounded, fixed read commands in disposable children. Driver/provider process
//! state and threads are released on exit; a private job bounds hung readers.
use serde::de::DeserializeOwned;
use std::{
    alloc::{Layout, alloc, dealloc},
    fs::File,
    io::Read,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, IntoRawHandle, OwnedHandle},
    },
    ptr::{self, NonNull},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, SetHandleInformation,
        WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    },
    System::{JobObjects::*, Pipes::CreatePipe, Threading::*},
};

pub const READ_TIMEOUT: Duration = Duration::from_secs(5);

struct ReadJob(HANDLE);
impl ReadJob {
    fn new() -> Result<Self, String> {
        // SAFETY: Job handle is owned here. Its only member will be our read-only
        // fixed read child; it is killed if the parent exits during a provider hang.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(format!(
                    "Isolated read job: {}",
                    std::io::Error::last_os_error()
                ));
            }
            let job = Self(handle);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            ) == 0
            {
                return Err(format!(
                    "Isolated read job limits: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(job)
        }
    }
}
impl Drop for ReadJob {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct ProcessAttributes {
    buffer: NonNull<u8>,
    layout: Layout,
}
impl ProcessAttributes {
    fn new() -> Result<Self, String> {
        let mut bytes = 0;
        // SAFETY: The initial call obtains the buffer size for two attributes.
        unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &mut bytes) };
        if bytes == 0 {
            return Err(native_error("Isolated read process attribute size"));
        }
        // Match the Windows x64 heap alignment for the opaque native buffer.
        let layout = Layout::from_size_align(bytes, 16).map_err(|e| e.to_string())?;
        // SAFETY: Layout is nonzero and valid; this object owns the allocation.
        let buffer = NonNull::new(unsafe { alloc(layout) })
            .ok_or("Isolated read process attribute allocation failed")?;
        // SAFETY: The buffer has the size and alignment required by Windows.
        if unsafe { InitializeProcThreadAttributeList(buffer.as_ptr().cast(), 2, 0, &mut bytes) }
            == 0
        {
            let error = native_error("Isolated read process attributes");
            // SAFETY: Failed initialization needs only allocation cleanup.
            unsafe { dealloc(buffer.as_ptr(), layout) };
            return Err(error);
        }
        Ok(Self { buffer, layout })
    }

    fn set_handles(&mut self, attribute: u32, handles: &[HANDLE]) -> Result<(), String> {
        // SAFETY: Attribute values remain alive until the list is destroyed;
        // callers keep their handle arrays and owned handles in the same scope.
        if unsafe {
            UpdateProcThreadAttribute(
                self.buffer.as_ptr().cast(),
                0,
                attribute as usize,
                handles.as_ptr().cast(),
                std::mem::size_of_val(handles),
                ptr::null_mut(),
                ptr::null(),
            )
        } == 0
        {
            return Err(native_error("Isolated read process attribute"));
        }
        Ok(())
    }
}
impl Drop for ProcessAttributes {
    fn drop(&mut self) {
        // SAFETY: The list is initialized and has its original owned allocation.
        unsafe {
            DeleteProcThreadAttributeList(self.buffer.as_ptr().cast());
            dealloc(self.buffer.as_ptr(), self.layout);
        }
    }
}

fn native_error(context: &str) -> String {
    format!("{context}: {}", std::io::Error::last_os_error())
}

fn spawn_read(job: &ReadJob, read: ReadCommand) -> Result<(OwnedHandle, File), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut application: Vec<u16> = executable.as_os_str().encode_wide().collect();
    // The executable path cannot contain quotes. The only argument is fixed,
    // and lpApplicationName selects the exact current executable explicitly.
    let mut command = vec![u16::from(b'"')];
    command.extend_from_slice(&application);
    command.extend(format!("\" {}\0", read.argument()).encode_utf16());
    application.push(0);
    // SAFETY: All native structures are initialized and all created handles are
    // immediately owned. Only this child's pipe writer and NUL are inherited.
    unsafe {
        let security = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: ptr::null_mut(),
            bInheritHandle: 1,
        };
        let (mut read, mut write) = (ptr::null_mut(), ptr::null_mut());
        if CreatePipe(&mut read, &mut write, &security, 0) == 0 {
            return Err(native_error("Isolated read stdout pipe"));
        }
        let read = OwnedHandle::from_raw_handle(read);
        let write = OwnedHandle::from_raw_handle(write);
        if SetHandleInformation(read.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) == 0 {
            return Err(native_error("Isolated read stdout inheritance"));
        }
        let null_name: Vec<u16> = "NUL\0".encode_utf16().collect();
        let null = CreateFileW(
            null_name.as_ptr(),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &security,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        );
        if null == INVALID_HANDLE_VALUE {
            return Err(native_error("Isolated read NUL handle"));
        }
        let null = OwnedHandle::from_raw_handle(null);
        // Declare values before the attribute list so they also outlive its
        // destructor on every error path, as required by the Windows contract.
        let jobs = [job.0];
        let inherited = [write.as_raw_handle(), null.as_raw_handle()];
        let mut attributes = ProcessAttributes::new()?;
        attributes.set_handles(PROC_THREAD_ATTRIBUTE_JOB_LIST, &jobs)?;
        attributes.set_handles(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &inherited)?;
        let mut startup: STARTUPINFOEXW = std::mem::zeroed();
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = null.as_raw_handle();
        startup.StartupInfo.hStdOutput = write.as_raw_handle();
        startup.StartupInfo.hStdError = null.as_raw_handle();
        startup.lpAttributeList = attributes.buffer.as_ptr().cast();
        let mut process: PROCESS_INFORMATION = std::mem::zeroed();
        if CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            1,
            CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT,
            ptr::null(),
            ptr::null(),
            &startup.StartupInfo,
            &mut process,
        ) == 0
        {
            return Err(native_error(
                "Isolated read process could not start in its lifetime job",
            ));
        }
        let process_handle = OwnedHandle::from_raw_handle(process.hProcess);
        let _thread_handle = OwnedHandle::from_raw_handle(process.hThread);
        // Local writer/NUL handles close on return. The child is already inside
        // the kill-on-close job before any of its code can execute.
        Ok((
            process_handle,
            File::from_raw_handle(read.into_raw_handle()),
        ))
    }
}

fn child_status(process: &OwnedHandle) -> Result<Option<u32>, String> {
    // SAFETY: This is the owned handle of the child created above.
    unsafe {
        match WaitForSingleObject(process.as_raw_handle(), 0) {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                if GetExitCodeProcess(process.as_raw_handle(), &mut code) == 0 {
                    return Err(native_error("Isolated read exit status"));
                }
                Ok(Some(code))
            }
            _ => Err(native_error("Isolated read process wait")),
        }
    }
}

#[derive(Clone, Copy)]
pub enum ReadCommand {
    Inventory,
    GpuTelemetry,
}
impl ReadCommand {
    fn argument(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::GpuTelemetry => "gpu-telemetry-read",
        }
    }
}

pub fn capture<T: DeserializeOwned>(command: ReadCommand) -> Result<T, String> {
    let job = ReadJob::new()?;
    let (child, output) = spawn_read(&job, command)?;
    // Drain concurrently so a full pipe cannot deadlock child exit. Bound memory.
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = output.take(65_537).read_to_end(&mut bytes).map(|_| bytes);
        let _ = tx.send(result);
    });
    let deadline = Instant::now() + READ_TIMEOUT;
    let status = loop {
        match child_status(&child) {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            result => {
                // Dropping this private job terminates only our fixed read child
                // and closes its writer. Do not wait indefinitely on teardown.
                drop(job);
                return Err(match result {
                    Err(error) => error,
                    _ => "Isolated read exceeded the 5-second deadline".into(),
                });
            }
        }
    };
    let bytes = rx
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .map_err(|_| "Isolated read output did not finish within the 5-second deadline")?
        .map_err(|e| e.to_string())?;
    if status != 0 {
        return Err("Isolated read process failed".into());
    }
    if bytes.len() > 65_536 {
        return Err("Isolated read response exceeded 64 KiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid isolated read response: {e}"))
}
