//! Fixed, model-bound HP BIOS contracts. No watchdog or public raw opcode API.
use omen_core::{
    Inventory,
    hp::{GpuPowerStatus, SystemDesignData, decode_gpu_status, decode_system_design},
    keyboard::{KeyboardKind, KeyboardState, decode_keyboard_capability, decode_legacy_lighting},
};
use windows::{
    Win32::System::{
        Variant::{VARIANT, VT_ARRAY, VT_I4, VT_UI1, VT_UI4},
        Wmi::{CIM_FLAG_ARRAY, CIM_UINT8, CIM_UINT32, CIMTYPE_ENUMERATION},
    },
    core::{HSTRING, PCWSTR, w},
};
use wmi::{IWbemClassWrapper, Variant, WMIConnection, WMIError, WMIResult};

// Only fixed GET contracts observed in the installed manufacturer modules.
enum Read {
    GpuStatus,
    SystemDesign,
    KeyboardCapability,
    KeyboardColors,
    KeyboardBrightness,
}

struct ReadContract {
    command: u32,
    command_type: u32,
    input: &'static [u8],
    output_size: usize,
    method: &'static str,
}

impl Read {
    fn contract(self) -> ReadContract {
        let (command, command_type, input, output_size, method): (_, _, &[u8], _, _) = match self {
            Self::GpuStatus => (0x20008, 0x21, &[0; 4], 4, "hpqBIOSInt4"),
            Self::SystemDesign => (0x20008, 0x28, &[0; 4], 128, "hpqBIOSInt128"),
            Self::KeyboardCapability => (0x20008, 0x2B, &[], 4, "hpqBIOSInt4"),
            Self::KeyboardColors => (0x20009, 2, &[0], 128, "hpqBIOSInt128"),
            Self::KeyboardBrightness => (0x20009, 4, &[0], 128, "hpqBIOSInt128"),
        };
        ReadContract {
            command,
            command_type,
            input,
            output_size,
            method,
        }
    }
}

struct HpBios {
    connection: WMIConnection,
    path: String,
}

impl HpBios {
    fn new(inventory: &Inventory) -> WMIResult<Self> {
        // Even read support is initially restricted to the investigated firmware.
        if inventory.manufacturer != "HP" || inventory.board != "8E35" || inventory.bios != "F.13" {
            return Err(WMIError::ConvertVariantError(
                "HP reader is restricted to board 8E35 / BIOS F.13".into(),
            ));
        }
        let connection = WMIConnection::with_namespace_path("ROOT\\WMI")?;
        // HashMap deserialization deliberately omits WMI system properties,
        // including __PATH. Read the returned COM objects directly instead.
        let instances = connection.exec_query("SELECT * FROM hpqBIntM")?;
        let mut active = Vec::new();
        for instance in instances {
            let instance = instance?;
            if instance.get_property("Active")? == Variant::Bool(true) {
                active.push(instance);
            }
        }
        if active.len() != 1 {
            return Err(WMIError::ConvertVariantError(
                "Expected exactly one active HP BIOS instance".into(),
            ));
        }
        let path = match active[0].get_property("__PATH")? {
            Variant::String(path) if !path.is_empty() => path,
            _ => {
                return Err(WMIError::ConvertVariantError(
                    "HP BIOS instance path is missing".into(),
                ));
            }
        };
        Ok(Self { connection, path })
    }

    fn get(&self, read: Read) -> WMIResult<(u32, Vec<u8>)> {
        let contract = read.contract();
        self.invoke(&contract, contract.input)
    }
    fn set_colors(&self, colors: &[u8]) -> WMIResult<()> {
        if colors.len() != 128 {
            return Err(WMIError::ConvertVariantError(
                "A complete 128-byte color image is required".into(),
            ));
        }
        // Installed FourZone/OmenHsaClient: read full color image, replace RGB,
        // send command 0x20009/type3/size128 via hpqBIOSInt4. No brightness SET.
        let contract = ReadContract {
            command: 0x20009,
            command_type: 3,
            input: &[],
            output_size: 4,
            method: "hpqBIOSInt4",
        };
        self.invoke(&contract, colors).map(|_| ())
    }
    fn set_classic_mode_byte(&self, raw: u8) -> WMIResult<()> {
        if ![0x64, 0xe4].contains(&raw) {
            return Err(WMIError::ConvertVariantError(
                "Only measured classic mode bytes64/E4 can be written".into(),
            ));
        }
        let contract = ReadContract {
            command: 0x20009,
            command_type: 5,
            input: &[],
            output_size: 4,
            method: "hpqBIOSInt4",
        };
        self.invoke(&contract, &[raw, 0, 0, 0]).map(|_| ())
    }
    fn invoke(&self, contract: &ReadContract, bytes: &[u8]) -> WMIResult<(u32, Vec<u8>)> {
        let data = self.connection.get_object("hpqBDataIn")?.spawn_instance()?;
        data.put_property("Command", contract.command)?;
        data.put_property("CommandType", contract.command_type)?;
        data.put_property("Size", bytes.len() as u32)?;
        data.put_property("Sign", vec![0x53_u8, 0x45, 0x43, 0x55])?;
        if bytes.is_empty() {
            // wmi 0.18.4 converts Vec::<u8>::new() into VT_NULL, losing array type.
            // HP's native size-0 path uses VT_ARRAY|VT_UI1 with a null SAFEARRAY.
            let empty = empty_byte_array();
            // SAFETY: The instance/property exist, VARIANT is initialized and lives
            // through Put. COM copies it. Type must be 0 for instance properties.
            // windows::VARIANT clears its owned contents on drop (no array here).
            unsafe {
                data.inner.Put(w!("hpqBData"), 0, &empty, 0)?;
            }
        } else {
            data.put_property("hpqBData", bytes.to_vec())?;
        }
        let signature = self
            .connection
            .get_object("hpqBIntM")?
            .get_method(contract.method)?
            .ok_or_else(|| {
                WMIError::ConvertVariantError("HP GET input signature is missing".into())
            })?;
        let input = signature.spawn_instance()?;
        input.put_property("InData", Variant::Object(data))?;
        let output = self
            .connection
            .exec_method(&self.path, contract.method, Some(&input))?
            .ok_or_else(|| WMIError::ConvertVariantError("HP GET returned no output".into()))?;
        let output = match output.get_property("OutData")? {
            Variant::Object(object) => object,
            Variant::Unknown(object) => object.to_wbem_class_obj()?,
            _ => {
                return Err(WMIError::ConvertVariantError(
                    "HP OutData is not an embedded object".into(),
                ));
            }
        };
        let code: u32 = typed_property(&output, "rwReturnCode", CIM_UINT32)?.try_into()?;
        if code != 0 {
            return Err(WMIError::ConvertVariantError(format!(
                "HP BIOS return code: {code:#x}"
            )));
        }
        let bytes: Vec<u8> = typed_property(
            &output,
            "Data",
            CIMTYPE_ENUMERATION(CIM_FLAG_ARRAY.0 | CIM_UINT8.0),
        )?
        .try_into()?;
        if bytes.len() != contract.output_size {
            return Err(WMIError::ConvertVariantError(format!(
                "HP GET needs exactly {} output bytes, received {}",
                contract.output_size,
                bytes.len(),
            )));
        }
        Ok((code, bytes))
    }
}

/// Private typed RGB transport. The controller owns calibration/write gating,
/// durable capture, readback, and recovery; the transport never accepts opcodes.
pub(super) struct KeyboardControl {
    bios: HpBios,
    board: String,
    bios_version: String,
}
impl KeyboardControl {
    pub fn new(inventory: &Inventory) -> Result<Self, String> {
        Ok(Self {
            bios: HpBios::new(inventory).map_err(|e| query_error("keyboard", e))?,
            board: inventory.board.clone(),
            bios_version: inventory.bios.clone(),
        })
    }
    pub fn read(&self) -> Result<KeyboardState, String> {
        let get = |read| self.bios.get(read).map_err(|e| query_error("keyboard", e));
        let (code, bytes) = get(Read::KeyboardCapability)?;
        let capability = decode_keyboard_capability(code, &bytes)?;
        // Exact measured board/layout. One-zone and MCU variants need their own
        // SET/readback/restore qualification before this adapter can write them.
        if capability.kind != KeyboardKind::FourZoneWithNumpad || capability.raw != [1, 0, 0, 0] {
            return Err(
                "RGB control is restricted to the measured four-zone keyboard with numpad".into(),
            );
        }
        let (cc, colors) = get(Read::KeyboardColors)?;
        let (bc, brightness) = get(Read::KeyboardBrightness)?;
        Ok(KeyboardState {
            board: self.board.clone(),
            bios: self.bios_version.clone(),
            legacy: Some(decode_legacy_lighting(
                capability.kind,
                cc,
                &colors,
                bc,
                &brightness,
            )?),
            capability,
        })
    }
    pub fn set(
        &self,
        expected: &KeyboardState,
        colors: &[[u8; 3]],
    ) -> Result<KeyboardState, String> {
        expected.validate()?;
        let current = self.read()?;
        if current != *expected {
            return Err("Keyboard state changed after capture; no RGB SET was sent".into());
        }
        let target = current.with_colors(colors)?;
        if target != current {
            self.bios
                .set_colors(
                    &target
                        .legacy
                        .as_ref()
                        .ok_or("No legacy RGB image")?
                        .colors_raw,
                )
                .map_err(|e| query_error("keyboard-color SET", e))?;
        }
        Ok(target)
    }
    pub fn set_mode(
        &self,
        expected: &KeyboardState,
        custom: bool,
    ) -> Result<KeyboardState, String> {
        expected.validate()?;
        let current = self.read()?;
        if current != *expected {
            return Err("Keyboard state changed before mode SET".into());
        }
        let raw = current
            .legacy
            .as_ref()
            .ok_or("No classic keyboard state")?
            .brightness_raw_byte;
        if ![0x64, 0xe4].contains(&raw) {
            return Err("Mode/restore is qualified only at the observed100/228 raw states".into());
        }
        let target = current.with_classic_mode(custom)?;
        if target != current {
            self.bios
                .set_classic_mode_byte(
                    target
                        .legacy
                        .as_ref()
                        .ok_or("Missing mode target")?
                        .brightness_raw_byte,
                )
                .map_err(|e| query_error("keyboard mode SET", e))?;
        }
        Ok(target)
    }
}

fn empty_byte_array() -> VARIANT {
    let mut value = VARIANT::default();
    // SAFETY: This selects the initialized VARIANT arm. Its zeroed SAFEARRAY
    // pointer is retained. There is no allocation or borrowed buffer to free.
    unsafe {
        (&mut value.Anonymous.Anonymous).vt = VT_ARRAY | VT_UI1;
    }
    value
}

fn typed_property(
    object: &IWbemClassWrapper,
    property: &str,
    expected_type: CIMTYPE_ENUMERATION,
) -> WMIResult<Variant> {
    let name = HSTRING::from(property);
    let mut value = VARIANT::default();
    let mut cim_type = 0;
    // SAFETY: Name is terminated; writable initialized outputs live through Get.
    // VARIANT's Drop clears the returned allocation, including on error paths.
    unsafe {
        object.inner.Get(
            PCWSTR::from_raw(name.as_ptr()),
            0,
            &mut value,
            Some(&mut cim_type),
            None,
        )?;
    }
    if cim_type != expected_type.0 {
        return Err(WMIError::ConvertVariantError(format!(
            "HP {property} has CIM type {cim_type:#x}, expected {:#x}",
            expected_type.0,
        )));
    }
    // Do not let the general WMI converter truncate an unexpected numeric array.
    let valid_variant = if expected_type == CIM_UINT32 {
        value.vt() == VT_I4 || value.vt() == VT_UI4
    } else {
        value.vt() == (VT_ARRAY | VT_UI1)
    };
    if !valid_variant {
        return Err(WMIError::ConvertVariantError(format!(
            "HP {property} has an unexpected VARIANT type {:#x}",
            value.vt().0,
        )));
    }
    Variant::from_variant(&value)?.convert_into_cim_type(expected_type)
}

pub fn read_hp_gpu_status(inventory: &Inventory) -> Result<GpuPowerStatus, String> {
    let query = || -> WMIResult<GpuPowerStatus> {
        let (code, bytes) = HpBios::new(inventory)?.get(Read::GpuStatus)?;
        decode_gpu_status(code, &bytes).map_err(WMIError::ConvertVariantError)
    };
    query().map_err(|error| query_error("GPU-status", error))
}

pub fn read_hp_system_design(inventory: &Inventory) -> Result<SystemDesignData, String> {
    let query = || -> WMIResult<SystemDesignData> {
        let (code, bytes) = HpBios::new(inventory)?.get(Read::SystemDesign)?;
        decode_system_design(code, &bytes).map_err(WMIError::ConvertVariantError)
    };
    query().map_err(|error| query_error("system-design", error))
}

pub fn read_hp_keyboard_state(inventory: &Inventory) -> Result<KeyboardState, String> {
    let query = || -> WMIResult<KeyboardState> {
        let bios = HpBios::new(inventory)?;
        let (code, bytes) = bios.get(Read::KeyboardCapability)?;
        let capability =
            decode_keyboard_capability(code, &bytes).map_err(WMIError::ConvertVariantError)?;
        let legacy = if capability.kind.legacy_zone_count().is_some() {
            let (colors_code, colors) = bios.get(Read::KeyboardColors)?;
            let (brightness_code, brightness) = bios.get(Read::KeyboardBrightness)?;
            Some(
                decode_legacy_lighting(
                    capability.kind,
                    colors_code,
                    &colors,
                    brightness_code,
                    &brightness,
                )
                .map_err(WMIError::ConvertVariantError)?,
            )
        } else {
            None
        };
        Ok(KeyboardState {
            board: inventory.board.clone(),
            bios: inventory.bios.clone(),
            capability,
            legacy,
        })
    };
    query().map_err(|error| query_error("keyboard", error))
}

fn query_error(read: &str, error: WMIError) -> String {
    let needs_admin = matches!(
        &error,
        WMIError::HResultError { hres } if matches!(*hres as u32, 0x80041003 | 0x80070005)
    );
    let mut message = format!("HP {read} query failed: {error}");
    if needs_admin {
        message.push_str(". Access denied; use an administrator terminal. The application does not elevate itself.");
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_input_retains_byte_array_type_without_allocation() {
        let value = empty_byte_array();
        assert_eq!(value.vt(), VT_ARRAY | VT_UI1);
        // SAFETY: The variant tag selects parray; the constructor initializes it.
        assert!(unsafe { value.Anonymous.Anonymous.Anonymous.parray.is_null() });
        let generic: VARIANT = Variant::from(Vec::<u8>::new()).try_into().unwrap();
        assert_ne!(value.vt(), generic.vt());
    }
}
