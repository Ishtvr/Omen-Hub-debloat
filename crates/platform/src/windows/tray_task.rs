//! Registers or removes the "Start with Windows" logon task through public
//! COM. Highest run level needs administrator rights, which the dashboard has.
use crate::tray_task::{task_name, task_xml};
use std::path::Path;
use windows::{
    Win32::System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        TaskScheduler::{
            ITaskService, TASK_CREATE_OR_UPDATE, TASK_LOGON_INTERACTIVE_TOKEN, TaskScheduler,
        },
        Variant::VARIANT,
    },
    core::BSTR,
};

/// COM runs on its own short-lived thread so the UI thread's apartment stays
/// as it is.
fn with_service<T: Send + 'static>(
    operation: impl FnOnce(&ITaskService) -> windows::core::Result<T> + Send + 'static,
) -> Result<T, String> {
    std::thread::spawn(move || {
        // SAFETY: Initialized and uninitialized on this thread only.
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(|e| e.to_string())?;
            let result = (|| {
                let service: ITaskService =
                    CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
                let empty = VARIANT::default();
                service.Connect(&empty, &empty, &empty, &empty)?;
                operation(&service)
            })()
            .map_err(|e| e.to_string());
            CoUninitialize();
            result
        }
    })
    .join()
    .map_err(|_| "Task Scheduler thread panicked".to_string())?
}

/// Whether the logon task for this user exists.
pub fn tray_task_installed() -> bool {
    let Ok(name) = super::current_user_sid().map(|sid| task_name(&sid)) else {
        return false;
    };
    with_service(move |service| unsafe {
        service
            .GetFolder(&BSTR::from("\\"))?
            .GetTask(&BSTR::from(name.as_str()))
            .map(|_| ())
    })
    .is_ok()
}

/// Creates or updates the task for `executable`, or removes it with `None`.
pub fn set_tray_task(executable: Option<&Path>) -> Result<(), String> {
    let sid = super::current_user_sid()?;
    let name = task_name(&sid);
    let xml = executable.map(|path| task_xml(&sid, &path.to_string_lossy()));
    with_service(move |service| unsafe {
        let folder = service.GetFolder(&BSTR::from("\\"))?;
        match xml {
            Some(xml) => {
                let empty = VARIANT::default();
                folder.RegisterTask(
                    &BSTR::from(name.as_str()),
                    &BSTR::from(xml.as_str()),
                    TASK_CREATE_OR_UPDATE.0,
                    &empty,
                    &empty,
                    TASK_LOGON_INTERACTIVE_TOKEN,
                    &empty,
                )?;
                Ok(())
            }
            None => match folder.DeleteTask(&BSTR::from(name.as_str()), 0) {
                // Already absent.
                Err(e) if e.code().0 as u32 == 0x80070002 => Ok(()),
                other => other,
            },
        }
    })
}
