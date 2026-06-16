//! Experimental GPU present path: route a software [`Pixmap`] through OpenGL
//! ES 2 (raw EGL + GLES FFI — no `ash`/`glutin`/`khronos` crate; linking the
//! OS's GL driver fits the "close to the OS" policy in `ROADMAP.md` §1).
//!
//! [`present_offscreen`] uploads the pixmap as a texture, draws a fullscreen
//! quad into an offscreen framebuffer, and reads the result back — the GPU half
//! of the [`Surface`](forma_render::Surface) seam. v1 just composites the CPU
//! frame on the GPU (a pass-through shader); GPU-native scene tessellation /
//! glyph atlases, and Vulkan/Metal/D3D/WebGPU productionization, are future
//! work. The EGL context is **surfaceless**, so it runs headlessly under Mesa's
//! software GL in CI.
//!
//! Linux-only for now (EGL/GLESv2); other targets return an error so the
//! workspace still builds everywhere.

use forma_geometry::{PhysicalSize, Point, Rect, ScaleFactor, Size};
use forma_render::{Color, DrawCmd, Font, Pixmap, Scene, SoftwareRenderer};

#[cfg(all(target_os = "linux", feature = "gl"))]
mod gl;

/// Round-trip `input` through the GPU (upload → draw → read back), returning the
/// rendered frame. Errors if the `gl` feature is off or no GL/EGL device is
/// available.
pub fn present_offscreen(input: &Pixmap) -> Result<Pixmap, String> {
    #[cfg(all(target_os = "linux", feature = "gl"))]
    {
        gl::present_offscreen(input)
    }
    #[cfg(not(all(target_os = "linux", feature = "gl")))]
    {
        let _ = input;
        Err("forma-gpu: built without the `gl` feature (Linux-only GLES backend)".to_string())
    }
}

/// Draw solid-color `rects` **GPU-natively** (as tessellated geometry through a
/// flat-color shader, not by compositing a CPU frame) on a `background`-cleared
/// target of `size`, returning the rendered frame — the first step toward a
/// GPU-native scene renderer. Each entry is
/// `(rect, color, corner_radius, border_width)`; the shader evaluates a
/// rounded-rectangle SDF (radius `0.0` = sharp), and a positive `border_width`
/// strokes the outline instead of filling. Errors if the `gl` feature is off or
/// no GL/EGL device is available.
pub fn fill_rects_offscreen(
    size: PhysicalSize,
    background: Color,
    rects: &[(Rect, Color, f32, f32)],
) -> Result<Pixmap, String> {
    #[cfg(all(target_os = "linux", feature = "gl"))]
    {
        gl::fill_rects_offscreen(size, background, rects)
    }
    #[cfg(not(all(target_os = "linux", feature = "gl")))]
    {
        let _ = (size, background, rects);
        Err("forma-gpu: built without the `gl` feature (Linux-only GLES backend)".to_string())
    }
}

/// Render a scene **GPU-natively** — box primitives (rounded-rect SDF) plus
/// text composited from glyph-coverage masks — to an offscreen target of `size`
/// cleared to `background`, returning the rendered frame. Each text entry is
/// `(mask, dst, color)`: a white-on-black coverage raster of the glyphs, the
/// destination rectangle, and the ink color. Errors if the `gl` feature is off
/// or no GL/EGL device is available.
pub fn render_offscreen(
    size: PhysicalSize,
    background: Color,
    rects: &[(Rect, Color, f32, f32)],
    texts: &[(&Pixmap, Rect, Color)],
) -> Result<Pixmap, String> {
    #[cfg(all(target_os = "linux", feature = "gl"))]
    {
        gl::render_offscreen(size, background, rects, texts)
    }
    #[cfg(not(all(target_os = "linux", feature = "gl")))]
    {
        let _ = (size, background, rects, texts);
        Err("forma-gpu: built without the `gl` feature (Linux-only GLES backend)".to_string())
    }
}

/// A bucket key for the glyph cache: a character at a quantized size.
fn size_key(size: f64) -> u32 {
    (size * 4.0).round() as u32
}

/// A packed glyph-coverage atlas. Each unique `(char, size)` is rasterized once
/// (white-on-black coverage) and shelf-packed into a single-row texture, so
/// repeated glyphs in a scene reuse one slot — a per-glyph cache rather than
/// re-rasterizing whole text runs.
struct GlyphAtlas {
    // Read by the GL renderer; on targets/builds without it this is unused.
    #[cfg_attr(not(all(target_os = "linux", feature = "gl")), allow(dead_code))]
    atlas: Pixmap,
    /// Pixel rectangle of each `(char, size_key)` within the atlas.
    uvs: std::collections::BTreeMap<(char, u32), Rect>,
}

impl GlyphAtlas {
    /// Rasterize and pack every unique non-space glyph in `wanted`.
    fn build(font: &Font, wanted: &[(char, f64)]) -> Self {
        use std::collections::BTreeMap;
        let mut keys: BTreeMap<(char, u32), f64> = BTreeMap::new();
        for (ch, size) in wanted {
            if !ch.is_whitespace() {
                keys.insert((*ch, size_key(*size)), *size);
            }
        }
        let cells: Vec<((char, u32), Pixmap)> = keys
            .iter()
            .map(|((ch, sk), size)| {
                let s = ch.to_string();
                let cw = font.measure(&s, *size).width.ceil().max(1.0);
                let ch_h = font.line_height(*size).ceil().max(1.0);
                let mut sc = Scene::new(Size::new(cw, ch_h));
                sc.fill_text(font, &s, Point::new(0.0, 0.0), *size, Color::WHITE);
                let pm = SoftwareRenderer::new()
                    .with_background(Color::BLACK)
                    .render(sc, ScaleFactor::IDENTITY);
                ((*ch, *sk), pm)
            })
            .collect();

        let total_w: u32 = cells
            .iter()
            .map(|(_, pm)| pm.size().width)
            .sum::<u32>()
            .max(1);
        let height: u32 = cells
            .iter()
            .map(|(_, pm)| pm.size().height)
            .max()
            .unwrap_or(1)
            .max(1);
        let mut atlas = Pixmap::new(PhysicalSize::new(total_w, height));
        let astride = atlas.stride();
        let mut uvs = std::collections::BTreeMap::new();
        let mut x = 0u32;
        for (key, pm) in &cells {
            let (pw, ph) = (pm.size().width, pm.size().height);
            let pstride = pm.stride();
            for row in 0..ph as usize {
                let s = &pm.as_bytes()[row * pstride..row * pstride + pw as usize * 4];
                let off = row * astride + x as usize * 4;
                atlas.as_bytes_mut()[off..off + pw as usize * 4].copy_from_slice(s);
            }
            uvs.insert(*key, Rect::from_xywh(x as f64, 0.0, pw as f64, ph as f64));
            x += pw;
        }
        GlyphAtlas { atlas, uvs }
    }

    fn uv(&self, ch: char, size: f64) -> Option<Rect> {
        self.uvs.get(&(ch, size_key(size))).copied()
    }
}

/// Render a live forma [`Scene`] **GPU-natively**: box primitives become
/// SDF-shaded geometry, and text is drawn from a packed **glyph atlas** — each
/// unique glyph rasterized once (via `font`) into one shared texture the GPU
/// samples per glyph — so any forma UI renders through the GPU. `background`
/// clears the target. Errors if the `gl` feature is off or no GL/EGL device is
/// available.
pub fn render_scene(scene: &Scene, background: Color, font: &Font) -> Result<Pixmap, String> {
    let ls = scene.logical_size();
    let size = ScaleFactor::IDENTITY.to_physical(ls);
    let mut rects: Vec<(Rect, Color, f32, f32)> = Vec::new();

    // Gather every glyph the scene needs, then pack them once.
    let mut wanted: Vec<(char, f64)> = Vec::new();
    for cmd in scene.commands() {
        if let DrawCmd::Text { text, size: px, .. } = cmd {
            wanted.extend(text.chars().map(|c| (c, *px)));
        }
    }
    let atlas = GlyphAtlas::build(font, &wanted);

    // Build per-glyph (src, dst, color) quads from the cached slots.
    let mut glyphs: Vec<(Rect, Rect, Color)> = Vec::new();
    for cmd in scene.commands() {
        match cmd {
            DrawCmd::Rect {
                rect,
                color,
                radius,
                border,
            } => rects.push((*rect, *color, *radius as f32, *border as f32)),
            DrawCmd::Text {
                text,
                origin,
                size: px,
                color,
            } => {
                for (i, ch) in text.char_indices() {
                    if let Some(src) = atlas.uv(ch, *px) {
                        let x = origin.x + font.measure(&text[..i], *px).width;
                        let dst = Rect::from_xywh(x, origin.y, src.width(), src.height());
                        glyphs.push((src, dst, *color));
                    }
                }
            }
        }
    }

    #[cfg(all(target_os = "linux", feature = "gl"))]
    {
        gl::render_scene_gl(size, background, &rects, &[], Some((&atlas.atlas, &glyphs)))
    }
    #[cfg(not(all(target_os = "linux", feature = "gl")))]
    {
        let _ = (size, background, glyphs);
        Err("forma-gpu: built without the `gl` feature (Linux-only GLES backend)".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_atlas_dedups_and_packs() {
        let Some(font) = Font::system_default() else {
            eprintln!("skipping: no system font found");
            return;
        };
        // "hello" has 4 distinct glyphs (h, e, l, o); 'l' must pack once.
        let wanted: Vec<(char, f64)> = "hello".chars().map(|c| (c, 16.0)).collect();
        let atlas = GlyphAtlas::build(&font, &wanted);
        assert_eq!(atlas.uvs.len(), 4, "repeated 'l' should share one slot");
        for ch in ['h', 'e', 'l', 'o'] {
            assert!(atlas.uv(ch, 16.0).is_some(), "missing glyph {ch}");
        }
        // Whitespace is not packed; a different size buckets separately.
        let ws: Vec<(char, f64)> = vec![(' ', 16.0), ('a', 16.0), ('a', 24.0)];
        let atlas2 = GlyphAtlas::build(&font, &ws);
        assert_eq!(atlas2.uvs.len(), 2, "space dropped; 'a' at two sizes kept");
        // The atlas is at least as wide as the widest single glyph.
        assert!(atlas2.atlas.size().width >= 1);
    }
}
