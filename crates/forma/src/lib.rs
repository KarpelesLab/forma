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

use forma_core::{
    ActionId, Cx, DragId, Element, FocusId, Handlers, KeyInput, LayoutNode, collect_focusables,
    drag_at, focus_at, hit_test, layout, paint,
};
use forma_geometry::{Point, Rect, ScaleFactor, Size};
use forma_platform::{
    ButtonState, ControlFlow, Event, KeyCode, WindowAttributes, backend::headless,
};
use forma_render::{Font, Pixmap, Scene, SoftwareRenderer, Surface};
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
    font: Option<Font>,
    // Retained from the last frame build, for routing pointer events.
    tree: Option<LayoutNode>,
    handlers: Handlers<S>,
    pressed: Option<ActionId>,
    focused: Option<FocusId>,
    dragging: Option<(DragId, Rect)>,
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
            font: None,
            tree: None,
            handlers: Handlers::default(),
            pressed: None,
            focused: None,
            dragging: None,
        }
    }

    /// Set the font used to render text. Without one, text elements are laid
    /// out as zero-size and not painted.
    pub fn font(mut self, font: Font) -> Self {
        self.font = Some(font);
        self
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
        let font = self.font.as_ref();
        let tree = layout(
            &element,
            Rect::from_xywh(0.0, 0.0, size.width, size.height),
            font,
        );
        let mut scene = Scene::new(size);
        paint(&tree, &mut scene, font);
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

    /// The currently focused element, if any.
    pub fn focused(&self) -> Option<FocusId> {
        self.focused
    }

    fn ensure_tree(&mut self) {
        if self.tree.is_none() {
            let _ = self.build_frame();
        }
    }

    /// Route a completed click at `pos` (logical pixels): update keyboard focus
    /// to the focusable under the cursor (if any), then dispatch the hit
    /// element's tap handler. Returns `true` if a tap handler ran.
    pub fn click_at(&mut self, pos: Point) -> bool {
        self.ensure_tree();
        let (hit, foc) = self
            .tree
            .as_ref()
            .map(|t| (hit_test(t, pos), focus_at(t, pos)))
            .unwrap_or((None, None));

        // Clicking moves focus (to the focusable under the cursor, or away).
        if foc != self.focused {
            self.focused = foc;
            self.tree = None;
        }
        match hit {
            Some(id) => {
                let ran = self.handlers.dispatch(id, &mut self.state);
                if ran {
                    self.tree = None;
                }
                ran
            }
            None => false,
        }
    }

    /// Deliver committed `text` to the focused element. Returns `true` if a
    /// focused key handler consumed it.
    pub fn type_text(&mut self, text: &str) -> bool {
        self.send_key(KeyInput::Text(text.to_string()))
    }

    /// Deliver an editing key (backspace, arrows, …) to the focused element.
    pub fn press_key(&mut self, input: KeyInput) -> bool {
        self.send_key(input)
    }

    fn send_key(&mut self, input: KeyInput) -> bool {
        self.ensure_tree();
        let Some(id) = self.focused else { return false };
        let ran = self.handlers.dispatch_key(id, &input, &mut self.state);
        if ran {
            self.tree = None;
        }
        ran
    }

    /// Move focus to the next focusable element in tree order (wrapping),
    /// like pressing Tab. Returns `true` if focus changed.
    pub fn focus_next(&mut self) -> bool {
        self.ensure_tree();
        let mut order = Vec::new();
        if let Some(t) = self.tree.as_ref() {
            collect_focusables(t, &mut order);
        }
        if order.is_empty() {
            return false;
        }
        let next = match self.focused {
            Some(cur) => match order.iter().position(|f| *f == cur) {
                Some(i) => order[(i + 1) % order.len()],
                None => order[0],
            },
            None => order[0],
        };
        let changed = self.focused != Some(next);
        self.focused = Some(next);
        self.tree = None;
        changed
    }

    /// Begin or continue a pointer drag at `pos` (logical pixels). On the first
    /// call it latches onto the draggable element under the cursor; subsequent
    /// calls feed it the pointer's fractional x position until [`App::end_drag`].
    /// Returns `true` if a drag handler ran.
    pub fn drag_at_point(&mut self, pos: Point) -> bool {
        self.ensure_tree();
        if self.dragging.is_none() {
            self.dragging = self.tree.as_ref().and_then(|t| drag_at(t, pos));
        }
        let Some((id, bounds)) = self.dragging else {
            return false;
        };
        let fraction = if bounds.width() > 0.0 {
            ((pos.x - bounds.min_x()) / bounds.width()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let ran = self.handlers.dispatch_drag(id, fraction, &mut self.state);
        if ran {
            self.tree = None;
        }
        ran
    }

    /// End the current drag (pointer released).
    pub fn end_drag(&mut self) {
        self.dragging = None;
    }

    /// Run the app. The scaffold drives the [`headless`] backend through a
    /// redraw + close cycle, presenting frames into a real [`Surface`] and
    /// routing pointer/keyboard events; native backends replace the loop
    /// without changing the render or dispatch path.
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
                    // Latch a drag if a draggable sits under the cursor.
                    if self.drag_at_point(position) {
                        window.request_redraw();
                    }
                    ControlFlow::Wait
                }
                Event::PointerMoved { position } => {
                    if self.dragging.is_some() && self.drag_at_point(position) {
                        window.request_redraw();
                    }
                    ControlFlow::Wait
                }
                Event::PointerButton {
                    state: ButtonState::Released,
                    position,
                    ..
                } => {
                    if self.dragging.is_some() {
                        self.end_drag();
                    } else {
                        let down = self.pressed.take();
                        let up = self.tree.as_ref().and_then(|t| hit_test(t, position));
                        if down.is_some() && down == up {
                            self.click_at(position);
                            window.request_redraw();
                        }
                    }
                    ControlFlow::Wait
                }
                Event::Text(text) => {
                    if self.type_text(&text) {
                        window.request_redraw();
                    }
                    ControlFlow::Wait
                }
                Event::Key {
                    code: KeyCode::Tab,
                    state: ButtonState::Pressed,
                    ..
                } => {
                    if self.focus_next() {
                        window.request_redraw();
                    }
                    ControlFlow::Wait
                }
                Event::Key {
                    code,
                    state: ButtonState::Pressed,
                    ..
                } => {
                    if let Some(input) = map_key(code) {
                        if self.press_key(input) {
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

/// Translate a platform [`KeyCode`] into a core [`KeyInput`] editing command.
/// Character input arrives via [`Event::Text`], so only editing/navigation keys
/// map here.
fn map_key(code: KeyCode) -> Option<KeyInput> {
    Some(match code {
        KeyCode::Backspace => KeyInput::Backspace,
        KeyCode::ArrowLeft => KeyInput::Left,
        KeyCode::ArrowRight => KeyInput::Right,
        KeyCode::Enter => KeyInput::Enter,
        KeyCode::Escape => KeyInput::Escape,
        _ => return None,
    })
}

/// The common imports for building a Forma app.
pub mod prelude {
    pub use crate::App;
    pub use forma_anim::{Easing, Spring, Tween};
    pub use forma_core::{Align, Axis, BoxStyle, Cx, Element, KeyInput, View};
    pub use forma_geometry::{Insets, Point, Rect, ScaleFactor, Size};
    pub use forma_render::{Color, Font};
    pub use forma_style::Theme;
    pub use forma_widgets::{
        button, button_labeled, checkbox, column, divider, edit_string, label, panel, row,
        setting_row, slider, spacer, swatch, switch, text_field,
    };
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

    #[derive(Default)]
    struct Form {
        name: String,
    }

    /// A form with a single text field filling the window.
    fn form_app() -> App<Form, impl FnMut(&Form, &mut Cx<'_, Form>) -> Element> {
        App::new(Form::default(), |state: &Form, cx: &mut Cx<Form>| {
            let theme = *cx.theme();
            forma_widgets::text_field(cx, &theme, &state.name, |s: &mut Form, k| {
                forma_widgets::edit_string(&mut s.name, k)
            })
            .width(300.0)
            .height(100.0)
        })
        .logical_size(Size::new(300.0, 100.0))
    }

    #[test]
    fn focus_and_type_edits_state() {
        let mut app = form_app();
        // Typing with nothing focused is a no-op.
        assert!(!app.type_text("x"));
        assert_eq!(app.state().name, "");

        // Focus the field by clicking it, then type.
        app.click_at(Point::new(150.0, 50.0));
        assert!(app.focused().is_some());
        assert!(app.type_text("Ada"));
        assert_eq!(app.state().name, "Ada");

        // Backspace removes the last character.
        assert!(app.press_key(KeyInput::Backspace));
        assert_eq!(app.state().name, "Ad");
    }

    #[derive(Default)]
    struct Toggles {
        on: bool,
    }

    #[test]
    fn checkbox_toggles_on_click() {
        let mut app = App::new(Toggles::default(), |s: &Toggles, cx: &mut Cx<Toggles>| {
            let theme = *cx.theme();
            // A checkbox filling the window for a predictable hit point.
            forma_widgets::checkbox(cx, &theme, s.on, |t: &mut Toggles| t.on = !t.on)
                .width(100.0)
                .height(100.0)
        })
        .logical_size(Size::new(100.0, 100.0));

        assert!(!app.state().on);
        app.click_at(Point::new(50.0, 50.0));
        assert!(app.state().on);
        app.click_at(Point::new(50.0, 50.0));
        assert!(!app.state().on);
    }

    #[derive(Default)]
    struct Volume {
        level: f64,
    }

    #[test]
    fn slider_drag_sets_value_from_position() {
        let mut app = App::new(Volume::default(), |s: &Volume, cx: &mut Cx<Volume>| {
            let theme = *cx.theme();
            forma_widgets::slider(cx, &theme, s.level, |v: &mut Volume, f| v.level = f).width(200.0)
        })
        .logical_size(Size::new(200.0, 40.0));

        // Press at x=150 of a 200-wide slider -> fraction 0.75.
        app.drag_at_point(Point::new(150.0, 20.0));
        assert!(
            (app.state().level - 0.75).abs() < 1e-9,
            "got {}",
            app.state().level
        );
        // Drag to x=50 -> 0.25.
        app.drag_at_point(Point::new(50.0, 20.0));
        assert!((app.state().level - 0.25).abs() < 1e-9);
        app.end_drag();
    }

    #[test]
    fn tab_focuses_first_field() {
        let mut app = form_app();
        assert!(app.focused().is_none());
        assert!(app.focus_next());
        assert!(app.focused().is_some());
        assert!(app.type_text("hi"));
        assert_eq!(app.state().name, "hi");
    }
}
