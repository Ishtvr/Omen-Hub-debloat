//! Registers, reads or removes the "Start with Windows" logon task through
//! public COM. Highest run level needs administrator rights, which the
//! dashboard has.
use crate::tray_task::{ARGUMENTS, task_name, task_xml};
use std::path::Path;
use windows::{
    Win32::System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        TaskScheduler::{
            IExecAction, ITaskService, TASK_CREATE_OR_UPDATE, TASK_LOGON_INTERACTIVE_TOKEN,
            TaskScheduler,
        },
        Variant::VARIANT,
    },
    core::{BSTR, Interface},
};

/// HRESULT for "file not found": the task does not exist.
const NOT_FOUND: u32 = 0x80070002;

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

/// Program and arguments of this user's logon task; `None` when it is absent.
pub fn tray_task_command() -> Result<Option<(String, String)>, String> {
    let name = task_name(&super::current_user_sid()?);
    with_service(move |service| unsafe {
        let task = match service
            .GetFolder(&BSTR::from("\\"))?
            .GetTask(&BSTR::from(name.as_str()))
        {
            Ok(task) => task,
            Err(e) if e.code().0 as u32 == NOT_FOUND => return Ok(None),
            Err(e) => return Err(e),
        };
        let actions = task.Definition()?.Actions()?;
        let mut count = 0;
        actions.Count(&mut count)?;
        if count != 1 {
            return Ok(None);
        }
        let action: IExecAction = actions.get_Item(1)?.cast()?;
        let (mut path, mut arguments) = (BSTR::new(), BSTR::new());
        action.Path(&mut path)?;
        action.Arguments(&mut arguments)?;
        Ok(Some((
            path.to_string().trim_matches('"').to_owned(),
            arguments.to_string(),
        )))
    })
}

/// Creates or updates the task for `executable` with `arguments`, or removes
/// it with `None`.
pub fn set_tray_task(command: Option<(&Path, &str)>) -> Result<(), String> {
    if command.is_some_and(|(_, arguments)| !ARGUMENTS.contains(&arguments)) {
        return Err("Unsupported start arguments".into());
    }
    let sid = super::current_user_sid()?;
    let name = task_name(&sid);
    let xml = command.map(|(path, arguments)| task_xml(&sid, &path.to_string_lossy(), arguments));
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
                Err(e) if e.code().0 as u32 == NOT_FOUND => Ok(()),
                other => other,
            },
        }
    })
}
