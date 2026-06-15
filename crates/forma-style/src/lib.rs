//! Design tokens and themes for Forma's self-drawn widgets.
//!
//! Because every control is drawn by Forma (not the OS), a single [`Theme`]
//! value defines the entire look: a semantic [`Palette`] (roles + interaction
//! states + status colors + overlays), a [`Typography`] scale, a [`Spacing`]
//! scale, and corner [`radius`](Theme::radius). Widgets read tokens from the
//! theme rather than hard-coding values, so re-skinning is swapping the theme —
//! and the [builder methods](Theme::with_accent) make a custom theme a
//! one-liner.

#![forbid(unsafe_code)]

use forma_render::Color;

/// Semantic colors. Names describe *role*, not hue, so light/dark/custom themes
/// can supply different values for the same role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Window/background fill.
    pub background: Color,
    /// Raised surface (cards, panels).
    pub surface: Color,
    /// Primary accent (default button, selection).
    pub primary: Color,
    /// Primary under hover.
    pub primary_hover: Color,
    /// Primary while pressed/active.
    pub primary_active: Color,
    /// Content drawn on top of `primary`.
    pub on_primary: Color,
    /// Default text/icon color on `background`/`surface`.
    pub text: Color,
    /// De-emphasized text.
    pub text_muted: Color,
    /// Hairline borders and dividers.
    pub border: Color,
    /// Positive / success status.
    pub success: Color,
    /// Caution / warning status.
    pub warning: Color,
    /// Destructive / error status.
    pub danger: Color,
    /// Focus-ring color (the App draws it around the focused element).
    pub focus_ring: Color,
    /// Translucent overlay the App composites on a hovered element.
    pub hover_overlay: Color,
}

/// A type scale, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typography {
    pub caption: f64,
    pub body: f64,
    pub title: f64,
    pub heading: f64,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            caption: 12.0,
            body: 14.0,
            title: 18.0,
            heading: 24.0,
        }
    }
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

/// A complete look: palette + typography + spacing + corner radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    pub typography: Typography,
    pub spacing: Spacing,
    /// Default corner radius for rounded surfaces, in logical pixels.
    pub radius: f64,
    /// Base font size in logical pixels (mirrors `typography.body`).
    pub font_size: f64,
}

/// Derive the hover/active/ring tints from a `primary` accent.
fn states(primary: Color, dark: bool) -> (Color, Color) {
    if dark {
        (primary.lighten(0.12), primary.darken(0.12))
    } else {
        (primary.darken(0.08), primary.darken(0.18))
    }
}

impl Theme {
    /// The default light theme.
    pub fn light() -> Self {
        let primary = Color::rgb(0x3b, 0x82, 0xf6);
        let (primary_hover, primary_active) = states(primary, false);
        Self {
            palette: Palette {
                background: Color::rgb(0xf7, 0xf7, 0xf8),
                surface: Color::WHITE,
                primary,
                primary_hover,
                primary_active,
                on_primary: Color::WHITE,
                text: Color::rgb(0x1a, 0x1a, 0x1e),
                text_muted: Color::rgb(0x6b, 0x70, 0x80),
                border: Color::rgb(0xe2, 0xe4, 0xe9),
                success: Color::rgb(0x22, 0xa5, 0x5a),
                warning: Color::rgb(0xe0, 0x8a, 0x00),
                danger: Color::rgb(0xdc, 0x3a, 0x3a),
                focus_ring: primary,
                hover_overlay: Color::rgba(0, 0, 0, 18),
            },
            typography: Typography::default(),
            spacing: Spacing::default(),
            radius: 8.0,
            font_size: 14.0,
        }
    }

    /// The default dark theme.
    pub fn dark() -> Self {
        let primary = Color::rgb(0x60, 0x9c, 0xff);
        let (primary_hover, primary_active) = states(primary, true);
        Self {
            palette: Palette {
                background: Color::rgb(0x14, 0x15, 0x18),
                surface: Color::rgb(0x1e, 0x20, 0x25),
                primary,
                primary_hover,
                primary_active,
                on_primary: Color::rgb(0x0a, 0x0b, 0x0d),
                text: Color::rgb(0xec, 0xee, 0xf2),
                text_muted: Color::rgb(0x9a, 0xa0, 0xae),
                border: Color::rgb(0x2c, 0x2f, 0x36),
                success: Color::rgb(0x35, 0xc7, 0x76),
                warning: Color::rgb(0xf5, 0x9e, 0x0b),
                danger: Color::rgb(0xf2, 0x55, 0x55),
                focus_ring: primary,
                hover_overlay: Color::rgba(255, 255, 255, 28),
            },
            typography: Typography::default(),
            spacing: Spacing::default(),
            radius: 8.0,
            font_size: 14.0,
        }
    }

    /// Recolor the theme around a new `accent`: sets `primary`, derives its
    /// hover/active tints + focus ring, and picks a readable `on_primary`.
    pub fn with_accent(mut self, accent: Color) -> Self {
        let dark = self.palette.background.luminance() < 0.5;
        let (hover, active) = states(accent, dark);
        self.palette.primary = accent;
        self.palette.primary_hover = hover;
        self.palette.primary_active = active;
        self.palette.on_primary = accent.on_color();
        self.palette.focus_ring = accent;
        self
    }

    /// Set the default corner radius.
    pub fn with_radius(mut self, radius: f64) -> Self {
        self.radius = radius;
        self
    }

    /// Set the base font size (also updates `typography.body`).
    pub fn with_font_size(mut self, size: f64) -> Self {
        self.font_size = size;
        self.typography.body = size;
        self
    }

    /// A higher-contrast variant: pure black/white text on a near-pure
    /// background, square-ish corners, and bolder borders. Useful for
    /// accessibility-leaning skins.
    pub fn high_contrast(mut self) -> Self {
        let dark = self.palette.background.luminance() < 0.5;
        if dark {
            self.palette.background = Color::BLACK;
            self.palette.surface = Color::rgb(0x10, 0x10, 0x10);
            self.palette.text = Color::WHITE;
            self.palette.border = Color::rgb(0xc0, 0xc0, 0xc0);
        } else {
            self.palette.background = Color::WHITE;
            self.palette.surface = Color::WHITE;
            self.palette.text = Color::BLACK;
            self.palette.border = Color::BLACK;
        }
        self.palette.text_muted = self.palette.text;
        self.radius = 2.0;
        self
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

    #[test]
    fn with_accent_recolors_and_picks_on_color() {
        let t = Theme::light().with_accent(Color::rgb(0xff, 0xd0, 0x00)); // bright yellow
        assert_eq!(t.palette.primary, Color::rgb(0xff, 0xd0, 0x00));
        assert_eq!(t.palette.focus_ring, t.palette.primary);
        // On a bright accent, content should be dark.
        assert_eq!(t.palette.on_primary, Color::BLACK);
        // Hover/active are derived (different from the base).
        assert_ne!(t.palette.primary_hover, t.palette.primary);
    }

    #[test]
    fn high_contrast_maximizes_text_contrast() {
        let t = Theme::light().high_contrast();
        assert_eq!(t.palette.text, Color::BLACK);
        assert_eq!(t.palette.background, Color::WHITE);
    }
}
