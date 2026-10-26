#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
pub fn inventory() -> omen_core::Inventory {
    omen_core::Inventory {
        warnings: vec!["Hardware backend requires Windows".into()],
        ..Default::default()
    }
}

#[cfg(not(windows))]
pub fn power_status() -> omen_core::PowerStatus {
    Default::default()
}
#[cfg(any(windows, test))]
mod cooling_store;
#[cfg(any(windows, test))]
mod ownership_store;
pub mod profile_store;
