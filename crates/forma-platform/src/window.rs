use crate::event::WindowId;
use forma_geometry::{PhysicalSize, ScaleFactor, Size};
use forma_render::Surface;

/// Desired initial properties for a window. Consumed by the backend when
/// creating the window; later changes go through [`Window`] methods.
#[derive(Clone, Debug)]
pub struct WindowAttributes {
    pub title: String,
    /// Initial inner (content) size in logical pixels.
    pub logical_size: Size,
    pub resizable: bool,
}

impl Default for WindowAttributes {
    fn default() -> Self {
        Self {
            title: "Forma".to_string(),
            logical_size: Size::new(800.0, 600.0),
            resizable: true,
        }
    }
}

impl WindowAttributes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_logical_size(mut self, size: Size) -> Self {
        self.logical_size = size;
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }
}

/// A live native window.
///
/// Backends implement this; the rest of Forma drives windows only through this
/// trait. A window can hand out a [`Surface`] (via [`Window::create_surface`])
/// that the renderer presents into.
pub trait Window {
    fn id(&self) -> WindowId;

    /// Current drawable size in physical pixels.
    fn inner_size(&self) -> PhysicalSize;

    /// Current DPI scale factor (physical ÷ logical).
    fn scale_factor(&self) -> ScaleFactor;

    /// Ask the system to deliver a [`RedrawRequested`](crate::Event::RedrawRequested).
    fn request_redraw(&self);

    /// Update the window title.
    fn set_title(&self, title: &str);

    /// Create a [`Surface`] that presents into this window's drawable.
    fn create_surface(&self) -> Box<dyn Surface>;

    /// Read the system clipboard's text, if any. Default: `None` (backends that
    /// don't implement the clipboard fall back to the in-process mirror).
    fn clipboard(&self) -> Option<String> {
        None
    }

    /// Set the system clipboard's text. Default: a no-op.
    fn set_clipboard(&self, _text: &str) {}
}
