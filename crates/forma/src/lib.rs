//! Forma — a cross-platform, self-drawn UI toolkit in Rust.
//!
//! This umbrella crate ties the layers together and provides the [`App`]
//! entry point and a [`prelude`]. See `ROADMAP.md` for the architecture and
//! phased plan.
//!
//! ```
//! use forma::prelude::*;
//!
//! struct Counter {
//!     n: i64,
//! }
//!
//! let mut app = App::new(Counter { n: 0 }, |state: &Counter, cx: &mut Cx<Counter>| {
//!     let theme = *cx.theme();
//!     panel(
//!         &theme,
//!         vec![button(&theme).on_tap(cx, |s: &mut Counter| s.n += 1)],
//!     )
//! })
//! .title("Counter")
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

use forma_core::{ActionId, Cx, Element, Handlers, LayoutNode, hit_test, layout, paint};
use forma_geometry::{Point, Rect, ScaleFactor, Size};
use forma_platform::{ButtonState, ControlFlow, Event, WindowAttributes, backend::headless};
use forma_render::{Pixmap, Scene, SoftwareRenderer, Surface};
use forma_style::Theme;

/// A Forma application.
///
/// Holds the app `state`, a `build` closure mapping state (and a [`Cx`] for
/// registering event handlers) to an [`Element`] tree, the active [`Theme`],
/// and window attributes. Pointer taps are routed through the laid-out tree
/// back to the `on_tap` handlers the build closure registered.
///
/// [`App::run`] drives the headless backend through a present cycle so the full
/// build → layout → paint → rasterize → present path is wired end to end; the
/// native windowed event loop swaps in at that same seam (ROADMAP Phases 1–2).
pub struct App<S, F>
where
    F: FnMut(&S, &mut Cx<'_, S>) -> Element,
{
    state: S,
    build: F,
    theme: Theme,
    attrs: WindowAttributes,
    scale: ScaleFactor,
    // Retained from the last frame build, for routing pointer events.
    tree: Option<LayoutNode>,
    handlers: Handlers<S>,
    pressed: Option<ActionId>,
}

impl<S, F> std::fmt::Debug for App<S, F>
where
    F: FnMut(&S, &mut Cx<'_, S>) -> Element,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `state` and `build` are not required to be Debug.
        f.debug_struct("App")
            .field("theme", &self.theme)
            .field("attrs", &self.attrs)
            .field("scale", &self.scale)
            .field("handlers", &self.handlers)
            .finish_non_exhaustive()
    }
}

impl<S, F> App<S, F>
where
    F: FnMut(&S, &mut Cx<'_, S>) -> Element,
{
    /// Create an app from initial `state` and a `build` closure.
    pub fn new(state: S, build: F) -> Self {
        Self {
            state,
            build,
            theme: Theme::light(),
            attrs: WindowAttributes::new(),
            scale: ScaleFactor::IDENTITY,
            tree: None,
            handlers: Handlers::default(),
            pressed: None,
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

    /// Build the element tree for the current state, lay it out to fill the
    /// window, retain the layout tree + handlers for event routing, and return
    /// the painted [`Scene`].
    fn build_frame(&mut self) -> Scene {
        let theme = self.theme; // Theme is Copy; avoids borrowing self in `cx`.
        let mut cx = Cx::new(&theme);
        let element = (self.build)(&self.state, &mut cx);
        self.handlers = cx.into_handlers();

        let size = self.attrs.logical_size;
        let tree = layout(&element, Rect::from_xywh(0.0, 0.0, size.width, size.height));
        let mut scene = Scene::new(size);
        paint(&tree, &mut scene);
        self.tree = Some(tree);
        scene
    }

    /// Render a single frame off-screen and return it as a [`Pixmap`]. Needs no
    /// window — used for tests, thumbnails, and golden-image comparisons.
    pub fn render_once(&mut self) -> Pixmap {
        let scene = self.build_frame();
        SoftwareRenderer::new()
            .with_background(self.theme.palette.background)
            .render(scene, self.scale)
    }

    /// Route a completed click at `pos` (logical pixels): hit-test the current
    /// frame and dispatch the hit element's handler against the state. Returns
    /// `true` if a handler ran (the UI should be re-rendered).
    ///
    /// Ensures a frame has been built so the layout tree exists.
    pub fn click_at(&mut self, pos: Point) -> bool {
        if self.tree.is_none() {
            let _ = self.build_frame();
        }
        let hit = self.tree.as_ref().and_then(|t| hit_test(t, pos));
        match hit {
            Some(id) => {
                let ran = self.handlers.dispatch(id, &mut self.state);
                if ran {
                    // State changed: the retained tree is now stale.
                    self.tree = None;
                }
                ran
            }
            None => false,
        }
    }

    /// Run the app. The scaffold drives the [`headless`] backend through a
    /// redraw + close cycle, presenting frames into a real [`Surface`] and
    /// routing pointer press/release into clicks; native backends replace the
    /// loop without changing the render or dispatch path.
    pub fn run(mut self) {
        let attrs = self.attrs.clone();
        let mut surface: Option<Box<dyn Surface>> = None;
        headless::run(
            attrs,
            [Event::RedrawRequested, Event::CloseRequested],
            |event, window| match event {
                Event::RedrawRequested => {
                    let scene = self.build_frame();
                    let pixmap = SoftwareRenderer::new()
                        .with_background(self.theme.palette.background)
                        .render(scene, window.scale_factor());
                    let surface = surface.get_or_insert_with(|| window.create_surface());
                    surface.resize(window.inner_size());
                    surface.present(&pixmap, &[]);
                    ControlFlow::Wait
                }
                Event::PointerButton {
                    state: ButtonState::Pressed,
                    position,
                    ..
                } => {
                    self.pressed = self.tree.as_ref().and_then(|t| hit_test(t, position));
                    ControlFlow::Wait
                }
                Event::PointerButton {
                    state: ButtonState::Released,
                    position,
                    ..
                } => {
                    let released_on = self.tree.as_ref().and_then(|t| hit_test(t, position));
                    if let (Some(down), Some(up)) = (self.pressed.take(), released_on) {
                        if down == up && self.handlers.dispatch(down, &mut self.state) {
                            window.request_redraw();
                        }
                    }
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
    pub use forma_core::{Align, Axis, BoxStyle, Cx, Element, View};
    pub use forma_geometry::{Insets, Point, Rect, ScaleFactor, Size};
    pub use forma_render::Color;
    pub use forma_style::Theme;
    pub use forma_widgets::{button, column, divider, panel, row, setting_row, spacer, swatch};
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_widgets::{button, column};

    struct Counter {
        n: i64,
    }

    /// A counter view: a single 200×80 button at the window origin that
    /// increments the count. Fixed geometry keeps the hit point predictable.
    fn counter_app() -> App<Counter, impl FnMut(&Counter, &mut Cx<'_, Counter>) -> Element> {
        App::new(
            Counter { n: 0 },
            |_state: &Counter, cx: &mut Cx<Counter>| {
                let theme = *cx.theme();
                column(vec![
                    button(&theme)
                        .width(200.0)
                        .height(80.0)
                        .on_tap(cx, |s: &mut Counter| s.n += 1),
                ])
            },
        )
        .logical_size(Size::new(200.0, 80.0))
    }

    #[test]
    fn click_dispatches_handler_and_mutates_state() {
        let mut app = counter_app();
        assert_eq!(app.state().n, 0);
        // Click inside the button.
        assert!(app.click_at(Point::new(100.0, 40.0)));
        assert_eq!(app.state().n, 1);
        app.click_at(Point::new(100.0, 40.0));
        assert_eq!(app.state().n, 2);
    }

    #[test]
    fn click_outside_any_handler_is_a_noop() {
        let mut app = counter_app().logical_size(Size::new(400.0, 400.0));
        // (300, 300) is outside the 200×80 button.
        assert!(!app.click_at(Point::new(300.0, 300.0)));
        assert_eq!(app.state().n, 0);
    }

    #[test]
    fn render_once_matches_window_size() {
        let mut app = counter_app();
        assert_eq!(
            app.render_once().size(),
            forma_geometry::PhysicalSize::new(200, 80)
        );
    }
}
