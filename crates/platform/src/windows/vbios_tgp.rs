//! Reversible, Defender-safe, Secure-Boot-compatible flashless GPU Dynamic Boost
//! TGP ceiling control (`115..=135 W`) for HP 8E35 / BIOS F.13 / NVIDIA GeForce
//! RTX 5070 Laptop GPU (`PCI\VEN_10DE&DEV_2D18&SUBSYS_8E35103C`).
//!
//! Empirically verified on 25 November 2026 (`nvlddmkm.sys 621.14`, VBIOS
//! `98.06.2a.40.da`, Secure Boot ON, Windows Defender 100% enabled with 0 alerts):
//! - `nvlddmkm.sys` caches the stock VBIOS verbatim under volatile
//!   `HKLM\SYSTEM\CurrentControlSet\Control\Class\<DriverKey>\Session\vbios`
//!   and honors a persistent `REG_BINARY` value `vbios` in `<DriverKey>` in
//!   stock Monolithic RM without touching `EnableGpuFirmware`, Secure Boot,
//!   driver signature enforcement, EEPROM flash, or Windows Defender.
//! - Parsing the PCI Option ROM chain (`ROM[0]`, `ROM[1]` EFI) and NVIDIA BIT
//!   header (`'P'` token -> `+0x2C` Power Policy Table v4.0) locates `Policy[2]`
//!   (`PWR_POLICY_TOTAL_GPU`), whose stock limits are `limitMin=5 W`,
//!   `limitRated=80 W`, `limitMax=115 W`, `inflection=[100, 60, 20, 17, 0] W`,
//!   and `tgpRated=100 W`.
//! - Coherently shifting `limitRated`, `limitMax`, `inflection[0]`, and `tgpRated`
//!   by `+(target_w - 115) * 1000 mW` preserves driver monotonicity checks and
//!   scales the full Dynamic Boost envelope (`base + 20 W cTGP + 15 W PPAB`) up
//!   to `135 W` after a native `cfgmgr32.dll` dGPU restart (`CM_Disable_DevNode`
//!   + `CM_Enable_DevNode`) followed by HP WMI Unleashed re-sync.
//! - Deleting `vbios` and restarting the dGPU restores exact stock `80 / 115 W`.
use crate::{
    profile_store::{ProfileStore, VbiosTgpState},
    windows::nvidia::read_nvml_power_limits,
};
use omen_core::{
    Inventory,
    message::msg,
    profiles::{
        GPU_DYNAMIC_BOOST_LIMIT_DEFAULT_MAX_W, GPU_DYNAMIC_BOOST_LIMIT_HARD_MAX_W,
        GPU_DYNAMIC_BOOST_LIMIT_MIN_W, PerformanceProfile,
    },
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Devices::DeviceAndDriverInstallation::{
        CM_Disable_DevNode, CM_Enable_DevNode, CM_Get_DevNode_Status, CM_Locate_DevNodeW,
        CR_SUCCESS, DN_STARTED,
    },
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS},
    System::Registry::{
        HKEY, HKEY_LOCAL_MACHINE, KEY_ENUMERATE_SUB_KEYS, KEY_QUERY_VALUE, KEY_SET_VALUE,
        REG_BINARY, REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegDeleteValueW, RegEnumKeyExW,
        RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    },
};

const DISPLAY_CLASS_GUID: &str = "{4d36e968-e325-11ce-bfc1-08002be10318}";
const PCI_ENUM_SUBKEY: &str = "SYSTEM\\CurrentControlSet\\Enum\\PCI";
const CLASS_ROOT_SUBKEY: &str = "SYSTEM\\CurrentControlSet\\Control\\Class";
const TARGET_PCI_PREFIX: &str = "VEN_10DE&DEV_2D18&SUBSYS_8E35103C";
const VALUE_VBIOS: &str = "vbios";
const MIN_ROM_BYTES: usize = 0x40000;
const MAX_ROM_BYTES: usize = 1024 * 1024;

// Verified stock Policy[2] (PWR_POLICY_TOTAL_GPU) invariants on 8E35 / 98.06.2a.40.da.
const STOCK_LIMIT_MIN_MW: u32 = 5_000;
const STOCK_LIMIT_RATED_MW: u32 = 80_000;
const STOCK_LIMIT_MAX_MW: u32 = 115_000;
const STOCK_INFLECTION_MW: [u32; 5] = [100_000, 60_000, 20_000, 17_000, 0];
const STOCK_TGP_RATED_MW: u32 = 100_000;

const PWR_POLICY_TABLE_VERSION_40: u8 = 0x40;
const PWR_POLICY_HEADER_SIZE: u8 = 56;
const PWR_POLICY_ENTRY_SIZE: u8 = 102;
const PWR_POLICY_ENTRY_COUNT: u8 = 20;
const PWR_POLICY_TGP_INDEX: u8 = 2;
const PWR_POLICY_CLASS_TOTAL_GPU: u8 = 1;
const PWR_POLICY_CHANNEL_TOTAL_GPU: u8 = 2;

const OFF_LIMIT_MIN: usize = 0x0D;
const OFF_LIMIT_RATED: usize = 0x11;
const OFF_LIMIT_MAX: usize = 0x15;
const OFF_INFLECTION_0: usize = 0x2D;
const OFF_TGP_RATED: usize = 0x49;

const DISABLE_SETTLE: Duration = Duration::from_millis(450);
const DEVNODE_WAIT: Duration = Duration::from_secs(12);
const NVML_WAIT: Duration = Duration::from_secs(8);
const POST_ENABLE_SETTLE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// No VBIOS override requested and none active.
    Unchanged,
    /// Override already active at the exact requested wattage (zero GPU restart).
    AlreadyActive(u32),
    /// Applied new VBIOS override and verified via NVML after dGPU restart.
    AppliedNow(u32),
    /// Removed active VBIOS override and verified stock 115 W via NVML after restart.
    RestoredToStock,
}

impl ApplyOutcome {
    pub fn gpu_restarted(self) -> bool {
        matches!(self, Self::AppliedNow(_) | Self::RestoredToStock)
    }
    pub fn active_watts(self) -> Option<u32> {
        match self {
            Self::AlreadyActive(w) | Self::AppliedNow(w) => Some(w),
            Self::Unchanged | Self::RestoredToStock => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VbiosTgpStatus {
    pub model_supported: bool,
    pub device_instance_id: Option<String>,
    pub driver_subkey: Option<String>,
    pub registry_override_present: bool,
    pub session_vbios_present: bool,
    pub session_vbios_stock_qualified: bool,
    pub stock_backup_present: bool,
    pub nvml_default_w: Option<u32>,
    pub nvml_max_w: Option<u32>,
    pub saved_state: Option<VbiosTgpState>,
    pub qualification_error: Option<String>,
}

#[derive(Debug, Clone)]
struct GpuDeviceLocation {
    instance_id: String,
    dev_inst: u32,
    driver_subkey: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Policy2Offsets {
    table_offset: usize,
    entry_offset: usize,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_u16_le(buf: &[u8], off: usize) -> Option<u16> {
    let bytes: [u8; 2] = buf.get(off..off + 2)?.try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

fn read_u32_le(buf: &[u8], off: usize) -> Option<u32> {
    let bytes: [u8; 4] = buf.get(off..off + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

fn write_u32_le(buf: &mut [u8], off: usize, value: u32) -> Result<(), String> {
    let slice = buf
        .get_mut(off..off + 4)
        .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;
    slice.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn model_supported(inventory: &Inventory) -> bool {
    inventory.manufacturer == "HP"
        && inventory.board == "8E35"
        && inventory.bios == "F.13"
        && inventory
            .gpus
            .iter()
            .any(|gpu| gpu.to_ascii_lowercase().contains("rtx 5070 laptop"))
}

fn driver_subkey_valid(driver: &str) -> bool {
    let Some(suffix) = driver
        .to_ascii_lowercase()
        .strip_prefix(&format!("{DISPLAY_CLASS_GUID}\\"))
        .map(str::to_owned)
    else {
        return false;
    };
    suffix.len() == 4 && suffix.bytes().all(|b| b.is_ascii_digit())
}

/// Pure structural parser for NVIDIA VBIOS PCI Option ROM + BIT `'P'` +
/// Power Policy Table v4.0 + `Policy[2]` (`PWR_POLICY_TOTAL_GPU`).
///
/// Validates EVERY stock invariant before allowing any patch; rejects unknown
/// VBIOS versions, modified tables, or corrupted ROMs without writing a byte.
fn locate_and_validate_stock_policy2(rom: &[u8]) -> Result<Policy2Offsets, String> {
    if !(MIN_ROM_BYTES..=MAX_ROM_BYTES).contains(&rom.len()) {
        return Err(msg("vbios_tgp.err.rom_layout", &[]));
    }
    if rom.get(0..2) != Some(&[0x55, 0xAA]) {
        return Err(msg("vbios_tgp.err.rom_layout", &[]));
    }

    // Parse PCI Option ROM image chain to derive ROM[0] length and EFI ROM[1] shift.
    let pcir0_rel =
        usize::from(read_u16_le(rom, 0x18).ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?);
    if rom.get(pcir0_rel..pcir0_rel + 4) != Some(b"PCIR") {
        return Err(msg("vbios_tgp.err.rom_layout", &[]));
    }
    let rom0_len = usize::from(
        read_u16_le(rom, pcir0_rel + 0x10).ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?,
    ) * 512;
    let rom0_ind = *rom
        .get(pcir0_rel + 0x15)
        .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;
    if rom0_len == 0 || rom0_len >= rom.len() || (rom0_ind & 0x80) != 0 {
        return Err(msg("vbios_tgp.err.rom_layout", &[]));
    }
    if rom.get(rom0_len..rom0_len + 2) != Some(&[0x55, 0xAA]) {
        return Err(msg("vbios_tgp.err.rom_layout", &[]));
    }
    let pcir1_rel = usize::from(
        read_u16_le(rom, rom0_len + 0x18).ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?,
    );
    let pcir1 = rom0_len
        .checked_add(pcir1_rel)
        .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;
    if rom.get(pcir1..pcir1 + 4) != Some(b"PCIR") {
        return Err(msg("vbios_tgp.err.rom_layout", &[]));
    }
    let efi_len = usize::from(
        read_u16_le(rom, pcir1 + 0x10).ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?,
    ) * 512;
    if efi_len == 0 || rom0_len + efi_len >= rom.len() {
        return Err(msg("vbios_tgp.err.rom_layout", &[]));
    }

    // Locate BIT header (`FF B8 "BIT\0"`) inside ROM[0].
    let bit_sig = [0xFF, 0xB8, b'B', b'I', b'T', 0x00];
    let search_end = rom0_len.min(0x4000);
    let bit_off = rom[..search_end]
        .windows(bit_sig.len())
        .position(|w| w == bit_sig)
        .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;
    let bit_hdr_sz = usize::from(
        *rom.get(bit_off + 8)
            .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?,
    );
    let bit_tok_sz = usize::from(
        *rom.get(bit_off + 9)
            .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?,
    );
    let bit_tok_cnt = usize::from(
        *rom.get(bit_off + 10)
            .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?,
    );
    if bit_hdr_sz < 12 || bit_tok_sz != 6 || bit_tok_cnt == 0 {
        return Err(msg("vbios_tgp.err.rom_layout", &[]));
    }

    // Find BIT `'P'` (Performance / Power pointers, v2).
    let mut p_data_ptr = None;
    for index in 0..bit_tok_cnt {
        let tok_off = bit_off + bit_hdr_sz + index * bit_tok_sz;
        let id = *rom
            .get(tok_off)
            .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;
        if id == b'P' {
            let ver = *rom
                .get(tok_off + 1)
                .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;
            let data_sz = read_u16_le(rom, tok_off + 2)
                .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;
            let data_ptr = usize::from(
                read_u16_le(rom, tok_off + 4)
                    .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?,
            );
            if ver != 2 || usize::from(data_sz) < 0x30 {
                return Err(msg("vbios_tgp.err.rom_layout", &[]));
            }
            p_data_ptr = Some(data_ptr);
            break;
        }
    }
    let p_data_ptr = p_data_ptr.ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;
    let raw_tbl_ptr = read_u32_le(rom, p_data_ptr + 0x2C)
        .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))? as usize;
    let table_offset = if raw_tbl_ptr >= rom0_len {
        raw_tbl_ptr
            .checked_add(efi_len)
            .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?
    } else {
        raw_tbl_ptr
    };

    // Validate Power Policy Table v4.0 header.
    let hdr = rom
        .get(table_offset..table_offset + usize::from(PWR_POLICY_HEADER_SIZE))
        .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;
    if hdr[0] != PWR_POLICY_TABLE_VERSION_40
        || hdr[1] != PWR_POLICY_HEADER_SIZE
        || hdr[2] != PWR_POLICY_ENTRY_SIZE
        || hdr[3] != PWR_POLICY_ENTRY_COUNT
        || hdr[23] != PWR_POLICY_TGP_INDEX
    {
        return Err(msg("vbios_tgp.err.rom_layout", &[]));
    }

    let entry_offset = table_offset
        + usize::from(PWR_POLICY_HEADER_SIZE)
        + usize::from(PWR_POLICY_TGP_INDEX) * usize::from(PWR_POLICY_ENTRY_SIZE);
    let entry = rom
        .get(entry_offset..entry_offset + usize::from(PWR_POLICY_ENTRY_SIZE))
        .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;

    // Validate EVERY Policy[2] stock invariant before permitting a patch.
    let limit_min = read_u32_le(entry, OFF_LIMIT_MIN);
    let limit_rated = read_u32_le(entry, OFF_LIMIT_RATED);
    let limit_max = read_u32_le(entry, OFF_LIMIT_MAX);
    let tgp_rated = read_u32_le(entry, OFF_TGP_RATED);
    let infl = [
        read_u32_le(entry, OFF_INFLECTION_0),
        read_u32_le(entry, OFF_INFLECTION_0 + 4),
        read_u32_le(entry, OFF_INFLECTION_0 + 8),
        read_u32_le(entry, OFF_INFLECTION_0 + 12),
        read_u32_le(entry, OFF_INFLECTION_0 + 16),
    ];
    let infl_ok = infl
        .into_iter()
        .zip(STOCK_INFLECTION_MW)
        .all(|(actual, expected)| actual == Some(expected));

    if entry[0] != PWR_POLICY_CLASS_TOTAL_GPU
        || entry[1] != PWR_POLICY_CHANNEL_TOTAL_GPU
        || limit_min != Some(STOCK_LIMIT_MIN_MW)
        || limit_rated != Some(STOCK_LIMIT_RATED_MW)
        || limit_max != Some(STOCK_LIMIT_MAX_MW)
        || tgp_rated != Some(STOCK_TGP_RATED_MW)
        || !infl_ok
    {
        return Err(msg("vbios_tgp.err.rom_invariants", &[]));
    }

    Ok(Policy2Offsets {
        table_offset,
        entry_offset,
    })
}

/// Produces a coherently patched VBIOS image from a verified stock ROM for
/// `target_w` in `116..=135 W`, shifting `limitRated`, `limitMax`,
/// `inflection[0]`, and `tgpRated` by `+(target_w - 115) * 1000 mW`.
pub fn patch_policy2_for_target_w(stock_rom: &[u8], target_w: u32) -> Result<Vec<u8>, String> {
    patch_policy2_for_target_w_with_max(stock_rom, target_w, GPU_DYNAMIC_BOOST_LIMIT_DEFAULT_MAX_W)
}

/// Produces a coherently patched VBIOS image from a verified stock ROM for
/// `target_w` in `116..=max_w` (where `max_w` is clamped to `115..=175 W`),
/// shifting `limitRated`, `limitMax`, `inflection[0]`, and `tgpRated` by
/// `+(target_w - 115) * 1000 mW`.
pub fn patch_policy2_for_target_w_with_max(
    stock_rom: &[u8],
    target_w: u32,
    max_w: u32,
) -> Result<Vec<u8>, String> {
    let ceiling = max_w.clamp(
        GPU_DYNAMIC_BOOST_LIMIT_MIN_W,
        GPU_DYNAMIC_BOOST_LIMIT_HARD_MAX_W,
    );
    if !(GPU_DYNAMIC_BOOST_LIMIT_MIN_W + 1..=ceiling).contains(&target_w) {
        return Err(msg(
            "vbios_tgp.err.range",
            &[&GPU_DYNAMIC_BOOST_LIMIT_MIN_W, &ceiling],
        ));
    }
    let offsets = locate_and_validate_stock_policy2(stock_rom)?;
    let delta_mw = (target_w - GPU_DYNAMIC_BOOST_LIMIT_MIN_W)
        .checked_mul(1_000)
        .ok_or_else(|| msg("vbios_tgp.err.rom_layout", &[]))?;
    let new_rated = STOCK_LIMIT_RATED_MW + delta_mw;
    let new_max = STOCK_LIMIT_MAX_MW + delta_mw;
    let new_infl0 = STOCK_INFLECTION_MW[0] + delta_mw;
    let new_tgp_rated = STOCK_TGP_RATED_MW + delta_mw;

    // Driver invariant (`nvlddmkm`): inflection points stay strictly monotonic
    // and rated <= tgpRated <= max.
    if !(STOCK_LIMIT_MIN_MW < new_rated
        && new_rated <= new_tgp_rated
        && new_tgp_rated <= new_max
        && new_infl0 >= STOCK_INFLECTION_MW[1])
    {
        return Err(msg("vbios_tgp.err.rom_invariants", &[]));
    }

    let mut patched = stock_rom.to_vec();
    let base = offsets.entry_offset;
    write_u32_le(&mut patched, base + OFF_LIMIT_RATED, new_rated)?;
    write_u32_le(&mut patched, base + OFF_LIMIT_MAX, new_max)?;
    write_u32_le(&mut patched, base + OFF_INFLECTION_0, new_infl0)?;
    write_u32_le(&mut patched, base + OFF_TGP_RATED, new_tgp_rated)?;
    Ok(patched)
}

struct RegKey(HKEY);
impl Drop for RegKey {
    fn drop(&mut self) {
        // SAFETY: `self.0` was opened by `RegOpenKeyExW`.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
impl RegKey {
    fn open(subkey: &str, access: u32) -> Result<Self, String> {
        let mut handle: HKEY = ptr::null_mut();
        // SAFETY: Valid null-terminated UTF-16 subkey path and out pointer.
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_LOCAL_MACHINE,
                wide(subkey).as_ptr(),
                0,
                access,
                &mut handle,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("RegOpenKeyExW({subkey}): Win32 error {status}"));
        }
        Ok(Self(handle))
    }

    fn enum_subkeys(&self) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        let mut index: u32 = 0;
        loop {
            let mut buf = [0u16; 260];
            let mut len = buf.len() as u32;
            // SAFETY: Valid handle, writable buffer, and length in UTF-16 units.
            let status = unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    buf.as_mut_ptr(),
                    &mut len,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            };
            if status == ERROR_NO_MORE_ITEMS {
                break;
            }
            if status != ERROR_SUCCESS {
                return Err(format!("RegEnumKeyExW: Win32 error {status}"));
            }
            names.push(String::from_utf16(&buf[..len as usize]).map_err(|e| e.to_string())?);
            index += 1;
            if index > 4096 {
                break;
            }
        }
        Ok(names)
    }

    fn read_string(&self, name: &str) -> Result<Option<String>, String> {
        let mut value_type: u32 = 0;
        let mut size: u32 = 0;
        let wide_name = wide(name);
        // SAFETY: Querying required byte length with null buffer.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                wide_name.as_ptr(),
                ptr::null(),
                &mut value_type,
                ptr::null_mut(),
                &mut size,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != ERROR_SUCCESS {
            return Err(format!("RegQueryValueExW({name}): Win32 error {status}"));
        }
        if !matches!(value_type, REG_SZ | REG_EXPAND_SZ) || size == 0 || size > 4096 {
            return Err(format!("Registry value {name} has unexpected type/size"));
        }
        let mut units = vec![0u16; (size as usize).div_ceil(2)];
        let mut bytes = (units.len() * 2) as u32;
        // SAFETY: Buffer matches `bytes` length.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                wide_name.as_ptr(),
                ptr::null(),
                &mut value_type,
                units.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("RegQueryValueExW({name}): Win32 error {status}"));
        }
        let end = units.iter().position(|c| *c == 0).unwrap_or(units.len());
        String::from_utf16(&units[..end])
            .map(Some)
            .map_err(|e| e.to_string())
    }

    fn read_binary(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let mut value_type: u32 = 0;
        let mut size: u32 = 0;
        let wide_name = wide(name);
        // SAFETY: Querying required byte length with null buffer.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                wide_name.as_ptr(),
                ptr::null(),
                &mut value_type,
                ptr::null_mut(),
                &mut size,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status != ERROR_SUCCESS {
            return Err(format!("RegQueryValueExW({name}): Win32 error {status}"));
        }
        if value_type != REG_BINARY || size == 0 || (size as usize) > MAX_ROM_BYTES {
            return Err(format!("Registry binary {name} has unexpected type/size"));
        }
        let mut data = vec![0u8; size as usize];
        let mut bytes = size;
        // SAFETY: Buffer matches `bytes` length.
        let status = unsafe {
            RegQueryValueExW(
                self.0,
                wide_name.as_ptr(),
                ptr::null(),
                &mut value_type,
                data.as_mut_ptr(),
                &mut bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("RegQueryValueExW({name}): Win32 error {status}"));
        }
        data.truncate(bytes as usize);
        Ok(Some(data))
    }

    fn set_binary(&self, name: &str, data: &[u8]) -> Result<(), String> {
        let len = u32::try_from(data.len()).map_err(|_| "VBIOS image exceeds u32".to_owned())?;
        // SAFETY: Valid handle, UTF-16 name, and REG_BINARY payload.
        let status = unsafe {
            RegSetValueExW(
                self.0,
                wide(name).as_ptr(),
                0,
                REG_BINARY,
                data.as_ptr(),
                len,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("RegSetValueExW({name}): Win32 error {status}"));
        }
        Ok(())
    }

    fn delete_value(&self, name: &str) -> Result<(), String> {
        // SAFETY: Valid handle and null-terminated UTF-16 value name.
        let status = unsafe { RegDeleteValueW(self.0, wide(name).as_ptr()) };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("RegDeleteValueW({name}): Win32 error {status}"))
        }
    }
}

/// Dynamically resolves the present RTX 5070 Laptop GPU PnP devnode and its
/// display class registry key (`Control\Class\{4d36e968-...}\XXXX`), never
/// hardcoding `0000`.
fn locate_gpu_device() -> Result<GpuDeviceLocation, String> {
    let pci = RegKey::open(PCI_ENUM_SUBKEY, KEY_ENUMERATE_SUB_KEYS)?;
    for device_key in pci.enum_subkeys()? {
        if !device_key
            .to_ascii_uppercase()
            .starts_with(TARGET_PCI_PREFIX)
        {
            continue;
        }
        let dev_path = format!("{PCI_ENUM_SUBKEY}\\{device_key}");
        let Ok(dev_reg) = RegKey::open(&dev_path, KEY_ENUMERATE_SUB_KEYS) else {
            continue;
        };
        for instance in dev_reg.enum_subkeys()? {
            let instance_id = format!("PCI\\{device_key}\\{instance}");
            let mut dev_inst: u32 = 0;
            // SAFETY: Valid null-terminated PnP instance ID.
            let cr = unsafe { CM_Locate_DevNodeW(&mut dev_inst, wide(&instance_id).as_ptr(), 0) };
            if cr != CR_SUCCESS {
                continue;
            }
            let inst_path = format!("{dev_path}\\{instance}");
            let inst_reg = RegKey::open(&inst_path, KEY_QUERY_VALUE)?;
            let Some(driver) = inst_reg.read_string("Driver")? else {
                continue;
            };
            if !driver_subkey_valid(&driver) {
                continue;
            }
            let driver_subkey = format!("{CLASS_ROOT_SUBKEY}\\{driver}");
            return Ok(GpuDeviceLocation {
                instance_id,
                dev_inst,
                driver_subkey,
            });
        }
    }
    Err(msg("vbios_tgp.err.device_missing", &[]))
}

fn restart_gpu_and_verify(
    location: &GpuDeviceLocation,
    expected_max_w: u32,
    read_limits: &mut impl FnMut() -> Result<(u32, u32, u32), String>,
) -> Result<(), String> {
    // SAFETY: Verified RTX 5070 Laptop PnP devnode handle.
    let dis = unsafe { CM_Disable_DevNode(location.dev_inst, 0) };
    if dis != CR_SUCCESS {
        return Err(msg(
            "vbios_tgp.err.restart",
            &[&format!("disable=0x{dis:x}")],
        ));
    }
    std::thread::sleep(DISABLE_SETTLE);
    // SAFETY: Immediately re-enabling the same devnode.
    let en = unsafe { CM_Enable_DevNode(location.dev_inst, 0) };
    if en != CR_SUCCESS {
        return Err(msg("vbios_tgp.err.restart", &[&format!("enable=0x{en:x}")]));
    }

    let deadline = Instant::now() + DEVNODE_WAIT;
    loop {
        let mut status: u32 = 0;
        let mut problem: u32 = 0;
        // SAFETY: Valid devnode handle and out pointers.
        let cr = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, location.dev_inst, 0) };
        if cr == CR_SUCCESS && (status & DN_STARTED) != 0 && problem == 0 {
            break;
        }
        if Instant::now() >= deadline {
            return Err(msg(
                "vbios_tgp.err.restart",
                &[&format!(
                    "cr=0x{cr:x} status=0x{status:x} problem={problem}"
                )],
            ));
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    std::thread::sleep(POST_ENABLE_SETTLE);

    let nvml_deadline = Instant::now() + NVML_WAIT;
    loop {
        let last_error = match verify_limits(expected_max_w, read_limits) {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };
        if Instant::now() >= nvml_deadline {
            return Err(msg("vbios_tgp.err.verify", &[&last_error]));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn verify_limits(
    expected_max_w: u32,
    read_limits: &mut impl FnMut() -> Result<(u32, u32, u32), String>,
) -> Result<(), String> {
    let expected_default_mw =
        STOCK_LIMIT_RATED_MW + (expected_max_w - GPU_DYNAMIC_BOOST_LIMIT_MIN_W) * 1_000;
    let actual = read_limits()?;
    if actual
        == (
            expected_default_mw,
            STOCK_LIMIT_MIN_MW,
            expected_max_w * 1_000,
        )
    {
        Ok(())
    } else {
        Err(format!(
            "NVML reported default/min/max {:?} mW (expected {expected_default_mw}/{STOCK_LIMIT_MIN_MW}/{} mW)",
            actual,
            expected_max_w * 1_000
        ))
    }
}

fn read_session_vbios(driver_subkey: &str) -> Result<Option<Vec<u8>>, String> {
    let session_subkey = format!("{driver_subkey}\\Session");
    let Ok(session) = RegKey::open(&session_subkey, KEY_QUERY_VALUE) else {
        return Ok(None);
    };
    session.read_binary(VALUE_VBIOS)
}

/// Read-only inspection of the flashless GPU Dynamic Boost TGP override.
pub fn inspect(inventory: &Inventory, store: &ProfileStore) -> VbiosTgpStatus {
    let supported = model_supported(inventory);
    let location = if supported {
        locate_gpu_device().ok()
    } else {
        None
    };
    let registry_override_present = location.as_ref().is_some_and(|loc| {
        RegKey::open(&loc.driver_subkey, KEY_QUERY_VALUE)
            .and_then(|k| k.read_binary(VALUE_VBIOS))
            .is_ok_and(|v| v.is_some())
    });
    let session_rom = location
        .as_ref()
        .and_then(|loc| read_session_vbios(&loc.driver_subkey).ok().flatten());
    let session_vbios_stock_qualified = session_rom
        .as_ref()
        .is_some_and(|rom| locate_and_validate_stock_policy2(rom).is_ok());
    let stock_backup_present = store.vbios_stock_path().is_file();
    let nvml = read_nvml_power_limits().ok();
    let qualification_error = if !supported {
        Some(msg("vbios_tgp.err.unsupported", &[]))
    } else if location.is_none() {
        Some(msg("vbios_tgp.err.device_missing", &[]))
    } else if session_rom.is_none() && !stock_backup_present {
        Some(msg("vbios_tgp.err.session_missing", &[]))
    } else if !session_vbios_stock_qualified && !stock_backup_present {
        Some(msg("vbios_tgp.err.rom_invariants", &[]))
    } else {
        None
    };
    VbiosTgpStatus {
        model_supported: supported,
        device_instance_id: location.as_ref().map(|l| l.instance_id.clone()),
        driver_subkey: location.map(|l| l.driver_subkey),
        registry_override_present,
        session_vbios_present: session_rom.is_some(),
        session_vbios_stock_qualified,
        stock_backup_present,
        nvml_default_w: nvml.map(|(d, _, _)| d / 1000),
        nvml_max_w: nvml.map(|(_, _, m)| m / 1000),
        saved_state: store.vbios_tgp_state(),
        qualification_error,
    }
}

struct CeilingSource {
    location: GpuDeviceLocation,
    saved: Option<VbiosTgpState>,
    original_override: Option<Vec<u8>>,
    stock_rom: Vec<u8>,
}
impl CeilingSource {
    fn original_watts(&self) -> u32 {
        self.saved
            .as_ref()
            .map_or(GPU_DYNAMIC_BOOST_LIMIT_MIN_W, |s| s.target_watts)
    }
}

fn load_source(inventory: &Inventory, store: &ProfileStore) -> Result<CeilingSource, String> {
    if !model_supported(inventory) {
        return Err(msg("vbios_tgp.err.unsupported", &[]));
    }
    if super::power_status().ac_connected != Some(true) {
        return Err(msg("vbios_tgp.err.ac", &[]));
    }
    let location = locate_gpu_device()?;
    let saved = store.vbios_tgp_state();
    let class_key = RegKey::open(&location.driver_subkey, KEY_QUERY_VALUE)?;
    let original_override = class_key.read_binary(VALUE_VBIOS)?;
    let stock_rom = match (&saved, &original_override) {
        (None, None) => read_session_vbios(&location.driver_subkey)?
            .ok_or_else(|| msg("vbios_tgp.err.session_missing", &[]))?,
        (Some(state), Some(bytes))
            if state
                .driver_subkey
                .eq_ignore_ascii_case(&location.driver_subkey)
                && sha256_hex(bytes) == state.applied_sha256 =>
        {
            let stock = store
                .read_vbios_stock()?
                .filter(|stock| sha256_hex(stock) == state.stock_sha256)
                .ok_or_else(|| msg("vbios_tgp.err.backup_missing", &[]))?;
            if patch_policy2_for_target_w_with_max(
                &stock,
                state.target_watts,
                GPU_DYNAMIC_BOOST_LIMIT_HARD_MAX_W,
            )? != *bytes
            {
                return Err(msg("vbios_tgp.err.foreign_override", &[]));
            }
            stock
        }
        _ => return Err(msg("vbios_tgp.err.foreign_override", &[])),
    };
    locate_and_validate_stock_policy2(&stock_rom)?;
    Ok(CeilingSource {
        location,
        saved,
        original_override,
        stock_rom,
    })
}

/// Read-only check that the reported ceiling matches the exact known source.
pub fn preflight_target(
    inventory: &Inventory,
    store: &ProfileStore,
    observed_max_mw: u32,
) -> Result<(), String> {
    let source = load_source(inventory, store)?;
    if observed_max_mw != source.original_watts() * 1_000 {
        return Err(msg("gpu.err.ceiling_changed", &[]));
    }
    Ok(())
}

/// Applies or restores the flashless GPU Dynamic Boost TGP override for
/// `profile`, respecting the user's configured Settings ceiling (`115..=175 W`).
/// Callers must close any live NVML/NVAPI handles before calling this so the
/// dGPU PnP restart runs cleanly with zero stale handles.
pub fn apply_for_profile(
    inventory: &Inventory,
    store: &ProfileStore,
    profile: &PerformanceProfile,
    mut read_limits: impl FnMut() -> Result<(u32, u32, u32), String>,
) -> Result<ApplyOutcome, String> {
    let max_w = store.gpu_tgp_max_w()?;
    let target = profile.gpu_dynamic_boost_limit_w_max(max_w)?;
    match target {
        Some(target_w) => apply_target_with_max(inventory, store, target_w, max_w, read_limits),
        None => {
            if store.vbios_tgp_state().is_some()
                && restore_if_active(inventory, store, &mut read_limits)?
            {
                Ok(ApplyOutcome::RestoredToStock)
            } else {
                Ok(ApplyOutcome::Unchanged)
            }
        }
    }
}

/// Existing ceiling writer with an explicit upper limit (`115..=175 W`).
pub fn apply_target_with_max(
    inventory: &Inventory,
    store: &ProfileStore,
    target_w: u32,
    max_w: u32,
    mut read_limits: impl FnMut() -> Result<(u32, u32, u32), String>,
) -> Result<ApplyOutcome, String> {
    let ceiling = max_w.clamp(
        GPU_DYNAMIC_BOOST_LIMIT_MIN_W,
        GPU_DYNAMIC_BOOST_LIMIT_HARD_MAX_W,
    );
    if !(GPU_DYNAMIC_BOOST_LIMIT_MIN_W..=ceiling).contains(&target_w) {
        return Err(msg(
            "vbios_tgp.err.range",
            &[&GPU_DYNAMIC_BOOST_LIMIT_MIN_W, &ceiling],
        ));
    }
    let source = load_source(inventory, store)?;
    let original_w = source.original_watts();
    verify_limits(original_w, &mut read_limits)?;
    if original_w == target_w {
        return Ok(if source.saved.is_some() {
            ApplyOutcome::AlreadyActive(target_w)
        } else {
            ApplyOutcome::Unchanged
        });
    }

    let stock_target = target_w == GPU_DYNAMIC_BOOST_LIMIT_MIN_W;
    let target = if stock_target {
        None
    } else {
        Some(patch_policy2_for_target_w_with_max(
            &source.stock_rom,
            target_w,
            ceiling,
        )?)
    };
    let location = &source.location;
    let class_key = RegKey::open(&location.driver_subkey, KEY_QUERY_VALUE | KEY_SET_VALUE)?;
    if class_key.read_binary(VALUE_VBIOS)? != source.original_override {
        return Err(msg("vbios_tgp.err.foreign_override", &[]));
    }
    if source.saved.is_none() {
        store.save_vbios_stock(&source.stock_rom)?;
    }
    if let Some(bytes) = &target {
        store.save_vbios_tgp_state(Some(&VbiosTgpState {
            driver_subkey: location.driver_subkey.clone(),
            stock_sha256: sha256_hex(&source.stock_rom),
            applied_sha256: sha256_hex(bytes),
            target_watts: target_w,
        }))?;
    }
    let dispatch = match &target {
        Some(bytes) => class_key.set_binary(VALUE_VBIOS, bytes),
        None => class_key.delete_value(VALUE_VBIOS),
    };
    drop(class_key);

    let dispatched = dispatch.is_ok();
    let result =
        dispatch.and_then(|()| restart_gpu_and_verify(location, target_w, &mut read_limits));
    if let Err(error) = result {
        let restored = (|| {
            let rollback_key =
                RegKey::open(&location.driver_subkey, KEY_QUERY_VALUE | KEY_SET_VALUE)?;
            let current = rollback_key.read_binary(VALUE_VBIOS)?;
            if current != source.original_override {
                if current != target {
                    return Err(msg("vbios_tgp.err.foreign_override", &[]));
                }
                match &source.original_override {
                    Some(bytes) => rollback_key.set_binary(VALUE_VBIOS, bytes)?,
                    None => rollback_key.delete_value(VALUE_VBIOS)?,
                }
            }
            drop(rollback_key);
            store.save_vbios_tgp_state(source.saved.as_ref())?;
            if dispatched && verify_limits(original_w, &mut read_limits).is_err() {
                restart_gpu_and_verify(location, original_w, &mut read_limits)?;
            }
            Ok::<(), String>(())
        })();
        return match restored {
            Ok(()) => Err(error),
            Err(restore_error) => Err(msg("vbios_tgp.err.rollback", &[&error, &restore_error])),
        };
    }
    if stock_target {
        store.save_vbios_tgp_state(None)?;
        Ok(ApplyOutcome::RestoredToStock)
    } else {
        Ok(ApplyOutcome::AppliedNow(target_w))
    }
}

/// Removes an Omen Hub++ flashless GPU TGP override and restarts the dGPU back
/// to stock `80 / 115 W`. Returns `Ok(true)` if an active override was reverted,
/// `Ok(false)` if none was active (zero GPU restart).
pub fn restore_if_active(
    inventory: &Inventory,
    store: &ProfileStore,
    mut read_limits: impl FnMut() -> Result<(u32, u32, u32), String>,
) -> Result<bool, String> {
    let Some(saved) = store.vbios_tgp_state() else {
        return Ok(false);
    };
    if !model_supported(inventory) {
        return Err(msg("vbios_tgp.err.unsupported", &[]));
    }
    let location = locate_gpu_device()?;
    // Recovery must never follow a saved registry path to another adapter,
    // nor delete a replacement installed after this experiment.
    if !saved
        .driver_subkey
        .eq_ignore_ascii_case(&location.driver_subkey)
    {
        return Err(msg("vbios_tgp.err.foreign_override", &[]));
    }
    let class_key = RegKey::open(&location.driver_subkey, KEY_QUERY_VALUE | KEY_SET_VALUE)?;
    let existing = class_key.read_binary(VALUE_VBIOS)?;
    let had_value = existing.is_some();
    if let Some(bytes) = existing {
        if sha256_hex(&bytes) != saved.applied_sha256 {
            return Err(msg("vbios_tgp.err.foreign_override", &[]));
        }
        class_key.delete_value(VALUE_VBIOS)?;
    }
    drop(class_key);
    let already_stock = verify_limits(GPU_DYNAMIC_BOOST_LIMIT_MIN_W, &mut read_limits).is_ok();
    if had_value || !already_stock {
        restart_gpu_and_verify(&location, GPU_DYNAMIC_BOOST_LIMIT_MIN_W, &mut read_limits)?;
    }
    store.save_vbios_tgp_state(None)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal synthetic ROM matching the exact PCI Option ROM + BIT +
    /// Power Policy Table v4.0 + Policy[2] geometry verified on 8E35 / 98.06.2a.40.da.
    fn synthetic_stock_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x71B00];
        // ROM[0] header @ 0x0, PCIR @ 0x100, len = 0xFA00 (125 * 512)
        rom[0] = 0x55;
        rom[1] = 0xAA;
        rom[0x18..0x1A].copy_from_slice(&0x0100u16.to_le_bytes());
        rom[0x100..0x104].copy_from_slice(b"PCIR");
        rom[0x110..0x112].copy_from_slice(&((0xFA00u16) / 512).to_le_bytes());
        rom[0x115] = 0x00; // more images follow

        // ROM[1] (EFI) header @ 0xFA00, PCIR @ +0x1C, len = 0x18000 (192 * 512)
        rom[0xFA00] = 0x55;
        rom[0xFA01] = 0xAA;
        rom[0xFA18..0xFA1A].copy_from_slice(&0x001Cu16.to_le_bytes());
        rom[0xFA1C..0xFA20].copy_from_slice(b"PCIR");
        rom[0xFA2C..0xFA2E].copy_from_slice(&((0x18000u32 / 512) as u16).to_le_bytes());
        rom[0xFA31] = 0x80;

        // BIT header @ 0xDF0: hdrSz=12, tokSz=6, tokCnt=1 ('P' v2, dsz=312, dptr=0xF36)
        let bit = 0xDF0;
        rom[bit..bit + 6].copy_from_slice(&[0xFF, 0xB8, b'B', b'I', b'T', 0x00]);
        rom[bit + 8] = 12;
        rom[bit + 9] = 6;
        rom[bit + 10] = 1;
        let tok = bit + 12;
        rom[tok] = b'P';
        rom[tok + 1] = 2;
        rom[tok + 2..tok + 4].copy_from_slice(&312u16.to_le_bytes());
        rom[tok + 4..tok + 6].copy_from_slice(&0x0F36u16.to_le_bytes());
        // P[+0x2C] = 0x3FA17 -> mapped +0x18000 = 0x57A17
        rom[0xF36 + 0x2C..0xF36 + 0x30].copy_from_slice(&0x3FA17u32.to_le_bytes());

        // Power Policy Table v4.0 @ 0x57A17
        let tbl = 0x57A17;
        rom[tbl] = 0x40;
        rom[tbl + 1] = 56;
        rom[tbl + 2] = 102;
        rom[tbl + 3] = 20;
        rom[tbl + 23] = 2;

        // Policy[2] @ 0x57B1B
        let p2 = 0x57B1B;
        rom[p2] = 1;
        rom[p2 + 1] = 2;
        rom[p2 + OFF_LIMIT_MIN..p2 + OFF_LIMIT_MIN + 4]
            .copy_from_slice(&STOCK_LIMIT_MIN_MW.to_le_bytes());
        rom[p2 + OFF_LIMIT_RATED..p2 + OFF_LIMIT_RATED + 4]
            .copy_from_slice(&STOCK_LIMIT_RATED_MW.to_le_bytes());
        rom[p2 + OFF_LIMIT_MAX..p2 + OFF_LIMIT_MAX + 4]
            .copy_from_slice(&STOCK_LIMIT_MAX_MW.to_le_bytes());
        for (i, val) in STOCK_INFLECTION_MW.iter().enumerate() {
            let off = p2 + OFF_INFLECTION_0 + i * 4;
            rom[off..off + 4].copy_from_slice(&val.to_le_bytes());
        }
        rom[p2 + OFF_TGP_RATED..p2 + OFF_TGP_RATED + 4]
            .copy_from_slice(&STOCK_TGP_RATED_MW.to_le_bytes());
        rom
    }

    #[test]
    fn parser_locates_exact_policy2_offsets_and_patches_coherently() {
        let stock = synthetic_stock_rom();
        let offsets = locate_and_validate_stock_policy2(&stock).unwrap();
        assert_eq!(offsets.table_offset, 0x57A17);
        assert_eq!(offsets.entry_offset, 0x57B1B);

        // 125 W (+10 W shift)
        let p125 = patch_policy2_for_target_w(&stock, 125).unwrap();
        assert_eq!(read_u32_le(&p125, 0x57B1B + OFF_LIMIT_MIN), Some(5_000));
        assert_eq!(read_u32_le(&p125, 0x57B1B + OFF_LIMIT_RATED), Some(90_000));
        assert_eq!(read_u32_le(&p125, 0x57B1B + OFF_LIMIT_MAX), Some(125_000));
        assert_eq!(
            read_u32_le(&p125, 0x57B1B + OFF_INFLECTION_0),
            Some(110_000)
        );
        assert_eq!(
            read_u32_le(&p125, 0x57B1B + OFF_INFLECTION_0 + 4),
            Some(60_000)
        );
        assert_eq!(read_u32_le(&p125, 0x57B1B + OFF_TGP_RATED), Some(110_000));

        // 135 W (+20 W shift)
        let p135 = patch_policy2_for_target_w(&stock, 135).unwrap();
        assert_eq!(read_u32_le(&p135, 0x57B1B + OFF_LIMIT_RATED), Some(100_000));
        assert_eq!(read_u32_le(&p135, 0x57B1B + OFF_LIMIT_MAX), Some(135_000));
        assert_eq!(
            read_u32_le(&p135, 0x57B1B + OFF_INFLECTION_0),
            Some(120_000)
        );
        assert_eq!(read_u32_le(&p135, 0x57B1B + OFF_TGP_RATED), Some(120_000));

        // Double-patching an already patched ROM MUST be rejected!
        assert!(patch_policy2_for_target_w(&p125, 135).is_err());

        // Only the 4 patched fields differ from stock.
        let diffs: Vec<usize> = stock
            .iter()
            .zip(p135.iter())
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect();
        assert!(diffs.iter().all(|off| {
            [
                OFF_LIMIT_RATED,
                OFF_LIMIT_MAX,
                OFF_INFLECTION_0,
                OFF_TGP_RATED,
            ]
            .iter()
            .any(|f| (0x57B1B + f..0x57B1B + f + 4).contains(off))
        }));
    }

    #[test]
    fn patcher_rejects_out_of_bounds_targets_and_unmatched_roms() {
        let stock = synthetic_stock_rom();
        for bad in [0, 80, 114, 115, 136, 150, 200] {
            assert!(patch_policy2_for_target_w(&stock, bad).is_err(), "{bad}");
        }

        // Corrupted Policy[2] limitMax must fail validation.
        let mut wrong_max = stock.clone();
        write_u32_le(&mut wrong_max, 0x57B1B + OFF_LIMIT_MAX, 140_000).unwrap();
        assert!(locate_and_validate_stock_policy2(&wrong_max).is_err());

        // Corrupted inflection[1] must fail validation.
        let mut wrong_infl = stock.clone();
        write_u32_le(&mut wrong_infl, 0x57B1B + OFF_INFLECTION_0 + 4, 65_000).unwrap();
        assert!(locate_and_validate_stock_policy2(&wrong_infl).is_err());

        // Truncated ROM must fail.
        assert!(locate_and_validate_stock_policy2(&stock[..1024]).is_err());

        // Custom ceiling above 135 W works up to configured ceiling (clamped to 175 W).
        let p150 = patch_policy2_for_target_w_with_max(&stock, 150, 150).unwrap();
        assert_eq!(read_u32_le(&p150, 0x57B1B + OFF_LIMIT_RATED), Some(115_000));
        assert_eq!(read_u32_le(&p150, 0x57B1B + OFF_LIMIT_MAX), Some(150_000));
        assert_eq!(
            read_u32_le(&p150, 0x57B1B + OFF_INFLECTION_0),
            Some(135_000)
        );
        assert_eq!(read_u32_le(&p150, 0x57B1B + OFF_TGP_RATED), Some(135_000));
        assert!(patch_policy2_for_target_w_with_max(&stock, 151, 150).is_err());
        assert!(patch_policy2_for_target_w_with_max(&stock, 176, 200).is_err());
    }

    #[test]
    fn driver_subkey_validation_accepts_only_display_class_instances() {
        assert!(driver_subkey_valid(
            "{4d36e968-e325-11ce-bfc1-08002be10318}\\0000"
        ));
        assert!(driver_subkey_valid(
            "{4D36E968-E325-11CE-BFC1-08002BE10318}\\0003"
        ));
        assert!(!driver_subkey_valid(
            "{4d36e968-e325-11ce-bfc1-08002be10318}\\0000\\Session"
        ));
        assert!(!driver_subkey_valid("..\\0000"));
        assert!(!driver_subkey_valid(
            "{00000000-0000-0000-0000-000000000000}\\0000"
        ));
    }
}
