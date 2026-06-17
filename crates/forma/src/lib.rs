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
    ActionId, Anchor, BoxStyle, Cx, Damage, DragId, Element, FocusId, Handlers, KeyInput,
    LayoutNode, TextPosId, caret_index_at, collect_focusables, drag_at, find_text_pos, focus_at,
    hit_test, layout, measure, paint, paint_focus, paint_hover, text_pos_at,
};
use forma_geometry::{Point, Rect, ScaleFactor, Size};
use forma_platform::{
    ButtonState, ControlFlow, Event, KeyCode, Modifiers, WindowAttributes, WindowId, backend,
};
use forma_render::{Color, Font, Pixmap, Scene, SoftwareRenderer, Surface};
use forma_style::Theme;

/// A Forma application.
///
/// Holds the app `state`, a `build` closure mapping state (and a [`Cx`] for
/// registering event handlers) to an [`Element`] tree, the active [`Theme`],
/// and window attributes. Pointer taps are routed through the laid-out tree
/// back to the `on_tap` handlers the build closure registered.
///
/// [`App::run`] drives the platform backend (native X11 when `$DISPLAY` is set,
/// else a one-shot headless present) through the full build → layout → paint →
/// rasterize → present path, routing pointer and keyboard events back to the
/// registered handlers.
pub struct App<S> {
    // Global application state, shared by every window's view.
    state: S,
    // Shared presentation settings used by all windows.
    theme: Theme,
    font: Option<Font>,
    // One [`Pane`] per OS window. There is always at least one (the primary,
    // index 0); additional windows are added via [`App::open_window`].
    panes: Vec<Pane<S>>,
}

/// A pluggable rasterizer: turns a built [`Scene`] (with a background color, at a
/// scale factor) into the [`Pixmap`] the [`Surface`] presents. Set via
/// [`App::render_with`] to route frames through a GPU backend.
pub type FrameRenderer = Box<dyn FnMut(&Scene, Color, ScaleFactor) -> Pixmap>;

/// A view closure mapping the shared state (and a [`Cx`] for registering event
/// handlers) to an [`Element`] tree. Boxed so each window can hold its own view.
pub type ViewFn<S> = Box<dyn FnMut(&S, &mut Cx<'_, S>) -> Element>;

/// Everything specific to one OS window: its view onto the shared state, its
/// window attributes, and all the retained per-window render/event state (the
/// laid-out tree, handlers, focus/hover/drag/selection, scroll offsets, the
/// on-screen diff baseline, and an optional custom rasterizer).
///
/// Multiple panes read and mutate the same `App::state`, so they stay in sync;
/// each maintains its own input focus and damage tracking independently.
struct Pane<S> {
    view: ViewFn<S>,
    attrs: WindowAttributes,
    scale: ScaleFactor,
    // Optional GPU (or other) rasterizer used in place of the software renderer
    // to turn each frame's `Scene` into the `Pixmap` that is presented. Lets a
    // GPU backend (forma-gpu) drive on-screen present through the `Surface` seam
    // without forma depending on it. `None` = software rasterization.
    frame_renderer: Option<FrameRenderer>,
    // Retained from the last frame build, for routing pointer events.
    tree: Option<LayoutNode>,
    handlers: Handlers<S>,
    pressed: Option<ActionId>,
    focused: Option<FocusId>,
    hovered: Option<ActionId>,
    dragging: Option<(DragId, Rect)>,
    // The text element currently being selected by a pointer drag.
    text_selecting: Option<TextPosId>,
    // Set when state/focus/hover changed so the next build rebuilds the tree.
    dirty: bool,
    // The tree + overlay state that is currently on screen, used as the diff
    // baseline so a present can be limited to the region that actually changed.
    presented: Option<LayoutNode>,
    painted_hovered: Option<ActionId>,
    painted_focused: Option<FocusId>,
    // Cross-frame memo cache for `Cx::memo` (static subtree reuse).
    memo_cache: std::collections::HashMap<u64, Element>,
    // Per-container scroll offsets (vertical, logical px), adjusted by wheel
    // events and re-applied + clamped each frame by `apply_scroll`.
    scroll_offsets: std::collections::HashMap<forma_core::ScrollId, f64>,
    // Last pointer position, so a `Scroll` event (which carries only a delta)
    // can find the scroll container under the cursor.
    last_pointer: Point,
    // Whether the frame just built / on screen has overlay layers — overlays
    // can't be localized by the tree diff, so their presence forces full damage.
    overlay_active: bool,
    painted_overlay_active: bool,
}

impl<S> std::fmt::Debug for App<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `state` and the view closures are not required to be Debug.
        f.debug_struct("App")
            .field("theme", &self.theme)
            .field("windows", &self.panes.len())
            .finish_non_exhaustive()
    }
}

impl<S> Pane<S> {
    /// Create a pane for `view` with default window attributes.
    fn new(view: ViewFn<S>) -> Self {
        Self {
            view,
            attrs: WindowAttributes::new(),
            scale: ScaleFactor::IDENTITY,
            frame_renderer: None,
            tree: None,
            handlers: Handlers::default(),
            pressed: None,
            focused: None,
            hovered: None,
            dragging: None,
            text_selecting: None,
            dirty: true,
            presented: None,
            painted_hovered: None,
            painted_focused: None,
            memo_cache: std::collections::HashMap::new(),
            scroll_offsets: std::collections::HashMap::new(),
            last_pointer: Point::new(0.0, 0.0),
            overlay_active: false,
            painted_overlay_active: false,
        }
    }
}

impl<S> App<S> {
    /// Create an app from initial `state` and a primary-window `build` closure.
    pub fn new(state: S, build: impl FnMut(&S, &mut Cx<'_, S>) -> Element + 'static) -> Self {
        Self {
            state,
            theme: Theme::light(),
            font: None,
            panes: vec![Pane::new(Box::new(build))],
        }
    }

    /// The primary window's pane (index 0), which the builder methods and the
    /// single-window public API operate on. Always present.
    fn primary(&mut self) -> &mut Pane<S> {
        &mut self.panes[0]
    }

    /// Register an additional OS window with its own `view` onto the shared
    /// state. Forma drives every registered window once [`App::run`] starts.
    ///
    /// Note: true OS multi-window presentation is wired per backend (X11 first);
    /// on backends that don't yet support it only the primary window is shown.
    pub fn open_window(
        &mut self,
        attrs: WindowAttributes,
        view: impl FnMut(&S, &mut Cx<'_, S>) -> Element + 'static,
    ) {
        let mut pane = Pane::new(Box::new(view));
        pane.attrs = attrs;
        self.panes.push(pane);
    }

    /// Set the font used to render text. Without one, text elements are laid
    /// out as zero-size and not painted.
    pub fn font(mut self, font: Font) -> Self {
        self.font = Some(font);
        self
    }

    /// Route frame rasterization through a custom renderer — e.g. a GPU backend
    /// (`forma-gpu`) that turns each frame's [`Scene`] into a [`Pixmap`] on the
    /// GPU, which is then presented through the platform [`Surface`]. This wires
    /// GPU rendering into the live present path without forma depending on any
    /// GPU crate. Without it, frames are rasterized on the CPU.
    pub fn render_with(
        mut self,
        renderer: impl FnMut(&Scene, Color, ScaleFactor) -> Pixmap + 'static,
    ) -> Self {
        self.primary().frame_renderer = Some(Box::new(renderer));
        self
    }

    /// Set the window title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.primary().attrs.title = title.into();
        self
    }

    /// Set the initial logical window size.
    pub fn logical_size(mut self, size: Size) -> Self {
        self.primary().attrs.logical_size = size;
        self
    }

    /// Set the active theme.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Override the DPI scale used for off-screen rendering (default 1×).
    pub fn scale(mut self, scale: ScaleFactor) -> Self {
        self.primary().scale = scale;
        self
    }

    /// Access the current state.
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S> Pane<S> {
    /// Rasterize a built `scene` into a `Pixmap` — through the custom
    /// [`render_with`](App::render_with) renderer if set, else the software path.
    fn rasterize(&mut self, scene: Scene, theme: &Theme, scale: ScaleFactor) -> Pixmap {
        let bg = theme.palette.background;
        match self.frame_renderer.as_mut() {
            Some(render) => render(&scene, bg, scale),
            None => SoftwareRenderer::new()
                .with_background(bg)
                .render(scene, scale),
        }
    }

    /// Build the element tree for the current state, lay it out to fill the
    /// window, retain the layout tree + handlers for event routing, and return
    /// the painted [`Scene`].
    fn build_frame(&mut self, state: &S, theme: &Theme, font: Option<&Font>) -> Scene {
        let theme = *theme; // Theme is Copy; avoids borrowing through `theme` in `cx`.
        let mut cx = Cx::new(&theme);
        cx.set_memo_cache(std::mem::take(&mut self.memo_cache));
        let element = (self.view)(state, &mut cx);
        self.memo_cache = cx.take_memo_cache();
        let overlays = cx.take_overlays();
        self.handlers = cx.into_handlers();

        let size = self.attrs.logical_size;
        let win = Rect::from_xywh(0.0, 0.0, size.width, size.height);
        let main = layout(&element, win, font);

        // Compose the main tree with any floating overlay layers under one
        // routable/paintable root: each overlay (with a scrim behind a modal
        // one) is a later child, so it paints on top and hit-tests first.
        let mut tree = if overlays.is_empty() {
            main
        } else {
            let mut roots = Vec::with_capacity(overlays.len() * 2 + 1);
            roots.push(main);
            for spec in &overlays {
                // A full-window catcher behind the overlay: a dark scrim for a
                // modal, or an invisible click-catcher for a non-modal that wants
                // outside-press dismissal. Either way it carries the dismiss
                // action and blocks clicks from reaching the tree below.
                if spec.modal || spec.dismiss.is_some() {
                    let mut catcher = Element::boxed(BoxStyle {
                        fill: spec.modal.then(|| Color::rgba(0, 0, 0, 0x80)),
                        radius: 0.0,
                        border: None,
                    })
                    .width(size.width)
                    .height(size.height);
                    catcher.action = spec.dismiss;
                    roots.push(layout(&catcher, win, font));
                }
                let m = measure(&spec.content, size, font);
                let origin = match spec.anchor {
                    Anchor::At(p) => p,
                    Anchor::Center => Point::new(
                        ((size.width - m.width) / 2.0).max(0.0),
                        ((size.height - m.height) / 2.0).max(0.0),
                    ),
                };
                let bounds = Rect::from_xywh(origin.x, origin.y, m.width, m.height);
                roots.push(layout(&spec.content, bounds, font));
            }
            LayoutNode::container(win, roots)
        };
        self.overlay_active = !overlays.is_empty();
        // Apply (and clamp) scroll-container offsets to the laid-out tree before
        // painting, so the retained tree matches what's drawn for event routing.
        forma_core::apply_scroll(&mut tree, &mut self.scroll_offsets);
        let mut scene = Scene::new(size);
        paint(&tree, &mut scene, font);
        // Lighten the hovered tappable element with the theme's overlay.
        if let Some(hid) = self.hovered {
            paint_hover(&tree, hid, &mut scene, theme.palette.hover_overlay);
        }
        // Overlay a focus ring + caret on the focused element.
        if let Some(fid) = self.focused {
            paint_focus(
                &tree,
                fid,
                &mut scene,
                font,
                theme.palette.focus_ring,
                theme.palette.text,
                theme.palette.selection,
            );
        }
        self.tree = Some(tree);
        self.dirty = false;
        scene
    }

    /// Compute the [`Damage`] of the frame just built (in `self.tree`) relative
    /// to what is currently on screen (`self.presented`), then adopt the new
    /// frame as the on-screen baseline.
    ///
    /// Hover/focus overlays are painted outside the [`LayoutNode`] tree, so a
    /// change to either can't be localized by the tree diff — those frames, plus
    /// the first frame and any root-size (resize) change, report [`Damage::Full`].
    fn take_damage(&mut self) -> Damage {
        let overlay_changed = self.hovered != self.painted_hovered
            || self.focused != self.painted_focused
            // Floating overlays paint above the tree and can't be localized by
            // the diff, so any frame with overlays (or that just had them) is full.
            || self.overlay_active
            || self.painted_overlay_active;
        let damage = match (&self.presented, &self.tree) {
            (Some(old), Some(new)) if !overlay_changed && old.bounds == new.bounds => {
                forma_core::diff_trees(old, new)
            }
            _ => Damage::Full,
        };
        self.presented = self.tree.clone();
        self.painted_hovered = self.hovered;
        self.painted_focused = self.focused;
        self.painted_overlay_active = self.overlay_active;
        damage
    }

    /// Build, paint, and rasterize the next frame, returning the [`Pixmap`] and
    /// the [`Damage`] (changed region, in logical pixels) relative to the
    /// previously returned frame. The first call always reports [`Damage::Full`].
    fn render_frame(&mut self, state: &S, theme: &Theme, font: Option<&Font>) -> (Pixmap, Damage) {
        let scene = self.build_frame(state, theme, font);
        let scale = self.scale;
        let pixmap = self.rasterize(scene, theme, scale);
        let damage = self.take_damage();
        (pixmap, damage)
    }

    /// Build the [accessibility tree](forma_core::accessibility_tree) for the
    /// current frame, or `None` until one has been built.
    fn accessibility_tree(&self) -> Option<forma_core::AccessNode> {
        self.tree
            .as_ref()
            .map(|t| forma_core::accessibility_tree(t, self.focused))
    }

    fn ensure_tree(&mut self, state: &S, theme: &Theme, font: Option<&Font>) {
        if self.tree.is_none() || self.dirty {
            let _ = self.build_frame(state, theme, font);
        }
    }

    /// Route a completed click at `pos` (logical pixels): update keyboard focus
    /// to the focusable under the cursor (if any), then dispatch the hit
    /// element's tap handler. Returns `true` if a tap handler ran.
    fn click_at(&mut self, state: &mut S, theme: &Theme, font: Option<&Font>, pos: Point) -> bool {
        self.ensure_tree(state, theme, font);
        let (hit, foc) = self
            .tree
            .as_ref()
            .map(|t| (hit_test(t, pos), focus_at(t, pos)))
            .unwrap_or((None, None));

        // Clicking moves focus (to the focusable under the cursor, or away).
        if foc != self.focused {
            self.focused = foc;
            self.dirty = true;
        }
        match hit {
            Some(id) => {
                let ran = self.handlers.dispatch(id, state);
                if ran {
                    self.dirty = true;
                }
                ran
            }
            None => false,
        }
    }

    /// Deliver committed `text` to the focused element. Returns `true` if a
    /// focused key handler consumed it.
    fn type_text(&mut self, state: &mut S, theme: &Theme, font: Option<&Font>, text: &str) -> bool {
        self.send_key(state, theme, font, KeyInput::Text(text.to_string()))
    }

    /// Deliver an editing key (backspace, arrows, …) to the focused element.
    fn press_key(
        &mut self,
        state: &mut S,
        theme: &Theme,
        font: Option<&Font>,
        input: KeyInput,
    ) -> bool {
        self.send_key(state, theme, font, input)
    }

    fn send_key(
        &mut self,
        state: &mut S,
        theme: &Theme,
        font: Option<&Font>,
        input: KeyInput,
    ) -> bool {
        self.ensure_tree(state, theme, font);
        let Some(id) = self.focused else { return false };
        let ran = self.handlers.dispatch_key(id, &input, state);
        if ran {
            self.dirty = true;
        }
        ran
    }

    /// Move focus to the next focusable element in tree order (wrapping),
    /// like pressing Tab. Returns `true` if focus changed.
    fn focus_next(&mut self, state: &S, theme: &Theme, font: Option<&Font>) -> bool {
        self.ensure_tree(state, theme, font);
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
        self.dirty = true;
        changed
    }

    /// Begin or continue a pointer drag at `pos` (logical pixels). On the first
    /// call it latches onto the draggable element under the cursor; subsequent
    /// calls feed it the pointer's fractional x position until [`App::end_drag`].
    /// Returns `true` if a drag handler ran.
    fn drag_at_point(
        &mut self,
        state: &mut S,
        theme: &Theme,
        font: Option<&Font>,
        pos: Point,
    ) -> bool {
        self.ensure_tree(state, theme, font);
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
        let ran = self.handlers.dispatch_drag(id, fraction, state);
        if ran {
            self.dirty = true;
        }
        ran
    }

    /// End the current drag (pointer released).
    fn end_drag(&mut self) {
        self.dragging = None;
    }

    /// Begin a pointer text interaction at `pos` (logical pixels): if an editable
    /// text element is under the cursor, focus it, resolve the caret byte index
    /// from the pointer x, place the caret there (clearing any selection), and
    /// latch a drag-selection. Returns `true` if a text element was hit.
    fn text_press_at(
        &mut self,
        state: &mut S,
        theme: &Theme,
        font: Option<&Font>,
        pos: Point,
    ) -> bool {
        self.ensure_tree(state, theme, font);
        let Some((id, index, foc)) = self.tree.as_ref().and_then(|t| {
            let (id, node) = text_pos_at(t, pos)?;
            let index = caret_index_at(node, pos, font)?;
            Some((id, index, focus_at(t, pos)))
        }) else {
            return false;
        };
        if foc != self.focused {
            self.focused = foc;
        }
        self.text_selecting = Some(id);
        self.handlers.dispatch_text_pos(id, index, false, state);
        self.dirty = true;
        true
    }

    /// Continue a latched text drag-selection at `pos`: resolve the caret index
    /// from the pointer x and extend the selection to it. Returns `true` if a
    /// selection is active and was updated.
    fn text_drag_at(
        &mut self,
        state: &mut S,
        theme: &Theme,
        font: Option<&Font>,
        pos: Point,
    ) -> bool {
        let Some(id) = self.text_selecting else {
            return false;
        };
        self.ensure_tree(state, theme, font);
        let Some(index) = self
            .tree
            .as_ref()
            .and_then(|t| find_text_pos(t, id))
            .and_then(|node| caret_index_at(node, pos, font))
        else {
            return false;
        };
        let ran = self.handlers.dispatch_text_pos(id, index, true, state);
        if ran {
            self.dirty = true;
        }
        ran
    }

    /// End the current pointer text selection (pointer released).
    fn end_text_select(&mut self) {
        self.text_selecting = None;
    }

    /// Update the hovered element to whatever tappable sits under `pos`.
    /// Returns `true` if the hovered element changed (the UI should repaint).
    fn hover_at(&mut self, state: &S, theme: &Theme, font: Option<&Font>, pos: Point) -> bool {
        self.ensure_tree(state, theme, font);
        let now = self.tree.as_ref().and_then(|t| hit_test(t, pos));
        let changed = now != self.hovered;
        self.hovered = now;
        changed
    }

    /// Scroll the container under the last pointer position by `dy` logical
    /// pixels (positive = reveal content further down). Returns whether anything
    /// scrolled (the offset is re-clamped to the content during the next build).
    fn scroll_by(&mut self, state: &S, theme: &Theme, font: Option<&Font>, dy: f64) -> bool {
        self.ensure_tree(state, theme, font);
        let Some(id) = self
            .tree
            .as_ref()
            .and_then(|t| forma_core::scroll_at(t, self.last_pointer))
        else {
            return false;
        };
        let off = self.scroll_offsets.entry(id).or_insert(0.0);
        let before = *off;
        *off = (*off + dy).max(0.0);
        // A scroll always rebuilds (apply_scroll re-clamps to the content); only
        // report movement when the unclamped offset actually changed.
        let moved = (*off - before).abs() > f64::EPSILON;
        if moved {
            self.dirty = true;
        }
        moved
    }
}

impl<S> App<S> {
    /// Build, paint, and rasterize the primary window's next frame, returning
    /// the [`Pixmap`] and the [`Damage`] (changed region, logical pixels)
    /// relative to the previously returned frame. The first call reports
    /// [`Damage::Full`].
    pub fn render_frame(&mut self) -> (Pixmap, Damage) {
        let App {
            state,
            theme,
            font,
            panes,
            ..
        } = self;
        panes[0].render_frame(state, theme, font.as_ref())
    }

    /// Render a single frame off-screen and return it as a [`Pixmap`]. Needs no
    /// window — used for tests, thumbnails, and golden-image comparisons.
    pub fn render_once(&mut self) -> Pixmap {
        self.render_frame().0
    }

    /// The primary window's currently focused element, if any.
    pub fn focused(&self) -> Option<FocusId> {
        self.panes[0].focused
    }

    /// Build the [accessibility tree](forma_core::accessibility_tree) for the
    /// primary window's current frame — the semantic view a platform AT backend
    /// would expose. Returns `None` until a frame has been built (call
    /// [`render_once`](App::render_once) or any event-routing method first).
    pub fn accessibility_tree(&self) -> Option<forma_core::AccessNode> {
        self.panes[0].accessibility_tree()
    }

    /// Route a completed click at `pos` (logical pixels) to the primary window.
    pub fn click_at(&mut self, pos: Point) -> bool {
        self.pane_click_at(0, pos)
    }

    /// Deliver committed `text` to the primary window's focused element.
    pub fn type_text(&mut self, text: &str) -> bool {
        self.pane_type_text(0, text)
    }

    /// Deliver an editing key to the primary window's focused element.
    pub fn press_key(&mut self, input: KeyInput) -> bool {
        self.pane_press_key(0, input)
    }

    /// Move focus to the next focusable element in the primary window (Tab).
    pub fn focus_next(&mut self) -> bool {
        self.pane_focus_next(0)
    }

    /// Begin or continue a pointer drag at `pos` in the primary window.
    pub fn drag_at_point(&mut self, pos: Point) -> bool {
        self.pane_drag_at_point(0, pos)
    }

    /// End the current drag (pointer released).
    pub fn end_drag(&mut self) {
        self.panes[0].end_drag();
    }

    /// Begin a pointer text interaction at `pos` in the primary window.
    pub fn text_press_at(&mut self, pos: Point) -> bool {
        self.pane_text_press_at(0, pos)
    }

    /// Continue a latched text drag-selection at `pos` in the primary window.
    pub fn text_drag_at(&mut self, pos: Point) -> bool {
        self.pane_text_drag_at(0, pos)
    }

    /// End the current pointer text selection (pointer released).
    pub fn end_text_select(&mut self) {
        self.panes[0].end_text_select();
    }

    /// Update the primary window's hovered element to whatever sits under `pos`.
    pub fn hover_at(&mut self, pos: Point) -> bool {
        self.pane_hover_at(0, pos)
    }

    /// Scroll the container under the last pointer position in the primary
    /// window by `dy` logical pixels.
    pub fn scroll_by(&mut self, dy: f64) -> bool {
        self.pane_scroll_by(0, dy)
    }

    // --- Per-pane event routing (by window index) --------------------------
    // The live multi-window loop routes each event to the pane that owns the
    // window it arrived on; the public methods above are these against pane 0.

    fn pane_click_at(&mut self, idx: usize, pos: Point) -> bool {
        let App {
            state,
            theme,
            font,
            panes,
            ..
        } = self;
        panes[idx].click_at(state, theme, font.as_ref(), pos)
    }

    fn pane_type_text(&mut self, idx: usize, text: &str) -> bool {
        let App {
            state,
            theme,
            font,
            panes,
            ..
        } = self;
        panes[idx].type_text(state, theme, font.as_ref(), text)
    }

    fn pane_press_key(&mut self, idx: usize, input: KeyInput) -> bool {
        let App {
            state,
            theme,
            font,
            panes,
            ..
        } = self;
        panes[idx].press_key(state, theme, font.as_ref(), input)
    }

    fn pane_focus_next(&mut self, idx: usize) -> bool {
        let App {
            state,
            theme,
            font,
            panes,
            ..
        } = self;
        panes[idx].focus_next(state, theme, font.as_ref())
    }

    fn pane_drag_at_point(&mut self, idx: usize, pos: Point) -> bool {
        let App {
            state,
            theme,
            font,
            panes,
            ..
        } = self;
        panes[idx].drag_at_point(state, theme, font.as_ref(), pos)
    }

    fn pane_text_press_at(&mut self, idx: usize, pos: Point) -> bool {
        let App {
            state,
            theme,
            font,
            panes,
            ..
        } = self;
        panes[idx].text_press_at(state, theme, font.as_ref(), pos)
    }

    fn pane_text_drag_at(&mut self, idx: usize, pos: Point) -> bool {
        let App {
            state,
            theme,
            font,
            panes,
            ..
        } = self;
        panes[idx].text_drag_at(state, theme, font.as_ref(), pos)
    }

    fn pane_hover_at(&mut self, idx: usize, pos: Point) -> bool {
        let App {
            state,
            theme,
            font,
            panes,
            ..
        } = self;
        panes[idx].hover_at(state, theme, font.as_ref(), pos)
    }

    fn pane_scroll_by(&mut self, idx: usize, dy: f64) -> bool {
        let App {
            state,
            theme,
            font,
            panes,
            ..
        } = self;
        panes[idx].scroll_by(state, theme, font.as_ref(), dy)
    }

    /// Run the app against the platform backend ([`backend::run`]): native X11
    /// when `$DISPLAY` is set, else a one-shot headless present. Frames are
    /// rendered into each window's [`Surface`]; pointer/keyboard events route
    /// through the same dispatch path used by the headless tests.
    ///
    /// Every window registered with [`open_window`](App::open_window) is opened
    /// as a real OS window on backends that support it (X11 today); each renders
    /// its own pane onto the shared state, and events are routed to the window
    /// they arrived on. Backends without multi-window show only the primary
    /// window. The loop ends when the last window closes.
    pub fn run(mut self) {
        let primary_attrs = self.panes[0].attrs.clone();
        // Per-pane surfaces, created lazily on first present, parallel to panes.
        let mut surfaces: Vec<Option<Box<dyn Surface>>> =
            (0..self.panes.len()).map(|_| None).collect();
        // native window id -> pane index, filled as windows are created.
        let mut id_to_pane: std::collections::HashMap<WindowId, usize> =
            std::collections::HashMap::new();
        let mut open_count = 0usize;
        let mut opened_extras = false;

        // `force` presents the whole frame regardless of computed damage — used
        // for expose/resize, where the window's pixels were lost and a partial
        // update would leave stale or blank regions.
        let present = |app: &mut Self,
                       surfaces: &mut [Option<Box<dyn Surface>>],
                       idx: usize,
                       window: &dyn forma_platform::Window,
                       force: bool| {
            let App {
                state,
                theme,
                font,
                panes,
                ..
            } = app;
            let pane = &mut panes[idx];
            let scene = pane.build_frame(state, theme, font.as_ref());
            let pixmap = pane.rasterize(scene, theme, window.scale_factor());
            let damage = pane.take_damage();
            if !force && damage.is_empty() {
                return; // Nothing changed since the last present.
            }
            let surface = surfaces[idx].get_or_insert_with(|| window.create_surface());
            surface.resize(window.inner_size());
            // Limit the present to the changed region (empty slice = full frame).
            let regions = if force {
                Vec::new()
            } else {
                let bounds = Rect::from_xywh(
                    0.0,
                    0.0,
                    pixmap.size().width as f64,
                    pixmap.size().height as f64,
                );
                damage.to_physical(window.scale_factor(), bounds)
            };
            surface.present(&pixmap, &regions);
        };

        backend::run(primary_attrs, |event, window| {
            let wid = window.id();
            // On the first event we learn the primary window's id; register it
            // and ask the backend to open the remaining panes as OS windows.
            if !opened_extras {
                opened_extras = true;
                id_to_pane.insert(wid, 0);
                open_count = 1;
                for idx in 1..self.panes.len() {
                    let attrs = self.panes[idx].attrs.clone();
                    if let Some(id) = window.open_window(attrs) {
                        id_to_pane.insert(id, idx);
                        open_count += 1;
                    }
                }
            }
            let idx = id_to_pane.get(&wid).copied().unwrap_or(0);
            match event {
                Event::RedrawRequested => {
                    present(&mut self, &mut surfaces, idx, window, true);
                    ControlFlow::Wait
                }
                Event::Resized(size) => {
                    self.panes[idx].attrs.logical_size = window.scale_factor().to_logical(size);
                    self.panes[idx].dirty = true;
                    present(&mut self, &mut surfaces, idx, window, true);
                    ControlFlow::Wait
                }
                Event::PointerButton {
                    state: ButtonState::Pressed,
                    position,
                    ..
                } => {
                    self.panes[idx].pressed = self.panes[idx]
                        .tree
                        .as_ref()
                        .and_then(|t| hit_test(t, position));
                    // Editable text under the cursor starts a click/drag
                    // selection; otherwise latch a drag if a draggable sits there.
                    if self.pane_text_press_at(idx, position)
                        || self.pane_drag_at_point(idx, position)
                    {
                        present(&mut self, &mut surfaces, idx, window, false);
                    }
                    ControlFlow::Wait
                }
                Event::PointerMoved { position } => {
                    self.panes[idx].last_pointer = position;
                    if self.panes[idx].text_selecting.is_some() {
                        if self.pane_text_drag_at(idx, position) {
                            present(&mut self, &mut surfaces, idx, window, false);
                        }
                    } else if self.panes[idx].dragging.is_some() {
                        if self.pane_drag_at_point(idx, position) {
                            present(&mut self, &mut surfaces, idx, window, false);
                        }
                    } else if self.pane_hover_at(idx, position) {
                        present(&mut self, &mut surfaces, idx, window, false);
                    }
                    ControlFlow::Wait
                }
                Event::PointerButton {
                    state: ButtonState::Released,
                    position,
                    ..
                } => {
                    if self.panes[idx].text_selecting.is_some() {
                        self.panes[idx].end_text_select();
                    } else if self.panes[idx].dragging.is_some() {
                        self.panes[idx].end_drag();
                    } else {
                        let down = self.panes[idx].pressed.take();
                        let up = self.panes[idx]
                            .tree
                            .as_ref()
                            .and_then(|t| hit_test(t, position));
                        if down.is_some() && down == up && self.pane_click_at(idx, position) {
                            present(&mut self, &mut surfaces, idx, window, false);
                        }
                    }
                    ControlFlow::Wait
                }
                Event::Text(text) => {
                    if self.pane_type_text(idx, &text) {
                        present(&mut self, &mut surfaces, idx, window, false);
                    }
                    ControlFlow::Wait
                }
                Event::Key {
                    code: KeyCode::Tab,
                    state: ButtonState::Pressed,
                    ..
                } => {
                    if self.pane_focus_next(idx) {
                        present(&mut self, &mut surfaces, idx, window, false);
                    }
                    ControlFlow::Wait
                }
                Event::Key {
                    code,
                    state: ButtonState::Pressed,
                    modifiers,
                } => {
                    if let Some(input) = map_key(code, modifiers) {
                        // Pull the OS clipboard into the mirror before a paste,
                        // and push the mirror to the OS after a copy/cut, so
                        // editing interoperates with other apps (the mirror alone
                        // covers the in-app case + headless).
                        if input == KeyInput::Paste
                            && let Some(text) = window.clipboard()
                        {
                            forma_core::set_clipboard_text(&text);
                        }
                        let writes_clipboard = matches!(input, KeyInput::Copy | KeyInput::Cut);
                        if self.pane_press_key(idx, input) {
                            present(&mut self, &mut surfaces, idx, window, false);
                        }
                        if writes_clipboard {
                            window.set_clipboard(&forma_core::clipboard_text());
                        }
                    }
                    ControlFlow::Wait
                }
                Event::Scroll { delta } => {
                    if self.pane_scroll_by(idx, delta.dy) {
                        present(&mut self, &mut surfaces, idx, window, false);
                    }
                    ControlFlow::Wait
                }
                Event::CloseRequested => {
                    // Close just this window; end the loop only when the last
                    // window goes away.
                    id_to_pane.remove(&wid);
                    open_count = open_count.saturating_sub(1);
                    if open_count == 0 {
                        ControlFlow::Exit
                    } else {
                        window.close_window();
                        ControlFlow::Wait
                    }
                }
                _ => ControlFlow::Wait,
            }
        });
    }
}

/// Translate a platform [`KeyCode`] (plus active `modifiers`) into a core
/// [`KeyInput`] editing command. Character input arrives via [`Event::Text`], so
/// only editing/navigation keys map here. Shift turns caret motions into
/// selection-extending motions; Ctrl/Cmd+A selects all.
fn map_key(code: KeyCode, modifiers: Modifiers) -> Option<KeyInput> {
    let shift = modifiers.shift;
    Some(match code {
        KeyCode::Backspace => KeyInput::Backspace,
        KeyCode::Delete => KeyInput::Delete,
        KeyCode::ArrowLeft if shift => KeyInput::SelectLeft,
        KeyCode::ArrowLeft => KeyInput::Left,
        KeyCode::ArrowRight if shift => KeyInput::SelectRight,
        KeyCode::ArrowRight => KeyInput::Right,
        KeyCode::ArrowUp if shift => KeyInput::SelectUp,
        KeyCode::ArrowUp => KeyInput::Up,
        KeyCode::ArrowDown if shift => KeyInput::SelectDown,
        KeyCode::ArrowDown => KeyInput::Down,
        KeyCode::Home if shift => KeyInput::SelectHome,
        KeyCode::Home => KeyInput::Home,
        KeyCode::End if shift => KeyInput::SelectEnd,
        KeyCode::End => KeyInput::End,
        KeyCode::Char('a') | KeyCode::Char('A') if modifiers.ctrl || modifiers.meta => {
            KeyInput::SelectAll
        }
        KeyCode::Char('c') | KeyCode::Char('C') if modifiers.ctrl || modifiers.meta => {
            KeyInput::Copy
        }
        KeyCode::Char('x') | KeyCode::Char('X') if modifiers.ctrl || modifiers.meta => {
            KeyInput::Cut
        }
        KeyCode::Char('v') | KeyCode::Char('V') if modifiers.ctrl || modifiers.meta => {
            KeyInput::Paste
        }
        KeyCode::Enter => KeyInput::Enter,
        KeyCode::Escape => KeyInput::Escape,
        _ => return None,
    })
}

/// The common imports for building a Forma app.
pub mod prelude {
    pub use crate::App;
    pub use forma_anim::{Easing, Spring, Tween};
    pub use forma_core::{Align, Anchor, Axis, BoxStyle, Cx, Element, KeyInput, OverlaySpec, View};
    pub use forma_geometry::{Insets, Point, Rect, ScaleFactor, Size};
    pub use forma_render::{Color, Font};
    pub use forma_style::Theme;
    pub use forma_style::{Palette, Spacing, Typography};
    pub use forma_widgets::{
        EditBuffer, Variant, button, button_labeled, button_variant, checkbox, column, divider,
        edit_string, heading, label, menu, menu_item, open_dialog, open_menu, panel, paragraph,
        progress_bar, radio, row, scroll, setting_row, slider, spacer, spinner, swatch, switch,
        text_area, text_editor, text_field, tooltip,
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
    fn counter_app() -> App<Counter> {
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

    /// Two stacked 200×40 boxes; tapping the top one flips a flag that only
    /// recolors the *bottom* box. The layout never moves, so a state change
    /// should damage just the bottom box — not the whole window.
    fn damage_app() -> App<bool> {
        use forma_core::BoxStyle;
        use forma_render::Color;
        App::new(false, |flipped: &bool, cx: &mut Cx<bool>| {
            let bottom = if *flipped {
                Color::rgb(200, 0, 0)
            } else {
                Color::rgb(0, 0, 200)
            };
            column(vec![
                Element::boxed(BoxStyle {
                    fill: Some(Color::rgb(20, 20, 20)),
                    radius: 0.0,
                    border: None,
                })
                .width(200.0)
                .height(40.0)
                .on_tap(cx, |f: &mut bool| *f = !*f),
                Element::boxed(BoxStyle {
                    fill: Some(bottom),
                    radius: 0.0,
                    border: None,
                })
                .width(200.0)
                .height(40.0),
            ])
        })
        .logical_size(Size::new(200.0, 80.0))
    }

    #[test]
    fn first_frame_is_full_then_unchanged_is_none() {
        let mut app = damage_app();
        let (_p, d0) = app.render_frame();
        assert_eq!(d0, Damage::Full, "first frame must repaint everything");
        // No state change between frames → nothing to repaint.
        let (_p, d1) = app.render_frame();
        assert_eq!(d1, Damage::None);
        assert!(d1.is_empty());
    }

    #[test]
    fn state_change_damages_only_the_changed_box() {
        let mut app = damage_app();
        let _ = app.render_frame(); // prime the baseline (Full)

        // Tap the top box; only the bottom box's color changes.
        assert!(app.click_at(Point::new(100.0, 20.0)));
        let (_p, d) = app.render_frame();

        let bound = match d {
            Damage::Regions(_) => d.bounding().expect("some region"),
            other => panic!("expected localized regions, got {other:?}"),
        };
        // Damage is confined to the bottom box (y in 40..80), not the full 80px.
        assert!(bound.min_y() >= 40.0, "damage strayed above the bottom box");
        assert!(bound.height() <= 40.0, "damage taller than the bottom box");
        assert!(bound.width() <= 200.0);
    }

    #[derive(Default)]
    struct Form {
        name: String,
    }

    /// A form with a single text field filling the window.
    fn form_app() -> App<Form> {
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

    #[test]
    fn accessibility_tree_reflects_focus() {
        use forma_core::Role;
        // A container root holding a field — the realistic shape (the root is
        // the Window, the field a nested TextField).
        let mut app = App::new(Form::default(), |state: &Form, cx: &mut Cx<Form>| {
            let theme = *cx.theme();
            let field = forma_widgets::text_field(cx, &theme, &state.name, |s: &mut Form, k| {
                forma_widgets::edit_string(&mut s.name, k)
            });
            column(vec![field])
        })
        .logical_size(Size::new(300.0, 100.0));
        app.focus_next(); // focus the field
        let tree = app.accessibility_tree().expect("a frame was built");
        assert_eq!(tree.role, Role::Window);
        // Exactly one node is focused, and it's a text field.
        let focused: Vec<_> = tree
            .descendants()
            .into_iter()
            .filter(|n| n.focused)
            .collect();
        assert_eq!(focused.len(), 1);
        assert_eq!(focused[0].role, Role::TextField);
    }
}
