//! Power state of the NVIDIA dGPU as Windows records it in the device tree.
//! Reading this property never touches the device, unlike an NVAPI or NVML
//! query, which wakes a GPU that has powered down in hybrid mode.
use windows_sys::Win32::Devices::{
    DeviceAndDriverInstallation::{
        CM_GETIDLIST_FILTER_CLASS, CM_GETIDLIST_FILTER_PRESENT, CM_Get_DevNode_PropertyW,
        CM_Get_Device_ID_List_SizeW, CM_Get_Device_ID_ListW, CM_LOCATE_DEVNODE_NORMAL,
        CM_Locate_DevNodeW, CR_SUCCESS,
    },
    Properties::{DEVPKEY_Device_PowerData, DEVPROP_TYPE_BINARY},
};

/// Device setup class of display adapters.
const DISPLAY_CLASS: &str = "{4d36e968-e325-11ce-bfc1-08002be10318}";
const NVIDIA_PREFIX: &str = "PCI\\VEN_10DE&";

/// `CM_POWER_DATA.PD_MostRecentPowerState` (a `DEVICE_POWER_STATE`) from the
/// property bytes: the structure starts with its size, then this state.
fn recent_state(data: &[u8]) -> Option<u32> {
    let size = u32::from_le_bytes(data.get(..4)?.try_into().ok()?) as usize;
    if size < 8 || size > data.len() {
        return None;
    }
    Some(u32::from_le_bytes(data.get(4..8)?.try_into().ok()?))
}

/// D0 is awake; D1 to D3 are low-power states. Anything else is unknown.
fn awake(state: u32) -> Option<bool> {
    match state {
        1 => Some(true),
        2..=4 => Some(false),
        _ => None,
    }
}

/// Instance ids of the present display adapters (a multi-string list).
fn display_adapters() -> Option<Vec<Vec<u16>>> {
    let filter: Vec<u16> = DISPLAY_CLASS.encode_utf16().chain([0]).collect();
    let flags = CM_GETIDLIST_FILTER_CLASS | CM_GETIDLIST_FILTER_PRESENT;
    let mut length = 0;
    // SAFETY: Terminated filter string and a writable length.
    if unsafe { CM_Get_Device_ID_List_SizeW(&mut length, filter.as_ptr(), flags) } != CR_SUCCESS
        || length == 0
        || length > 65_536
    {
        return None;
    }
    let mut buffer = vec![0u16; length as usize];
    // SAFETY: The buffer has the length the size query returned.
    if unsafe { CM_Get_Device_ID_ListW(filter.as_ptr(), buffer.as_mut_ptr(), length, flags) }
        != CR_SUCCESS
    {
        return None;
    }
    Some(
        buffer
            .split(|c| *c == 0)
            .filter(|id| !id.is_empty())
            .map(|id| id.iter().copied().chain([0]).collect())
            .collect(),
    )
}

/// Whether the single NVIDIA display adapter is in D0. `None` when there is
/// no such adapter, more than one, or Windows does not report a state: the
/// caller then behaves as before this check existed.
pub fn nvidia_gpu_awake() -> Option<bool> {
    let adapters = display_adapters()?;
    let mut nvidia = adapters.iter().filter(|id| {
        String::from_utf16_lossy(id)
            .to_ascii_uppercase()
            .starts_with(NVIDIA_PREFIX)
    });
    let (id, None) = (nvidia.next()?, nvidia.next()) else {
        return None;
    };
    let mut node = 0;
    // SAFETY: Terminated instance id from the list above; writable node.
    if unsafe { CM_Locate_DevNodeW(&mut node, id.as_ptr(), CM_LOCATE_DEVNODE_NORMAL) } != CR_SUCCESS
    {
        return None;
    }
    let mut data = [0u8; 128];
    let mut size = data.len() as u32;
    let mut kind = 0;
    // SAFETY: Writable type, buffer and size for the duration of the call.
    let result = unsafe {
        CM_Get_DevNode_PropertyW(
            node,
            &DEVPKEY_Device_PowerData,
            &mut kind,
            data.as_mut_ptr(),
            &mut size,
            0,
        )
    };
    if result != CR_SUCCESS || kind != DEVPROP_TYPE_BINARY || size as usize > data.len() {
        return None;
    }
    awake(recent_state(&data[..size as usize])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_complete_power_record_yields_a_state() {
        // 56-byte CM_POWER_DATA as read on 8E35 with the dGPU in D0.
        let mut data = [0u8; 56];
        data[0] = 56;
        data[4] = 1;
        assert_eq!(recent_state(&data), Some(1));
        data[4] = 4;
        assert_eq!(recent_state(&data), Some(4));
        // Truncated, or a size field beyond the bytes read.
        assert_eq!(recent_state(&data[..7]), None);
        data[0] = 64;
        assert_eq!(recent_state(&data), None);
        data[0] = 4;
        assert_eq!(recent_state(&data), None);
    }

    #[test]
    fn d0_is_awake_and_unknown_states_are_not_guessed() {
        assert_eq!(awake(1), Some(true));
        for low in 2..=4 {
            assert_eq!(awake(low), Some(false));
        }
        for unknown in [0, 5, 99] {
            assert_eq!(awake(unknown), None);
        }
    }

    #[test]
    #[ignore = "reads the device tree of the reference device"]
    fn the_reference_device_reports_a_state() {
        println!("NVIDIA dGPU awake: {:?}", nvidia_gpu_awake());
        assert!(nvidia_gpu_awake().is_some());
    }
}
