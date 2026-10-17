//! Values behind the dashboard quick switches: who controls the fans and the
//! BIOS graphics mode (MUX). Stored keys stay stable across releases.

/// Fan control chosen on the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FanMode {
    /// The BIOS regulates with its own default fan table; the service sends nothing.
    #[default]
    Bios,
    /// The service follows the saved shared curve (BIOS while none is saved).
    Curve,
    /// Both fans at maximum.
    Max,
}
impl FanMode {
    pub const ALL: [Self; 3] = [Self::Bios, Self::Curve, Self::Max];
    pub fn key(self) -> &'static str {
        match self {
            Self::Bios => "bios",
            Self::Curve => "curve",
            Self::Max => "max",
        }
    }
    /// Unknown values fall back to the BIOS. Before the three-way switch,
    /// "auto" meant following the saved curve, so it keeps that meaning.
    pub fn parse(stored: &str) -> Self {
        match stored {
            "curve" | "auto" => Self::Curve,
            "max" => Self::Max,
            _ => Self::Bios,
        }
    }
    /// Only the curve and maximum need the background fan service.
    pub fn needs_service(self) -> bool {
        self != Self::Bios
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }
    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
}

/// BIOS graphics mode offered on the dashboard. Hybrid is the default; the
/// BIOS applies a change at the next restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphicsMode {
    #[default]
    Hybrid,
    /// Discrete GPU only (MUX to the NVIDIA GPU).
    Discrete,
}
impl GraphicsMode {
    pub const ALL: [Self; 2] = [Self::Hybrid, Self::Discrete];
    pub fn key(self) -> &'static str {
        match self {
            Self::Hybrid => "hybrid",
            Self::Discrete => "discrete",
        }
    }
    pub fn parse(stored: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.key() == stored)
    }
    /// HP BIOS graphics mode byte: 0 Hybrid, 1 Discrete.
    pub fn bios_byte(self) -> u8 {
        match self {
            Self::Hybrid => 0,
            Self::Discrete => 1,
        }
    }
    pub fn from_bios_byte(byte: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.bios_byte() == byte)
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }
    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
}

/// Last graphics mode the dashboard wrote. `boot_mode` is the mode the
/// running Windows session started with (None: a mode the switch does not
/// offer, such as Optimus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsRecord {
    pub mode: GraphicsMode,
    pub written_unix_s: u64,
    pub boot_mode: Option<GraphicsMode>,
}
impl GraphicsRecord {
    /// Record for a write at `now`. `bios_before` is the BIOS byte read just
    /// before it; an earlier write in this boot already holds the boot mode.
    pub fn after_write(
        previous: Option<Self>,
        mode: GraphicsMode,
        bios_before: u8,
        now_unix_s: u64,
        boot_unix_s: u64,
    ) -> Self {
        let boot_mode = match previous {
            Some(record) if record.written_unix_s > boot_unix_s => record.boot_mode,
            _ => GraphicsMode::from_bios_byte(bios_before),
        };
        Self {
            mode,
            written_unix_s: now_unix_s,
            boot_mode,
        }
    }
    /// A mode chosen in this boot that differs from the running one takes
    /// effect only after a restart.
    pub fn restart_pending(&self, boot_unix_s: u64) -> bool {
        self.written_unix_s > boot_unix_s && self.boot_mode != Some(self.mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fan_modes_round_trip_and_keep_the_legacy_curve_meaning() {
        for mode in FanMode::ALL {
            assert_eq!(FanMode::parse(mode.key()), mode);
            assert_eq!(FanMode::from_index(mode.index()), Some(mode));
        }
        assert_eq!(FanMode::parse("auto"), FanMode::Curve);
        assert_eq!(FanMode::parse(""), FanMode::Bios);
        assert_eq!(FanMode::parse("turbo"), FanMode::Bios);
        assert_eq!(FanMode::default(), FanMode::Bios);
        assert!(!FanMode::Bios.needs_service());
        assert!(FanMode::Curve.needs_service() && FanMode::Max.needs_service());
        assert_eq!(FanMode::from_index(3), None);
    }

    #[test]
    fn graphics_modes_map_to_the_bios_bytes() {
        assert_eq!(GraphicsMode::default(), GraphicsMode::Hybrid);
        assert_eq!(GraphicsMode::Hybrid.bios_byte(), 0);
        assert_eq!(GraphicsMode::Discrete.bios_byte(), 1);
        for mode in GraphicsMode::ALL {
            assert_eq!(GraphicsMode::parse(mode.key()), Some(mode));
            assert_eq!(GraphicsMode::from_bios_byte(mode.bios_byte()), Some(mode));
            assert_eq!(GraphicsMode::from_index(mode.index()), Some(mode));
        }
        // Optimus (2) and UMA (3) are not offered by the quick switch.
        assert_eq!(GraphicsMode::from_bios_byte(2), None);
        assert_eq!(GraphicsMode::from_bios_byte(3), None);
        assert_eq!(GraphicsMode::parse("optimus"), None);
    }

    #[test]
    fn only_a_changed_mode_in_this_boot_waits_for_restart() {
        let boot = 1_000;
        // Hybrid boot, GPU only chosen: active after the restart.
        let discrete = GraphicsRecord::after_write(None, GraphicsMode::Discrete, 0, 1_100, boot);
        assert_eq!(discrete.boot_mode, Some(GraphicsMode::Hybrid));
        assert!(discrete.restart_pending(boot));
        // Back to Hybrid before restarting: the BIOS now holds GPU only, but
        // the running session is still Hybrid, so nothing is pending.
        let back =
            GraphicsRecord::after_write(Some(discrete), GraphicsMode::Hybrid, 1, 1_200, boot);
        assert_eq!(back.boot_mode, Some(GraphicsMode::Hybrid));
        assert!(!back.restart_pending(boot));
        // After the restart the earlier write is history.
        assert!(!discrete.restart_pending(1_150));
        let next =
            GraphicsRecord::after_write(Some(discrete), GraphicsMode::Hybrid, 1, 2_000, 1_150);
        assert_eq!(next.boot_mode, Some(GraphicsMode::Discrete));
        assert!(next.restart_pending(1_150));
        // An Optimus boot has no offered mode; any choice waits for restart.
        let optimus = GraphicsRecord::after_write(None, GraphicsMode::Hybrid, 2, 1_100, boot);
        assert_eq!(optimus.boot_mode, None);
        assert!(optimus.restart_pending(boot));
    }
}
