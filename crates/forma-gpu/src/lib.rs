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

/// Render a live forma [`Scene`] **GPU-natively**: its box primitives become
/// SDF-shaded geometry and each text run is CPU-rasterized (via `font`) to a
/// coverage mask the GPU then composites — so any forma UI renders through the
/// GPU, not just hand-built rectangles. `background` clears the target. Errors
/// if the `gl` feature is off or no GL/EGL device is available.
pub fn render_scene(scene: &Scene, background: Color, font: &Font) -> Result<Pixmap, String> {
    let ls = scene.logical_size();
    let size = ScaleFactor::IDENTITY.to_physical(ls);
    let mut rects: Vec<(Rect, Color, f32, f32)> = Vec::new();
    // Keep the rasterized masks alive for the borrow passed to `render_offscreen`.
    let mut masks: Vec<(Pixmap, Rect, Color)> = Vec::new();
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
                let m = font.measure(text, *px);
                let (tw, th) = (m.width.ceil() + 4.0, m.height.ceil() + 2.0);
                let mut ts = Scene::new(Size::new(tw, th));
                ts.fill_text(font, text, Point::new(2.0, 1.0), *px, Color::WHITE);
                let pm = SoftwareRenderer::new()
                    .with_background(Color::BLACK)
                    .render(ts, ScaleFactor::IDENTITY);
                masks.push((
                    pm,
                    Rect::from_xywh(origin.x - 2.0, origin.y - 1.0, tw, th),
                    *color,
                ));
            }
        }
    }
    let texts: Vec<(&Pixmap, Rect, Color)> = masks.iter().map(|(p, r, c)| (p, *r, *c)).collect();
    render_offscreen(size, background, &rects, &texts)
}
