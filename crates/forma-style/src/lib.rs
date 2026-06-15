//! Design tokens and themes for Forma's self-drawn widgets.
//!
//! Because every control is drawn by Forma (not the OS), a single [`Theme`]
//! value defines the entire look: a semantic [`Palette`], a [`Spacing`] scale,
//! and corner [`radius`](Theme::radius). Widgets read tokens from the theme
//! rather than hard-coding colors, so re-skinning is a matter of swapping the
//! theme.

#![forbid(unsafe_code)]

use forma_render::Color;

/// Semantic colors. Names describe *role*, not hue, so light/dark/custom
/// themes can supply different values for the same role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Window/background fill.
    pub background: Color,
    /// Raised surface (cards, panels).
    pub surface: Color,
    /// Primary accent (default button, selection).
    pub primary: Color,
    /// Content drawn on top of `primary`.
    pub on_primary: Color,
    /// Default text/icon color on `background`/`surface`.
    pub text: Color,
    /// De-emphasized text.
    pub text_muted: Color,
    /// Hairline borders and dividers.
    pub border: Color,
}

/// A spacing scale in logical pixels. A small fixed scale keeps layouts
/// rhythmically consistent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    pub xs: f64,
    pub sm: f64,
    pub md: f64,
    pub lg: f64,
    pub xl: f64,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 12.0,
            lg: 16.0,
            xl: 24.0,
        }
    }
}

/// A complete look: palette + spacing + default corner radius + base font size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    pub spacing: Spacing,
    /// Default corner radius for rounded surfaces, in logical pixels.
    pub radius: f64,
    /// Base font size in logical pixels.
    pub font_size: f64,
}

impl Theme {
    /// The default light theme.
    pub fn light() -> Self {
        Self {
            palette: Palette {
                background: Color::rgb(0xf7, 0xf7, 0xf8),
                surface: Color::WHITE,
                primary: Color::rgb(0x3b, 0x82, 0xf6),
                on_primary: Color::WHITE,
                text: Color::rgb(0x1a, 0x1a, 0x1e),
                text_muted: Color::rgb(0x6b, 0x70, 0x80),
                border: Color::rgb(0xe2, 0xe4, 0xe9),
            },
            spacing: Spacing::default(),
            radius: 8.0,
            font_size: 14.0,
        }
    }

    /// The default dark theme.
    pub fn dark() -> Self {
        Self {
            palette: Palette {
                background: Color::rgb(0x14, 0x15, 0x18),
                surface: Color::rgb(0x1e, 0x20, 0x25),
                primary: Color::rgb(0x60, 0x9c, 0xff),
                on_primary: Color::rgb(0x0a, 0x0b, 0x0d),
                text: Color::rgb(0xec, 0xee, 0xf2),
                text_muted: Color::rgb(0x9a, 0xa0, 0xae),
                border: Color::rgb(0x2c, 0x2f, 0x36),
            },
            spacing: Spacing::default(),
            radius: 8.0,
            font_size: 14.0,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_differ_in_palette_not_metrics() {
        let (l, d) = (Theme::light(), Theme::dark());
        assert_ne!(l.palette.background, d.palette.background);
        assert_eq!(l.spacing, d.spacing);
        assert_eq!(l.radius, d.radius);
    }
}
