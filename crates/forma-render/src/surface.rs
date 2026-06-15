use forma_geometry::{PhysicalSize, Rect};

/// A CPU pixel buffer in straight (non-premultiplied) RGBA8, row-major,
/// tightly packed (`stride == width * 4`).
///
/// This is the payload handed to a [`Surface`] for display. It is what the
/// software backend produces today; a future GPU backend may bypass it.
#[derive(Clone, Debug)]
pub struct Pixmap {
    size: PhysicalSize,
    data: Vec<u8>,
}

impl Pixmap {
    /// Allocate a fully transparent pixmap of `size`.
    pub fn new(size: PhysicalSize) -> Self {
        Self {
            size,
            data: vec![0u8; size.pixel_count() as usize * 4],
        }
    }

    /// Wrap existing RGBA8 bytes. Panics if `data.len() != width*height*4`.
    pub fn from_rgba8(size: PhysicalSize, data: Vec<u8>) -> Self {
        assert_eq!(
            data.len(),
            size.pixel_count() as usize * 4,
            "pixmap byte length must equal width*height*4"
        );
        Self { size, data }
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize {
        self.size
    }

    /// Bytes per row.
    #[inline]
    pub fn stride(&self) -> usize {
        self.size.width as usize * 4
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    #[inline]
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// The four bytes `[r, g, b, a]` at `(x, y)`, or `None` if out of bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.size.width || y >= self.size.height {
            return None;
        }
        let i = y as usize * self.stride() + x as usize * 4;
        Some([
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ])
    }
}

/// A presentable destination owned by the platform layer (a window's drawable,
/// a canvas, a layer-backed view, …).
///
/// This trait is the **GPU-readiness seam**: the software backend presents a
/// [`Pixmap`] by blitting; a future GPU backend can implement the same trait
/// by uploading or compositing directly. `damage` lists the regions (in
/// physical pixels) that actually changed since the last present — an empty
/// slice means "assume everything changed."
pub trait Surface {
    /// Resize the backing store to `size` physical pixels.
    fn resize(&mut self, size: PhysicalSize);

    /// The current backing-store size in physical pixels.
    fn size(&self) -> PhysicalSize;

    /// Present `pixmap`, optionally limited to the `damage` regions.
    fn present(&mut self, pixmap: &Pixmap, damage: &[Rect]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_access_bounds() {
        let mut pm = Pixmap::new(PhysicalSize::new(2, 2));
        pm.as_bytes_mut()[4..8].copy_from_slice(&[10, 20, 30, 40]);
        assert_eq!(pm.pixel(1, 0), Some([10, 20, 30, 40]));
        assert_eq!(pm.pixel(2, 0), None);
        assert_eq!(pm.stride(), 8);
    }
}
