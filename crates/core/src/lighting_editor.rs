//! Four independently selectable zones over the investigated RGB contract. Brightness
//! is a color-value adjustment, never a guessed firmware brightness/mode byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Wasd,
    Left,
    Middle,
    NumpadRight,
}
impl Group {
    pub const ALL: [Self; 4] = [Self::Wasd, Self::Left, Self::Middle, Self::NumpadRight];
    pub fn zones(self) -> &'static [usize] {
        // FourZoneModule 1101.2609.3.0: 0=KBRight, 1=KBMid,
        // 2=KBLeft, 3=KBWasd. Capability [1,0,0,0] includes the numpad.
        match self {
            Self::Wasd => &[3],
            Self::Left => &[2],
            Self::Middle => &[1],
            Self::NumpadRight => &[0],
        }
    }
    pub fn for_zone(zone: usize) -> Self {
        match zone {
            3 => Self::Wasd,
            0 => Self::NumpadRight,
            1 => Self::Middle,
            _ => Self::Left,
        }
    }
}
fn base(color: [u8; 3]) -> [u8; 3] {
    let max = *color.iter().max().unwrap();
    if max == 0 {
        return [255; 3];
    }
    color.map(|c| ((u32::from(c) * 255 + u32::from(max) / 2) / u32::from(max)) as u8)
}
fn level(color: [u8; 3]) -> u8 {
    ((u32::from(*color.iter().max().unwrap()) * 100 + 127) / 255) as u8
}
#[derive(Debug, Clone)]
pub struct Draft {
    original: [[u8; 3]; 4],
    original_enabled: bool,
    pub colors: [[u8; 3]; 4],
    bases: [[u8; 3]; 4],
    pub enabled: bool,
    pub group: Group,
}
impl Draft {
    pub fn new(colors: [[u8; 3]; 4], enabled: bool) -> Self {
        Self {
            original: colors,
            original_enabled: enabled,
            colors,
            bases: colors.map(base),
            enabled,
            group: Group::Wasd,
        }
    }
    pub fn color(&self) -> Option<[u8; 3]> {
        let value = self.colors[self.group.zones()[0]];
        self.group
            .zones()
            .iter()
            .all(|i| self.colors[*i] == value)
            .then_some(value)
    }
    pub fn brightness(&self) -> Option<u8> {
        let value = level(self.colors[self.group.zones()[0]]);
        self.group
            .zones()
            .iter()
            .all(|i| level(self.colors[*i]) == value)
            .then_some(value)
    }
    pub fn set_color(&mut self, color: [u8; 3]) {
        for i in self.group.zones() {
            self.colors[*i] = color;
            self.bases[*i] = base(color);
        }
    }
    pub fn set_brightness(&mut self, percent: u8) -> Result<(), String> {
        if percent > 100 {
            return Err(crate::message::msg("lighting.invalid_brightness", &[]));
        }
        for i in self.group.zones() {
            self.colors[*i] =
                self.bases[*i].map(|c| ((u32::from(c) * u32::from(percent) + 50) / 100) as u8);
        }
        Ok(())
    }
    pub fn dirty(&self) -> bool {
        self.colors != self.original || self.enabled != self.original_enabled
    }
    pub fn reset(&mut self) {
        self.colors = self.original;
        self.bases = self.colors.map(base);
        self.enabled = self.original_enabled;
    }
    pub fn accept(&mut self) {
        self.original = self.colors;
        self.original_enabled = self.enabled;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn group_edits_follow_vendor_zone_order_and_preserve_other_zones() {
        let colors = [[1, 2, 3], [4, 5, 6], [7, 8, 9], [10, 11, 12]];
        let mut d = Draft::new(colors, true);
        d.set_color([255, 0, 0]);
        assert_eq!(d.colors, [[1, 2, 3], [4, 5, 6], [7, 8, 9], [255, 0, 0]]);
        d.group = Group::Left;
        assert_eq!(d.color(), Some([7, 8, 9]));
        d.set_color([0, 255, 0]);
        assert_eq!(d.colors, [[1, 2, 3], [4, 5, 6], [0, 255, 0], [255, 0, 0]]);
        d.group = Group::Middle;
        d.set_color([40, 50, 60]);
        assert_eq!(
            d.colors,
            [[1, 2, 3], [40, 50, 60], [0, 255, 0], [255, 0, 0]]
        );
        d.group = Group::NumpadRight;
        d.set_color([0, 0, 255]);
        assert_eq!(d.colors[0], [0, 0, 255]);
        d.reset();
        assert_eq!(d.colors, colors);
        assert!(!d.dirty());
    }
    #[test]
    fn dimming_never_compounds_rounding_or_changes_another_group() {
        let colors = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 128, 64]];
        let mut d = Draft::new(colors, false);
        d.set_brightness(50).unwrap();
        assert_eq!(d.colors[3], [128, 64, 32]);
        d.set_brightness(20).unwrap();
        assert_eq!(d.colors[3], [51, 26, 13]);
        d.set_brightness(0).unwrap();
        assert_eq!(d.colors[3], [0, 0, 0]);
        d.set_brightness(100).unwrap();
        assert_eq!(d.colors, colors);
        assert!(!d.enabled);
        assert!(!d.dirty());
        assert!(d.set_brightness(101).is_err());
        assert_eq!(d.colors, colors);
    }
    #[test]
    fn middle_brightness_preserves_left_and_other_zones() {
        let colors = [[1, 2, 3], [0, 255, 0], [0, 0, 255], [4, 5, 6]];
        let mut d = Draft::new(colors, true);
        d.group = Group::Middle;
        assert_eq!(d.color(), Some([0, 255, 0]));
        d.set_brightness(40).unwrap();
        assert_eq!(d.colors, [[1, 2, 3], [0, 102, 0], [0, 0, 255], [4, 5, 6]]);
        assert_eq!(d.brightness(), Some(40));
        assert_eq!(d.color(), Some([0, 102, 0]));
    }
}
