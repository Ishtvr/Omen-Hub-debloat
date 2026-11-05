//! Keep the small event-driven UI and fixed control commands responsive under
//! CPU load. High is a Windows scheduling class, not a CPU power/clock setting.
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetPriorityClass, HIGH_PRIORITY_CLASS, SetPriorityClass,
};

pub fn configure() -> Result<(), String> {
    // SAFETY: The pseudo-handle belongs to this process and must not be closed.
    // Never request realtime priority: input and system housekeeping must run.
    unsafe {
        let process = GetCurrentProcess();
        if SetPriorityClass(process, HIGH_PRIORITY_CLASS) == 0 {
            return Err(omen_core::message::msg(
                "process.err.priority",
                &[&std::io::Error::last_os_error()],
            ));
        }
        if GetPriorityClass(process) != HIGH_PRIORITY_CLASS {
            return Err(omen_core::message::msg(
                "process.err.priority_readback",
                &[],
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_priority_is_set_and_read_back_without_realtime() {
        struct Restore(u32);
        impl Drop for Restore {
            fn drop(&mut self) {
                // SAFETY: Restore this test process's original scheduling class.
                unsafe { SetPriorityClass(GetCurrentProcess(), self.0) };
            }
        }
        let original = unsafe { GetPriorityClass(GetCurrentProcess()) };
        assert_ne!(original, 0);
        let _restore = Restore(original);
        configure().unwrap();
        assert_eq!(
            unsafe { GetPriorityClass(GetCurrentProcess()) },
            HIGH_PRIORITY_CLASS
        );
    }
}
