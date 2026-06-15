//! The software rendering backend: rasterizes a [`Scene`] on the CPU via
//! `oxideav-raster` into a [`Pixmap`] ready for any [`Surface`] to present.

use crate::{Color, Pixmap, Scene};
use forma_geometry::{PhysicalSize, ScaleFactor};
use oxideav_raster::Renderer;

/// Rasterizes scenes on the CPU. Cheap to construct and reusable across
/// frames; the underlying `oxideav` renderer also caches per-subtree work.
#[derive(Debug, Default)]
pub struct SoftwareRenderer {
    background: Color,
}

impl SoftwareRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the canvas clear color (default: transparent).
    pub fn with_background(mut self, color: Color) -> Self {
        self.background = color;
        self
    }

    /// Rasterize `scene` at the given `scale`, returning a physical-pixel
    /// [`Pixmap`].
    ///
    /// The scene's logical size combined with `scale` determines the output
    /// resolution; the rasterizer maps logical → physical via the frame's
    /// view box, so widgets are authored once in logical pixels and stay
    /// crisp at any DPI.
    pub fn render(&self, scene: Scene, scale: ScaleFactor) -> Pixmap {
        let physical = scale.to_physical(scene.logical_size());
        self.render_at(scene, physical)
    }

    /// Rasterize `scene` into a buffer of exactly `physical` pixels.
    pub fn render_at(&self, scene: Scene, physical: PhysicalSize) -> Pixmap {
        let (w, h) = (physical.width.max(1), physical.height.max(1));
        let frame = scene.into_vector_frame();

        let mut renderer = Renderer::new(w, h);
        renderer.background = self.background.to_oxideav();
        let video = renderer.render(&frame);

        // `render` always produces a single packed-RGBA plane with
        // `stride == w * 4`; copy it into a tightly-packed Pixmap.
        let plane = &video.planes[0];
        let dst_stride = w as usize * 4;
        let total = dst_stride * h as usize;
        let mut data = vec![0u8; total];
        if plane.stride == dst_stride {
            data.copy_from_slice(&plane.data[..total]);
        } else {
            for y in 0..h as usize {
                let src = &plane.data[y * plane.stride..y * plane.stride + dst_stride];
                data[y * dst_stride..(y + 1) * dst_stride].copy_from_slice(src);
            }
        }
        Pixmap::from_rgba8(PhysicalSize::new(w, h), data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_geometry::{Rect, Size};

    #[test]
    fn fills_pixel_at_expected_location() {
        // 100x100 logical, 1x scale. Fill the whole canvas red, then check a
        // center pixel made it through the oxideav rasterizer.
        let mut scene = Scene::new(Size::new(100.0, 100.0));
        scene.fill_rect(
            Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
            Color::rgb(255, 0, 0),
        );
        let pm = SoftwareRenderer::new().render(scene, ScaleFactor::IDENTITY);

        assert_eq!(pm.size(), PhysicalSize::new(100, 100));
        let [r, g, b, a] = pm.pixel(50, 50).unwrap();
        assert_eq!((r, g, b, a), (255, 0, 0, 255));
    }

    #[test]
    fn hidpi_scale_doubles_resolution() {
        let scene = Scene::new(Size::new(100.0, 80.0));
        let pm = SoftwareRenderer::new().render(scene, ScaleFactor::new(2.0));
        assert_eq!(pm.size(), PhysicalSize::new(200, 160));
    }
}
