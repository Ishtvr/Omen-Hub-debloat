//! Inventory uses the same fixed, bounded child infrastructure as GPU telemetry.
use omen_core::Inventory;

pub fn read() -> Inventory {
    crate::read_process::capture(crate::read_process::ReadCommand::Inventory).unwrap_or_else(
        |error| Inventory {
            warnings: vec![error],
            ..Default::default()
        },
    )
}
