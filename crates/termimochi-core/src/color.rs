use std::{error::Error, fmt, str::FromStr};

/// An sRGB color with eight-bit channels.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl Rgb {
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    #[must_use]
    pub const fn red(self) -> u8 {
        self.red
    }

    #[must_use]
    pub const fn green(self) -> u8 {
        self.green
    }

    #[must_use]
    pub const fn blue(self) -> u8 {
        self.blue
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.red, self.green, self.blue)
    }

    /// WCAG relative luminance for this sRGB color.
    #[must_use]
    pub fn relative_luminance(self) -> f64 {
        fn linearize(channel: u8) -> f64 {
            let value = f64::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * linearize(self.red)
            + 0.7152 * linearize(self.green)
            + 0.0722 * linearize(self.blue)
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl FromStr for Rgb {
    type Err = ColorParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        let digits = value.strip_prefix('#').unwrap_or(value);
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ColorParseError(value.to_owned()));
        }

        let channel = |range| {
            u8::from_str_radix(&digits[range], 16).map_err(|_| ColorParseError(value.to_owned()))
        };

        Ok(Self::new(channel(0..2)?, channel(2..4)?, channel(4..6)?))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColorParseError(String);

impl fmt::Display for ColorParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid RGB hex color: {:?}", self.0)
    }
}

impl Error for ColorParseError {}

/// WCAG contrast ratio in the inclusive range 1.0 to 21.0.
#[must_use]
pub fn contrast_ratio(first: Rgb, second: Rgb) -> f64 {
    let first = first.relative_luminance();
    let second = second.relative_luminance();
    let lighter = first.max(second);
    let darker = first.min(second);
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!("#e7e8e5".parse::<Rgb>().unwrap().to_hex(), "#E7E8E5");
        assert_eq!("252B33".parse::<Rgb>().unwrap().to_hex(), "#252B33");
    }

    #[test]
    fn invalid_hex_is_rejected() {
        for value in ["tomato", "#fff", "#1234567", "#GG0000"] {
            assert!(value.parse::<Rgb>().is_err(), "accepted {value}");
        }
    }

    #[test]
    fn wcag_endpoints() {
        assert!((contrast_ratio(Rgb::new(0, 0, 0), Rgb::new(255, 255, 255)) - 21.0).abs() < 1e-6);
        assert!((contrast_ratio(Rgb::new(25, 50, 75), Rgb::new(25, 50, 75)) - 1.0).abs() < 1e-6);
    }
}
