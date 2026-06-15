use oxideav_core::Rgba;

/// A straight (non-premultiplied) 8-bit-per-channel sRGB color.
///
/// Matches `oxideav-core`'s `Rgba` model; [`Color::to_oxideav`] is a
/// zero-cost bridge used when building a scene.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    #[inline]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parse `#RGB`, `#RGBA`, `#RRGGBB`, or `#RRGGBBAA` (leading `#`
    /// optional). Returns `None` on any malformed input.
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.strip_prefix('#').unwrap_or(s);
        let h = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
        let n = |i: usize| u8::from_str_radix(&s[i..i + 1], 16).ok().map(|v| v * 17);
        match s.len() {
            3 => Some(Self::rgb(n(0)?, n(1)?, n(2)?)),
            4 => Some(Self::rgba(n(0)?, n(1)?, n(2)?, n(3)?)),
            6 => Some(Self::rgb(h(0)?, h(2)?, h(4)?)),
            8 => Some(Self::rgba(h(0)?, h(2)?, h(4)?, h(6)?)),
            _ => None,
        }
    }

    /// Returns a copy with the alpha channel replaced.
    #[inline]
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Bridge to the `oxideav-core` color used by the scene graph.
    #[inline]
    pub const fn to_oxideav(self) -> Rgba {
        Rgba {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }
}

impl From<Color> for Rgba {
    #[inline]
    fn from(c: Color) -> Rgba {
        c.to_oxideav()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        assert_eq!(Color::from_hex("#ff0000"), Some(Color::rgb(255, 0, 0)));
        assert_eq!(Color::from_hex("0f0"), Some(Color::rgb(0, 255, 0)));
        assert_eq!(
            Color::from_hex("#11223344"),
            Some(Color::rgba(0x11, 0x22, 0x33, 0x44))
        );
        assert_eq!(Color::from_hex("#abc"), Some(Color::rgb(0xaa, 0xbb, 0xcc)));
        assert_eq!(Color::from_hex("zzz"), None);
        assert_eq!(Color::from_hex("#12345"), None);
    }
}
