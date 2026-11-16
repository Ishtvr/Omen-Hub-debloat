//! Opaque sRGB/HSV conversion for local color editing, independent of hardware.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
}
impl Hsv {
    pub fn new(hue: f32, saturation: f32, value: f32) -> Option<Self> {
        (hue.is_finite() && saturation.is_finite() && value.is_finite()).then(|| Self {
            hue: hue.rem_euclid(360.0),
            saturation: saturation.clamp(0.0, 1.0),
            value: value.clamp(0.0, 1.0),
        })
    }
    pub fn from_rgb(rgb: [u8; 3]) -> Self {
        let [r, g, b] = rgb.map(|v| f32::from(v) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let c = max - min;
        let hue = if c == 0.0 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / c).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / c + 2.0)
        } else {
            60.0 * ((r - g) / c + 4.0)
        };
        Self {
            hue,
            saturation: if max == 0.0 { 0.0 } else { c / max },
            value: max,
        }
    }
    pub fn rgb(self) -> [u8; 3] {
        let h = self.hue / 60.0;
        let c = self.value * self.saturation;
        let x = c * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
        let m = self.value - c;
        let rgb = match h as u32 {
            0 => [c, x, 0.0],
            1 => [x, c, 0.0],
            2 => [0.0, c, x],
            3 => [0.0, x, c],
            4 => [x, 0.0, c],
            _ => [c, 0.0, x],
        };
        rgb.map(|v| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8)
    }
}
pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}
pub const SWATCHES: [[u8; 3]; 16] = [
    [255, 255, 255],
    [171, 179, 194],
    [58, 63, 75],
    [0, 0, 0],
    [255, 92, 115],
    [255, 147, 72],
    [255, 210, 92],
    [182, 221, 84],
    [78, 208, 154],
    [69, 207, 216],
    [83, 165, 246],
    [124, 135, 248],
    [177, 126, 239],
    [230, 118, 205],
    [255, 175, 204],
    [255, 223, 189],
];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgb_roundtrip_retains_exact_channels_including_gray_and_extremes() {
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(17) {
                    let rgb = [r as u8, g as u8, b as u8];
                    assert_eq!(Hsv::from_rgb(rgb).rgb(), rgb);
                }
            }
        }
        for rgb in SWATCHES {
            assert_eq!(Hsv::from_rgb(rgb).rgb(), rgb);
        }
    }
    #[test]
    fn hue_wrap_and_saturation_value_edges_are_bounded() {
        assert_eq!(Hsv::new(360.0, 1.0, 1.0).unwrap().rgb(), [255, 0, 0]);
        assert_eq!(Hsv::new(-120.0, 1.0, 1.0).unwrap().rgb(), [0, 0, 255]);
        assert_eq!(Hsv::new(12.0, 0.0, 1.0).unwrap().rgb(), [255; 3]);
        assert_eq!(Hsv::new(100.0, 1.0, 0.0).unwrap().rgb(), [0; 3]);
        assert!(Hsv::new(f32::NAN, 1.0, 1.0).is_none());
        assert!(Hsv::new(0.0, f32::INFINITY, 1.0).is_none());
    }
}
