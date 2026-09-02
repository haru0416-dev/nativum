//! sRGB colors stored as unpremultiplied RGBA.

/// 8-bit per channel color. Alpha is unpremultiplied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha.
    pub a: u8,
}

impl Color {
    /// Fully transparent.
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// Opaque constructor.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Parse `#rgb`, `#rrggbb`, or `#rrggbbaa`. Returns `None` on failure.
    pub fn parse(spec: &str) -> Option<Self> {
        let s = spec.strip_prefix('#').unwrap_or(spec);
        match s.len() {
            3 => {
                let n = u16::from_str_radix(s, 16).ok()?;
                let r = (((n >> 8) & 0xF) * 0x11) as u8;
                let g = (((n >> 4) & 0xF) * 0x11) as u8;
                let b = ((n & 0xF) * 0x11) as u8;
                Some(Self::rgb(r, g, b))
            }
            6 => {
                let n = u32::from_str_radix(s, 16).ok()?;
                Some(Self::rgb(
                    ((n >> 16) & 0xFF) as u8,
                    ((n >> 8) & 0xFF) as u8,
                    (n & 0xFF) as u8,
                ))
            }
            8 => {
                let n = u32::from_str_radix(s, 16).ok()?;
                Some(Self {
                    r: ((n >> 24) & 0xFF) as u8,
                    g: ((n >> 16) & 0xFF) as u8,
                    b: ((n >> 8) & 0xFF) as u8,
                    a: (n & 0xFF) as u8,
                })
            }
            _ => None,
        }
    }

    /// Premultiply by `coverage` in 0..=1 and blend over `dst` (src-over).
    pub fn blend_over(self, dst: Self, coverage: f32) -> Self {
        let a = (f32::from(self.a) / 255.0) * coverage.clamp(0.0, 1.0);
        if a <= 0.0 {
            return dst;
        }
        if a >= 1.0 {
            return Self {
                r: self.r,
                g: self.g,
                b: self.b,
                a: 255,
            };
        }
        let ia = 1.0 - a;
        let mix = |s: u8, d: u8| -> u8 { (a * f32::from(s) + ia * f32::from(d)).round() as u8 };
        Self {
            r: mix(self.r, dst.r),
            g: mix(self.g, dst.g),
            b: mix(self.b, dst.b),
            a: 255,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex() {
        assert_eq!(Color::parse("#abc"), Some(Color::rgb(0xAA, 0xBB, 0xCC)));
        assert_eq!(Color::parse("#112233"), Some(Color::rgb(0x11, 0x22, 0x33)));
    }
}
