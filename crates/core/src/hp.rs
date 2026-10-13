//! Strict decoder for the GPU-status response observed in the installed HP code.
//! This is not evidence of write support or of a particular sustained GPU wattage.
use serde::Serialize;

/// Observed fields of HP's 128-byte system design GET, not current power limits.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SystemDesignData {
    pub shipping_adapter_w: u16,
    pub capability_byte_raw: u8,
    pub software_fan_supported: bool,
    pub extreme_supported: bool,
    pub extreme_unlock_bit: bool,
    pub default_shared_budget_raw: u8,
    pub raw: Vec<u8>,
}

pub fn decode_system_design(return_code: u32, data: &[u8]) -> Result<SystemDesignData, String> {
    if return_code != 0 || data.len() != 128 {
        return Err("HP system-design GET failed or did not return exactly 128 bytes".into());
    }
    Ok(SystemDesignData {
        shipping_adapter_w: u16::from_le_bytes([data[0], data[1]]),
        capability_byte_raw: data[4],
        software_fan_supported: data[4] & 1 != 0,
        extreme_supported: data[4] & 2 != 0,
        extreme_unlock_bit: data[4] & 4 != 0,
        default_shared_budget_raw: data[8],
        raw: data.to_vec(),
    })
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct GpuPowerStatus {
    pub custom_tgp_enabled: bool,
    pub ppab_enabled: bool,
    pub d_state: u8,
    /// Board 8E35/F.13 returns 0 while its original Hub reports Unleashed.
    /// Preserve it without assigning a meaning from the older D1..D5 enum.
    pub d_state_known: bool,
    /// Preserve the raw field; zero's precise firmware semantics remain unverified.
    pub temperature_field_raw: u8,
    /// A zero raw field has no interpreted Celsius threshold.
    pub temperature_threshold_c: Option<u8>,
}

pub fn decode_gpu_status(return_code: u32, data: &[u8]) -> Result<GpuPowerStatus, String> {
    if return_code != 0 {
        return Err(format!("HP BIOS return code: {return_code:#x}"));
    }
    let [custom, ppab, state, temperature] = data else {
        return Err(format!(
            "HP GPU status needs exactly 4 bytes, received {}",
            data.len()
        ));
    };
    if *custom > 1 || *ppab > 1 || *state > 5 || *temperature > 110 {
        return Err(format!(
            "Unknown HP GPU-status layout; values were not interpreted. Raw reply: {data:02X?}"
        ));
    }
    Ok(GpuPowerStatus {
        custom_tgp_enabled: *custom == 1,
        ppab_enabled: *ppab == 1,
        d_state: *state,
        d_state_known: (1..=5).contains(state),
        temperature_field_raw: *temperature,
        temperature_threshold_c: (*temperature != 0).then_some(*temperature),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_design_preserves_unknown_bits_and_does_not_infer_a_current_mode() {
        let mut raw = [0; 128];
        raw[0] = 230;
        raw[4] = 0x0b;
        raw[8] = 55;
        let data = decode_system_design(0, &raw).unwrap();
        assert_eq!(data.shipping_adapter_w, 230);
        assert!(data.software_fan_supported && data.extreme_supported);
        assert!(!data.extreme_unlock_bit);
        assert_eq!(data.capability_byte_raw, 0x0b);
        assert_eq!(data.default_shared_budget_raw, 55);
        assert_eq!(data.raw, raw);
        assert!(decode_system_design(1, &raw).is_err());
        assert!(decode_system_design(0, &raw[..127]).is_err());
    }
    #[test]
    fn interprets_flags_without_claiming_watts() {
        assert_eq!(
            decode_gpu_status(0, &[1, 1, 1, 87]).unwrap(),
            GpuPowerStatus {
                custom_tgp_enabled: true,
                ppab_enabled: true,
                d_state: 1,
                d_state_known: true,
                temperature_field_raw: 87,
                temperature_threshold_c: Some(87),
            }
        );
        assert_eq!(
            decode_gpu_status(0, &[0, 0, 2, 0])
                .unwrap()
                .temperature_threshold_c,
            None
        );
    }
    #[test]
    fn firmware_errors_and_short_or_long_packets_are_not_success() {
        assert!(decode_gpu_status(3, &[0, 0, 1, 0]).is_err());
        for data in [&[][..], &[1, 1, 1][..], &[1, 1, 1, 87, 0][..]] {
            assert!(decode_gpu_status(0, data).is_err());
        }
    }
    #[test]
    fn unknown_layouts_are_rejected() {
        for data in [[2, 1, 1, 87], [1, 2, 1, 87], [1, 1, 6, 87], [1, 1, 1, 255]] {
            assert!(decode_gpu_status(0, &data).is_err());
        }
    }
    #[test]
    fn observed_zero_d_state_is_preserved_without_naming_it() {
        let status = decode_gpu_status(0, &[1, 1, 0, 87]).unwrap();
        assert!(status.custom_tgp_enabled && status.ppab_enabled);
        assert_eq!(status.d_state, 0);
        assert!(!status.d_state_known);
        assert_eq!(status.temperature_field_raw, 87);
    }
}
