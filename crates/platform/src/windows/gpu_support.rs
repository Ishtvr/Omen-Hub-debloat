//! Device evidence for NVIDIA clock/curve writes. By the user's decision of
//! 29 October 2026 any loaded driver version is accepted; the runtime ABI
//! checks, readback and restore protect against a changed driver.
//! Measured driver trials: 617.14 and 617.42 (2026-10-22: full-curve -15 MHz,
//! memory -24 MHz after a curve change and global +/-15/+/-24 MHz offsets,
//! every original control restored). This is control-path evidence, not an
//! overclock stability guarantee.
use super::gpu::NvidiaIdentity;
use omen_core::Inventory;

pub(super) fn driver_evidence(identity: &NvidiaIdentity) -> Result<(), String> {
    if identity.name != "NVIDIA GeForce RTX 5070 Laptop GPU" || identity.vbios != "98.06.2a.40.da" {
        return Err(omen_core::message::msg("support.err.gpu_vbios", &[]));
    }
    if identity.driver.trim().is_empty() {
        return Err(omen_core::message::msg("support.err.driver", &[]));
    }
    Ok(())
}

pub(super) fn model_evidence(
    inventory: &Inventory,
    identity: &NvidiaIdentity,
) -> Result<(), String> {
    if inventory.manufacturer != "HP" || inventory.board != "8E35" || inventory.bios != "F.13" {
        return Err(omen_core::message::msg("support.err.board", &[]));
    }
    driver_evidence(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(driver: &str) -> NvidiaIdentity {
        NvidiaIdentity {
            uuid: "test-device".into(),
            name: "NVIDIA GeForce RTX 5070 Laptop GPU".into(),
            vbios: "98.06.2a.40.da".into(),
            driver: driver.into(),
        }
    }

    #[test]
    fn any_loaded_driver_is_accepted_but_device_changes_and_missing_drivers_are_not() {
        let inventory = Inventory {
            manufacturer: "HP".into(),
            board: "8E35".into(),
            bios: "F.13".into(),
            ..Default::default()
        };
        for driver in ["617.14", "617.42", "617.43", "618.10"] {
            assert!(model_evidence(&inventory, &identity(driver)).is_ok());
        }
        for driver in ["", " "] {
            assert!(model_evidence(&inventory, &identity(driver)).is_err());
        }
        let mut other = identity("617.42");
        other.vbios = "98.06.2a.40.db".into();
        assert!(model_evidence(&inventory, &other).is_err());
        other = identity("617.42");
        other.name = "NVIDIA GeForce RTX 5070".into();
        assert!(model_evidence(&inventory, &other).is_err());
        let mut other_board = inventory;
        other_board.bios = "F.14".into();
        assert!(model_evidence(&other_board, &identity("617.42")).is_err());
    }
}
