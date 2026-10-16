//! CCD topology from the L3 cache domains, and the process access the
//! background service uses to pin the foreground program (documented
//! GetLogicalProcessorInformationEx / Get- and SetProcessAffinityMask).
use omen_core::ccd::{ProcessKey, Processes};
use std::ptr;
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, HANDLE},
    System::{
        SystemInformation::{
            CACHE_RELATIONSHIP, CacheUnified, GetLogicalProcessorInformationEx, RelationCache,
        },
        Threading::{
            GetCurrentProcessId, GetProcessAffinityMask, GetProcessTimes, OpenProcess,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, SetProcessAffinityMask,
        },
    },
    UI::WindowsAndMessaging::{GetForegroundWindow, GetShellWindow, GetWindowThreadProcessId},
};

/// Record header of SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX: Relationship and
/// Size (two 32-bit fields); the union follows at the next 8-byte boundary.
const HEADER: usize = 8;

/// Logical-processor masks of the unified L3 caches in processor group 0,
/// ascending. A Zen 4 CCD has exactly one L3, so these are the CCDs.
pub fn ccd_masks() -> Result<Vec<usize>, String> {
    let mut length = 0u32;
    // SAFETY: Documented size query with a null buffer.
    unsafe {
        GetLogicalProcessorInformationEx(RelationCache, ptr::null_mut(), &mut length);
    }
    if length == 0 || length > 1 << 20 {
        return Err("Processor topology size is not plausible".into());
    }
    // u64 storage keeps the records 8-byte aligned.
    let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
    // SAFETY: The buffer holds at least `length` bytes.
    if unsafe {
        GetLogicalProcessorInformationEx(RelationCache, buffer.as_mut_ptr().cast(), &mut length)
    } == 0
    {
        return Err(format!(
            "Processor topology: {}",
            std::io::Error::last_os_error()
        ));
    }
    let bytes = (length as usize).min(buffer.len() * 8);
    let base = buffer.as_ptr().cast::<u8>();
    let mut masks = vec![];
    let mut offset = 0;
    while offset + HEADER <= bytes {
        // SAFETY: Both header fields lie inside the bytes Windows wrote.
        let (relationship, size) = unsafe {
            (
                ptr::read_unaligned(base.add(offset).cast::<i32>()),
                ptr::read_unaligned(base.add(offset + 4).cast::<u32>()) as usize,
            )
        };
        if size < HEADER || offset + size > bytes {
            return Err("Processor topology record is malformed".into());
        }
        if relationship == RelationCache {
            if size < HEADER + size_of::<CACHE_RELATIONSHIP>() {
                return Err("Processor cache record is too short".into());
            }
            // SAFETY: The record is long enough for a complete cache entry.
            let cache = unsafe {
                ptr::read_unaligned(base.add(offset + HEADER).cast::<CACHE_RELATIONSHIP>())
            };
            if cache.Level == 3 && cache.Type == CacheUnified {
                // SAFETY: GroupMask is the first (and here only) group entry.
                let group = unsafe { cache.Anonymous.GroupMask };
                if cache.GroupCount > 1 || group.Group != 0 {
                    return Err("CCD affinity supports a single processor group".into());
                }
                masks.push(group.Mask);
            }
        }
        offset += size;
    }
    masks.sort_unstable();
    masks.dedup();
    if masks.contains(&0) || masks.windows(2).any(|pair| pair[0] & pair[1] != 0) {
        return Err("L3 cache domains overlap".into());
    }
    Ok(masks)
}

struct Process(HANDLE);
impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: Owned handle from OpenProcess.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
impl Process {
    fn open(pid: u32, access: u32) -> Result<Self, String> {
        // SAFETY: Plain handle request; null means failure.
        let handle = unsafe { OpenProcess(access, 0, pid) };
        if handle.is_null() {
            return Err(format!(
                "Process {pid}: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self(handle))
    }
    fn created(&self) -> Result<u64, String> {
        let mut times = [FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        }; 4];
        let [created, exited, kernel, user] = &mut times;
        // SAFETY: Valid handle and four writable FILETIME outputs.
        if unsafe { GetProcessTimes(self.0, created, exited, kernel, user) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok((u64::from(times[0].dwHighDateTime) << 32) | u64::from(times[0].dwLowDateTime))
    }
    /// Opens `key` only if it is still the same process (no PID reuse).
    fn of(key: ProcessKey, access: u32) -> Result<Self, String> {
        let process = Self::open(key.pid, access | PROCESS_QUERY_LIMITED_INFORMATION)?;
        if process.created()? != key.created {
            return Err(format!("Process {} has ended", key.pid));
        }
        Ok(process)
    }
}

/// The interactive desktop as seen by the background service.
pub struct WindowsProcesses {
    own: u32,
}
impl Default for WindowsProcesses {
    fn default() -> Self {
        Self {
            // SAFETY: No arguments.
            own: unsafe { GetCurrentProcessId() },
        }
    }
}
impl Processes for WindowsProcesses {
    fn foreground(&mut self) -> Option<ProcessKey> {
        // SAFETY: Plain window queries; null/zero results are handled.
        let (window, shell) = unsafe { (GetForegroundWindow(), GetShellWindow()) };
        if window.is_null() || window == shell {
            return None;
        }
        let mut pid = 0;
        // SAFETY: Valid output pointer.
        unsafe {
            GetWindowThreadProcessId(window, &mut pid);
        }
        // 0 = idle, 4 = System; never pin the service itself.
        if pid == 0 || pid == 4 || pid == self.own {
            return None;
        }
        let process = Process::open(pid, PROCESS_QUERY_LIMITED_INFORMATION).ok()?;
        Some(ProcessKey {
            pid,
            created: process.created().ok()?,
        })
    }
    fn affinity(&mut self, key: ProcessKey) -> Result<usize, String> {
        let process = Process::of(key, 0)?;
        let (mut mask, mut system) = (0usize, 0usize);
        // SAFETY: Valid handle and outputs.
        if unsafe { GetProcessAffinityMask(process.0, &mut mask, &mut system) } == 0 || mask == 0 {
            return Err(format!(
                "Affinity of process {}: {}",
                key.pid,
                std::io::Error::last_os_error()
            ));
        }
        Ok(mask)
    }
    fn set_affinity(&mut self, key: ProcessKey, mask: usize) -> Result<(), String> {
        let process = Process::of(key, PROCESS_SET_INFORMATION)?;
        // SAFETY: Valid handle with PROCESS_SET_INFORMATION.
        if unsafe { SetProcessAffinityMask(process.0, mask) } == 0 {
            return Err(format!(
                "Pinning process {}: {}",
                key.pid,
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_entry_follows_the_eight_byte_header() {
        // The union in SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX starts after
        // Relationship (i32) and Size (u32).
        let offset = std::mem::offset_of!(
            windows_sys::Win32::System::SystemInformation::SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
            Anonymous
        );
        assert_eq!(offset, HEADER);
    }
}
