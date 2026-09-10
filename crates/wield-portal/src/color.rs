//! Portal colour conversion.

/// An 8-bit-per-channel colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Convert normalized portal channels, clamping and rounding each channel.
    pub fn from_unit(r: f64, g: f64, b: f64) -> Self {
        fn channel(value: f64) -> u8 {
            (value.clamp(0.0, 1.0) * 255.0).round() as u8
        }
        Self {
            r: channel(r),
            g: channel(g),
            b: channel(b),
        }
    }

    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn rgb_string(&self) -> String {
        format!("rgb({}, {}, {})", self.r, self.g, self.b)
    }

    pub fn hsl_string(&self) -> String {
        let red = f64::from(self.r) / 255.0;
        let green = f64::from(self.g) / 255.0;
        let blue = f64::from(self.b) / 255.0;
        let maximum = red.max(green).max(blue);
        let minimum = red.min(green).min(blue);
        let lightness = (maximum + minimum) / 2.0;
        let delta = maximum - minimum;
        let (hue, saturation) = if delta == 0.0 {
            (0.0, 0.0)
        } else {
            let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
            let hue = if maximum == red {
                60.0 * ((green - blue) / delta).rem_euclid(6.0)
            } else if maximum == green {
                60.0 * ((blue - red) / delta + 2.0)
            } else {
                60.0 * ((red - green) / delta + 4.0)
            };
            (hue, saturation)
        };
        format!(
            "hsl({}, {}%, {}%)",
            hue.round() as u16,
            (saturation * 100.0).round() as u8,
            (lightness * 100.0).round() as u8
        )
    }

    /// Serialize all supported colour representations into the value payload.
    pub fn value_payload(&self) -> String {
        serde_json::json!({
            "hex": self.hex(),
            "rgb": self.rgb_string(),
            "hsl": self.hsl_string(),
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_unit_channels_to_hex_rgb_hsl() {
        let color = Rgb::from_unit(0.0, 0.5019608, 1.0);
        assert_eq!(color, Rgb { r: 0, g: 128, b: 255 });
        assert_eq!(color.hex(), "#0080ff");
        assert_eq!(color.rgb_string(), "rgb(0, 128, 255)");
        assert_eq!(color.hsl_string(), "hsl(210, 100%, 50%)");
    }

    #[test]
    fn clamps_out_of_range_channels() {
        assert_eq!(
            Rgb::from_unit(-1.0, 2.0, 0.5),
            Rgb { r: 0, g: 255, b: 128 }
        );
    }

    #[test]
    fn value_payload_is_json_with_all_three() {
        let payload = Rgb { r: 255, g: 255, b: 255 }.value_payload();
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value["hex"], "#ffffff");
        assert!(value["rgb"].is_string() && value["hsl"].is_string());
    }
}
