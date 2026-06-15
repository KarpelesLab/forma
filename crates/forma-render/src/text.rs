//! Text: font loading and shaping, bridged to the scene graph via
//! `oxideav-scribe`.
//!
//! A [`Font`] wraps a scribe `FaceChain`. [`Scene::fill_text`](crate::Scene)
//! shapes a string into positioned glyph outlines and emits them as
//! `oxideav-core` nodes, so text composites through the same CPU rasterizer as
//! every other primitive — no separate text pipeline.
//!
//! Apps provide font bytes via [`Font::from_bytes`]; [`Font::system_default`]
//! is a convenience that probes common OS font locations (handy for examples
//! and tests, not meant for shipping apps).

use crate::Color;
use crate::scene::Scene;
use core::fmt;
use forma_geometry::{Point, Size};
use oxideav_core::{Group, Node, Paint, Transform2D};
use oxideav_scribe::{Face, FaceChain, Shaper};

/// A loaded, shapeable font face.
pub struct Font {
    chain: FaceChain,
}

impl fmt::Debug for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Font")
            .field("units_per_em", &self.chain.primary().units_per_em())
            .finish()
    }
}

/// Error loading a [`Font`].
#[derive(Debug)]
pub struct FontError(String);

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "font load error: {}", self.0)
    }
}

impl std::error::Error for FontError {}

impl Font {
    /// Load a font from `sfnt` bytes (TrueType, OpenType/CFF, or TrueType
    /// Collection — the first face of a collection is used).
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, FontError> {
        let face = parse_face(bytes)?;
        Ok(Self {
            chain: FaceChain::new(face),
        })
    }

    /// Probe common operating-system font directories and load the first
    /// usable sans-serif face. Returns `None` if none is found.
    ///
    /// Intended for examples and tests; shipping apps should bundle or
    /// explicitly locate their fonts and use [`Font::from_bytes`].
    pub fn system_default() -> Option<Self> {
        const CANDIDATES: &[&str] = &[
            // Linux (Liberation / DejaVu / Noto are near-ubiquitous).
            "/usr/share/fonts/liberation-fonts/LiberationSans-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            // macOS.
            "/System/Library/Fonts/Helvetica.ttc",
            "/Library/Fonts/Arial.ttf",
            // Windows.
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
        ];
        for path in CANDIDATES {
            if let Ok(bytes) = std::fs::read(path)
                && let Ok(font) = Font::from_bytes(bytes)
            {
                return Some(font);
            }
        }
        None
    }

    /// Distance from the top of the text box to the baseline, in logical
    /// pixels, at `size_px`.
    pub fn ascent(&self, size_px: f64) -> f64 {
        self.chain.primary().ascent_px(size_px as f32) as f64
    }

    /// Full line height (ascent + descent + line gap) at `size_px`.
    pub fn line_height(&self, size_px: f64) -> f64 {
        self.chain.primary().line_height_px(size_px as f32) as f64
    }

    /// Measure the rendered size of `text` on a single line at `size_px`:
    /// summed advance width × line height.
    pub fn measure(&self, text: &str, size_px: f64) -> Size {
        let width: f32 = match self.chain.shape(text, size_px as f32) {
            Ok(glyphs) => glyphs.iter().map(|g| g.x_advance).sum(),
            Err(_) => 0.0,
        };
        Size::new(width as f64, self.line_height(size_px))
    }

    pub(crate) fn chain(&self) -> &FaceChain {
        &self.chain
    }
}

fn parse_face(bytes: Vec<u8>) -> Result<Face, FontError> {
    let result = match bytes.first_chunk::<4>() {
        Some(b"OTTO") => Face::from_otf_bytes(bytes),
        Some(b"ttcf") => Face::from_ttc_bytes(bytes, 0),
        _ => Face::from_ttf_bytes(bytes),
    };
    result.map_err(|e| FontError(format!("{e:?}")))
}

/// Recolor an outline glyph to `color`, recursing into groups (the shaper
/// wraps each glyph's path in a cache-keyed `Group`). Non-outline leaves (e.g.
/// color-bitmap emoji `Node::Image`) are left untouched.
fn recolor(node: Node, color: Color) -> Node {
    match node {
        Node::Path(mut path) => {
            path.fill = Some(Paint::Solid(color.to_oxideav()));
            Node::Path(path)
        }
        Node::Group(mut group) => {
            group.children = group
                .children
                .into_iter()
                .map(|c| recolor(c, color))
                .collect();
            Node::Group(group)
        }
        other => other,
    }
}

impl Scene {
    /// Shape and paint `text` with `font` at `origin` (the top-left of the text
    /// box, logical pixels), `size_px`, and `color`.
    ///
    /// Glyphs are emitted as scene-graph nodes under a group translated to the
    /// baseline, so they rasterize and composite like any other primitive.
    pub fn fill_text(
        &mut self,
        font: &Font,
        text: &str,
        origin: Point,
        size_px: f64,
        color: Color,
    ) {
        if text.is_empty() || size_px <= 0.0 {
            return;
        }
        let glyphs = Shaper::shape_to_paths(font.chain(), text, size_px as f32);
        if glyphs.is_empty() {
            return;
        }

        let mut run = Vec::with_capacity(glyphs.len());
        for (_face_idx, node, transform) in glyphs {
            let glyph = Group::new()
                .with_transform(transform)
                .with_child(recolor(node, color));
            run.push(Node::Group(glyph));
        }

        // Place the whole run: pen starts at origin.x, baseline drops by the
        // ascent so the text box top aligns to origin.y.
        let baseline = (origin.y + font.ascent(size_px)) as f32;
        let placed = Group::new()
            .with_transform(Transform2D::translate(origin.x as f32, baseline))
            .with_children(run);
        self.push_node(Node::Group(placed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SoftwareRenderer;
    use forma_geometry::{Rect, ScaleFactor};

    #[test]
    fn measure_is_monotonic_in_length() {
        let Some(font) = Font::system_default() else {
            eprintln!("skipping: no system font found");
            return;
        };
        let short = font.measure("i", 16.0);
        let long = font.measure("internationalization", 16.0);
        assert!(long.width > short.width);
        assert!(short.height > 0.0);
    }

    #[test]
    fn text_paints_visible_pixels() {
        let Some(font) = Font::system_default() else {
            eprintln!("skipping: no system font found");
            return;
        };
        let mut scene = Scene::new(Size::new(200.0, 50.0));
        // White background so black text stands out.
        scene.fill_rect(Rect::from_xywh(0.0, 0.0, 200.0, 50.0), Color::WHITE);
        scene.fill_text(&font, "Hello", Point::new(8.0, 8.0), 28.0, Color::BLACK);

        let pm = SoftwareRenderer::new().render(scene, ScaleFactor::IDENTITY);
        // Some pixel must be darkened by a glyph (not pure white).
        let mut darkened = 0;
        for y in 0..pm.size().height {
            for x in 0..pm.size().width {
                if let Some([r, _, _, _]) = pm.pixel(x, y)
                    && r < 128
                {
                    darkened += 1;
                }
            }
        }
        assert!(
            darkened > 20,
            "expected glyph coverage, got {darkened} dark pixels"
        );
    }

    #[test]
    fn text_uses_requested_color() {
        let Some(font) = Font::system_default() else {
            eprintln!("skipping: no system font found");
            return;
        };
        let mut scene = Scene::new(Size::new(160.0, 50.0));
        scene.fill_rect(Rect::from_xywh(0.0, 0.0, 160.0, 50.0), Color::WHITE);
        // Pure red text: glyph interiors must be red, not the default black.
        scene.fill_text(
            &font,
            "RED",
            Point::new(8.0, 8.0),
            32.0,
            Color::rgb(255, 0, 0),
        );

        let pm = SoftwareRenderer::new().render(scene, ScaleFactor::IDENTITY);
        let mut reddish = 0;
        for y in 0..pm.size().height {
            for x in 0..pm.size().width {
                if let Some([r, g, b, _]) = pm.pixel(x, y)
                    && r > 180
                    && g < 80
                    && b < 80
                {
                    reddish += 1;
                }
            }
        }
        assert!(
            reddish > 20,
            "expected red glyph coverage, got {reddish} red pixels"
        );
    }
}
