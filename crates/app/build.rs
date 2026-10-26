//! Embeds the original application icon and version information into the
//! Windows executable. Explorer, desktop shortcuts, the taskbar and the tray
//! all use this one resource.
#[path = "build_support/win_resource.rs"]
mod win_resource;

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let icon = manifest.join("assets/omen-hub-pp.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-changed=build_support/win_resource.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        println!("cargo:warning=Application icon is embedded only with the MSVC linker");
        return;
    }
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = [0u16; 4];
    for (slot, part) in parts.iter_mut().zip(version.split(['.', '-', '+'])) {
        *slot = part.parse().unwrap_or(0);
    }
    let mut resources = win_resource::icon_resources(&fs::read(&icon).expect("Read app icon"))
        .expect("Valid app icon");
    resources.push(win_resource::version_resource(&win_resource::VersionInfo {
        version: parts,
        prerelease: !env!("CARGO_PKG_VERSION_PRE").is_empty(),
        strings: &[
            ("FileDescription", "Omen Hub++"),
            ("FileVersion", version),
            ("InternalName", "omen-hub-pp"),
            ("LegalCopyright", "MIT License"),
            ("OriginalFilename", "omen-hub-pp.exe"),
            ("ProductName", "Omen Hub++"),
            ("ProductVersion", version),
        ],
    }));
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("omen-hub-pp.res");
    fs::write(&output, win_resource::res_file(&resources)).expect("Write resource file");
    // link.exe and lld-link convert a .res input with their built-in cvtres.
    println!("cargo:rustc-link-arg-bins={}", output.display());
}
