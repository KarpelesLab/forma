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

use forma_geometry::{PhysicalSize, Rect};
use forma_render::{Color, Pixmap};

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
/// GPU-native scene renderer. Errors if the `gl` feature is off or no GL/EGL
/// device is available.
pub fn fill_rects_offscreen(
    size: PhysicalSize,
    background: Color,
    rects: &[(Rect, Color)],
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
