//! Build-time evidence for exact NVIDIA driver/device combinations. A readable
//! ABI does not grant writes; that requires a measured complete restore trial.
use super::gpu::NvidiaIdentity;
use omen_core::Inventory;

pub(super) struct DriverEvidence {
    pub driver: &'static str,
    pub implementation_sha256: &'static str,
    pub writes_verified: bool,
}

const DRIVERS: [DriverEvidence; 2] = [
    DriverEvidence {
        driver: "617.14",
        implementation_sha256: "ff1527cea533c75f89033f7c0d9cdbaa6ec84bcac26c1db082227d68efd6a16e",
        writes_verified: true,
    },
    DriverEvidence {
        driver: "617.42",
        // 2026-10-22: Ghidra/exact function-range comparison plus full-curve
        // -15 MHz, memory -24 MHz after a curve change and global +/-15/+/-24
        // MHz trials. Every original control and production checkpoint restored.
        // This is control-path evidence, not an overclock stability guarantee.
        implementation_sha256: "c6a2014763c3662512b5d420dfc7632b777338881a13aeab3f21d8033eae6c71",
        writes_verified: true,
    },
];

pub(super) fn driver_evidence(
    identity: &NvidiaIdentity,
) -> Result<&'static DriverEvidence, String> {
    if identity.name != "NVIDIA GeForce RTX 5070 Laptop GPU" || identity.vbios != "98.06.2a.40.da" {
        return Err("GPU/VBIOS differs from the validated NVIDIA control contract".into());
    }
    DRIVERS
        .iter()
        .find(|entry| entry.driver == identity.driver)
        .ok_or_else(|| "NVIDIA control evidence exists only for drivers 617.14 and 617.42".into())
}

pub(super) fn model_evidence(
    inventory: &Inventory,
    identity: &NvidiaIdentity,
) -> Result<&'static DriverEvidence, String> {
    if inventory.manufacturer != "HP" || inventory.board != "8E35" || inventory.bios != "F.13" {
        return Err("GPU control evidence is limited to HP 8E35/F.13".into());
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
    fn evidence_is_exact_and_does_not_transfer_to_a_new_version_or_device() {
        let inventory = Inventory {
            manufacturer: "HP".into(),
            board: "8E35".into(),
            bios: "F.13".into(),
            ..Default::default()
        };
        for driver in ["617.14", "617.42"] {
            assert!(model_evidence(&inventory, &identity(driver)).is_ok());
        }
        for driver in ["617.43", "617.420", "617.14 ", ""] {
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

    #[test]
    fn implementation_identity_and_write_qualification_are_driver_specific() {
        let previous = driver_evidence(&identity("617.14")).unwrap();
        let candidate = driver_evidence(&identity("617.42")).unwrap();
        assert_ne!(
            previous.implementation_sha256,
            candidate.implementation_sha256
        );
        assert!(previous.writes_verified);
        assert!(candidate.writes_verified);
        assert_eq!(candidate.implementation_sha256.len(), 64);
    }
}
