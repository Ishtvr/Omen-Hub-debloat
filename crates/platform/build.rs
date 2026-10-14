//! Optional signed module is staged at build time, never downloaded at runtime.
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rustc-check-cfg=cfg(cpu_sensor_module)");
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../.tools/pawnio/modules-0.2.11/AMDFamily17.bin");
    println!("cargo:rerun-if-changed={}", source.display());
    if source.is_file() {
        let bytes = fs::read(&source).expect("Read staged signed CPU module");
        assert_eq!(bytes.len(), 10652, "Unexpected CPU module size");
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            "dae74615761b78bdf064dfb3e136252ddcc6fc727d88f14738d0e5800d427a91",
            "Staged CPU module differs from the audited signed release"
        );
        fs::write(
            PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("cpu-module.bin"),
            bytes,
        )
        .expect("Stage embedded CPU module");
        println!("cargo:rustc-cfg=cpu_sensor_module");
    } else {
        println!(
            "cargo:warning=CPU hardware reader unavailable: run scripts/prepare-cpu-module.ps1 for the pinned signed module"
        );
    }
}
