//! Forma — a cross-platform, self-drawn UI toolkit in Rust.
//!
//! This umbrella crate ties the layers together and provides the [`App`]
//! entry point and a [`prelude`]. See `ROADMAP.md` for the architecture and
//! phased plan.
//!
//! ```
//! use forma::prelude::*;
//!
//! struct State;
//!
//! let app = App::new(State, |_state: &State, theme: &Theme| {
//!     panel(theme, vec![setting_row(theme, Color::rgb(80, 140, 255))])
//! })
//! .title("Demo")
//! .logical_size(Size::new(360.0, 200.0));
//!
//! // Render one frame off-screen (no window needed) and inspect it.
//! let frame = app.render_once();
//! assert_eq!(frame.size().width, 360);
//! ```

#![forbid(unsafe_code)]

// Re-export the layer crates for direct access.
pub use forma_anim as anim;
pub use forma_core as core;
pub use forma_geometry as geometry;
pub use forma_layout as layout;
pub use forma_platform as platform;
pub use forma_render as render;
pub use forma_style as style;
pub use forma_widgets as widgets;

use forma_core::{Element, render_view};
use forma_geometry::{ScaleFactor, Size};
use forma_platform::{ControlFlow, Event, WindowAttributes, backend::headless};
use forma_render::{Pixmap, SoftwareRenderer, Surface};
use forma_style::Theme;

/// A Forma application: some `state`, a `view` function mapping state (and the
/// active [`Theme`]) to an [`Element`] tree, and window attributes.
///
/// The reactive update loop (events mutate state → re-render) lands with the
/// `forma-core` reactivity milestone. Today [`App::run`] drives the headless
/// backend through one present cycle so the full
/// build → layout → paint → rasterize → present path is wired end to end; the
/// native windowed event loop swaps in at that same seam (ROADMAP Phases 1–2).
pub struct App<S, F>
where
    F: Fn(&S, &Theme) -> Element,
{
    state: S,
    view: F,
    theme: Theme,
    attrs: WindowAttributes,
    scale: ScaleFactor,
}

impl<S, F> std::fmt::Debug for App<S, F>
where
    F: Fn(&S, &Theme) -> Element,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `state` and `view` are not required to be Debug.
        f.debug_struct("App")
            .field("theme", &self.theme)
            .field("attrs", &self.attrs)
            .field("scale", &self.scale)
            .finish_non_exhaustive()
    }
}

impl<S, F> App<S, F>
where
    F: Fn(&S, &Theme) -> Element,
{
    /// Create an app from initial `state` and a `view` function.
    pub fn new(state: S, view: F) -> Self {
        Self {
            state,
            view,
            theme: Theme::light(),
            attrs: WindowAttributes::new(),
            scale: ScaleFactor::IDENTITY,
        }
    }

    /// Set the window title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.attrs.title = title.into();
        self
    }

    /// Set the initial logical window size.
    pub fn logical_size(mut self, size: Size) -> Self {
        self.attrs.logical_size = size;
        self
    }

    /// Set the active theme.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Override the DPI scale used for off-screen rendering (default 1×).
    pub fn scale(mut self, scale: ScaleFactor) -> Self {
        self.scale = scale;
        self
    }

    /// Access the current state.
    pub fn state(&self) -> &S {
        &self.state
    }

    fn build_scene(&self) -> forma_render::Scene {
        let element = (self.view)(&self.state, &self.theme);
        render_view(&element, self.attrs.logical_size, &self.theme)
    }

    /// Render a single frame off-screen and return it as a [`Pixmap`]. Needs no
    /// window — used for tests, thumbnails, and golden-image comparisons.
    pub fn render_once(&self) -> Pixmap {
        let renderer = SoftwareRenderer::new().with_background(self.theme.palette.background);
        renderer.render(self.build_scene(), self.scale)
    }

    /// Run the app. The scaffold drives the [`headless`] backend through a
    /// redraw + close cycle, presenting one frame into a real [`Surface`];
    /// native backends replace the loop without changing the render path.
    pub fn run(self) {
        let mut surface: Option<Box<dyn Surface>> = None;
        headless::run(
            self.attrs.clone(),
            [Event::RedrawRequested, Event::CloseRequested],
            |event, window| match event {
                Event::RedrawRequested => {
                    let surface = surface.get_or_insert_with(|| window.create_surface());
                    let renderer =
                        SoftwareRenderer::new().with_background(self.theme.palette.background);
                    let pixmap = renderer.render(self.build_scene(), window.scale_factor());
                    surface.resize(window.inner_size());
                    surface.present(&pixmap, &[]);
                    ControlFlow::Wait
                }
                Event::CloseRequested => ControlFlow::Exit,
                _ => ControlFlow::Wait,
            },
        );
    }
}

/// The common imports for building a Forma app.
pub mod prelude {
    pub use crate::App;
    pub use forma_anim::{Easing, Spring, Tween};
    pub use forma_core::{Align, Axis, BoxStyle, Element, View};
    pub use forma_geometry::{Insets, Point, Rect, ScaleFactor, Size};
    pub use forma_render::Color;
    pub use forma_style::Theme;
    pub use forma_widgets::{button, column, divider, panel, row, setting_row, spacer, swatch};
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_widgets::{panel, setting_row};

    fn demo() -> App<(), impl Fn(&(), &Theme) -> Element> {
        App::new((), |_s, theme| {
            panel(
                theme,
                vec![
                    setting_row(theme, forma_render::Color::rgb(80, 140, 255)),
                    setting_row(theme, forma_render::Color::rgb(80, 200, 120)),
                ],
            )
        })
        .logical_size(Size::new(320.0, 180.0))
    }

    #[test]
    fn render_once_matches_window_size() {
        let frame = demo().render_once();
        assert_eq!(frame.size(), forma_geometry::PhysicalSize::new(320, 180));
    }

    #[test]
    fn theme_changes_background_pixel() {
        let light = demo().theme(Theme::light()).render_once();
        let dark = demo().theme(Theme::dark()).render_once();
        // Top-left corner is window background; it must differ between themes.
        assert_ne!(light.pixel(0, 0), dark.pixel(0, 0));
    }

    #[test]
    fn run_presents_a_frame_headlessly() {
        let mut surface_seen = false;
        // Reuse run()'s machinery indirectly: a present must have occurred.
        let app = demo();
        let probe = {
            let attrs = app.attrs.clone();
            let window = headless::run(attrs, [Event::RedrawRequested], |_e, w| {
                let mut s = w.create_surface();
                s.present(&app.render_once(), &[]);
                ControlFlow::Exit
            });
            window.frame_probe()
        };
        if probe.last_frame().is_some() {
            surface_seen = true;
        }
        assert!(surface_seen);
    }
}
