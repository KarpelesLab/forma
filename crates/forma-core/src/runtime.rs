//! The reactive runtime: interaction context, retained layout tree, and event
//! dispatch.
//!
//! Turns the static [`Element`](crate::Element) IR into an interactive UI. A
//! [`Cx`] is threaded through the view-building closure so widgets can register
//! `on_tap` (pointer) and `on_key` (keyboard/focus) handlers; building yields
//! an [`Element`] tree plus a parallel [`Handlers`] table. Laying the tree out
//! produces a retained [`LayoutNode`] tree that [`hit_test`] (pointer) and
//! [`focus_at`] / [`collect_focusables`] (keyboard focus) query to route events
//! back to the registered handlers.
//!
//! Handlers stay out of the [`Element`] IR (they live in the `Cx` tables,
//! addressed by [`ActionId`] / [`FocusId`]) so `Element` stays `Clone`/`Debug`
//! and the IR remains diff-friendly for a future reconciler.

use crate::element::BoxStyle;
use forma_geometry::{Point, Rect};
use forma_render::Color;
use forma_style::Theme;

/// A boxed pointer-tap handler that mutates the app state `S`.
type TapFn<S> = Box<dyn FnMut(&mut S)>;
/// A boxed keyboard handler: receives the [`KeyInput`] for the focused element.
type KeyFn<S> = Box<dyn FnMut(&mut S, &KeyInput)>;
/// A boxed drag handler: receives the pointer position as a fraction (0..=1)
/// along the element's width.
type DragFn<S> = Box<dyn FnMut(&mut S, f64)>;

/// An opaque handle to a registered tap handler, stamped onto the element that
/// owns it and resolved against the [`Cx`] tap table on dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionId(pub(crate) u32);

/// An opaque handle to a focusable element with a registered key handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FocusId(pub(crate) u32);

/// An opaque handle to an element with a registered drag handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DragId(pub(crate) u32);

/// A platform-neutral keyboard input, delivered to the focused element. The
/// app/platform layer translates raw key events into these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyInput {
    /// Committed text (one or more characters), e.g. from a key press or IME.
    Text(String),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Enter,
    Escape,
}

/// Build context threaded through a view closure.
///
/// Carries the active [`Theme`] and accumulates event handlers. Registering a
/// handler returns an id the caller stamps onto an element (see
/// [`Element::on_tap`](crate::Element::on_tap) /
/// [`Element::on_key`](crate::Element::on_key)).
pub struct Cx<'a, S> {
    theme: &'a Theme,
    taps: Vec<TapFn<S>>,
    keys: Vec<KeyFn<S>>,
    drags: Vec<DragFn<S>>,
}

impl<'a, S> Cx<'a, S> {
    /// Create a context borrowing `theme`.
    pub fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            taps: Vec::new(),
            keys: Vec::new(),
            drags: Vec::new(),
        }
    }

    /// The active theme.
    pub fn theme(&self) -> &Theme {
        self.theme
    }

    /// Register a pointer-tap handler, returning its [`ActionId`].
    pub fn register(&mut self, handler: impl FnMut(&mut S) + 'static) -> ActionId {
        let id = ActionId(self.taps.len() as u32);
        self.taps.push(Box::new(handler));
        id
    }

    /// Register a keyboard handler for a focusable element, returning its
    /// [`FocusId`].
    pub fn register_key(&mut self, handler: impl FnMut(&mut S, &KeyInput) + 'static) -> FocusId {
        let id = FocusId(self.keys.len() as u32);
        self.keys.push(Box::new(handler));
        id
    }

    /// Register a drag handler, returning its [`DragId`]. The handler receives
    /// the pointer's fractional x position (0..=1) across the element.
    pub fn register_drag(&mut self, handler: impl FnMut(&mut S, f64) + 'static) -> DragId {
        let id = DragId(self.drags.len() as u32);
        self.drags.push(Box::new(handler));
        id
    }

    /// Consume the context, yielding the accumulated [`Handlers`] table.
    pub fn into_handlers(self) -> Handlers<S> {
        Handlers {
            taps: self.taps,
            keys: self.keys,
            drags: self.drags,
        }
    }
}

impl<S> core::fmt::Debug for Cx<'_, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Cx")
            .field("taps", &self.taps.len())
            .field("keys", &self.keys.len())
            .field("drags", &self.drags.len())
            .finish_non_exhaustive()
    }
}

/// The handler tables produced by building a frame. Dispatch resolves an
/// [`ActionId`] / [`FocusId`] (from [`hit_test`] / [`focus_at`]) to its handler
/// and invokes it against the app state.
pub struct Handlers<S> {
    taps: Vec<TapFn<S>>,
    keys: Vec<KeyFn<S>>,
    drags: Vec<DragFn<S>>,
}

impl<S> Handlers<S> {
    /// Invoke the tap handler for `id`. Returns `true` if one existed and ran.
    pub fn dispatch(&mut self, id: ActionId, state: &mut S) -> bool {
        if let Some(handler) = self.taps.get_mut(id.0 as usize) {
            handler(state);
            true
        } else {
            false
        }
    }

    /// Invoke the key handler for focused element `id` with `input`. Returns
    /// `true` if one existed and ran.
    pub fn dispatch_key(&mut self, id: FocusId, input: &KeyInput, state: &mut S) -> bool {
        if let Some(handler) = self.keys.get_mut(id.0 as usize) {
            handler(state, input);
            true
        } else {
            false
        }
    }

    /// Invoke the drag handler for `id` with `fraction` (0..=1 across the
    /// element width). Returns `true` if one existed and ran.
    pub fn dispatch_drag(&mut self, id: DragId, fraction: f64, state: &mut S) -> bool {
        if let Some(handler) = self.drags.get_mut(id.0 as usize) {
            handler(state, fraction);
            true
        } else {
            false
        }
    }

    /// Total number of registered handlers (taps + keys + drags).
    pub fn len(&self) -> usize {
        self.taps.len() + self.keys.len() + self.drags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.taps.is_empty() && self.keys.is_empty() && self.drags.is_empty()
    }
}

impl<S> Default for Handlers<S> {
    fn default() -> Self {
        Self {
            taps: Vec::new(),
            keys: Vec::new(),
            drags: Vec::new(),
        }
    }
}

impl<S> core::fmt::Debug for Handlers<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Handlers")
            .field("taps", &self.taps.len())
            .field("keys", &self.keys.len())
            .field("drags", &self.drags.len())
            .finish()
    }
}

/// Paintable leaf content carried by a [`LayoutNode`] beyond its decoration.
#[derive(Clone, Debug, Default)]
pub enum NodeContent {
    /// Decoration only (the common case).
    #[default]
    None,
    /// A single line of text, painted at the node's bounds origin.
    Text {
        text: String,
        size: f64,
        color: Color,
    },
}

/// A laid-out, retained node: absolute bounds, paint decoration, optional text
/// content, the optional tap/focus handles it routes to, and laid-out children.
/// Produced by [`layout`](crate::layout) and consumed by paint, [`hit_test`],
/// and the focus queries.
#[derive(Clone, Debug)]
pub struct LayoutNode {
    pub bounds: Rect,
    pub decoration: BoxStyle,
    pub content: NodeContent,
    pub action: Option<ActionId>,
    pub focus: Option<FocusId>,
    pub drag: Option<DragId>,
    pub children: Vec<LayoutNode>,
}

/// Find the [`ActionId`] of the top-most tappable node containing `point`.
///
/// Children are painted after (on top of) their parent, so they are tested
/// first, last-to-first, mirroring paint order.
pub fn hit_test(node: &LayoutNode, point: Point) -> Option<ActionId> {
    for child in node.children.iter().rev() {
        if let Some(id) = hit_test(child, point) {
            return Some(id);
        }
    }
    if node.action.is_some() && node.bounds.contains(point) {
        node.action
    } else {
        None
    }
}

/// Find the [`FocusId`] of the top-most focusable node containing `point`
/// (used for click-to-focus).
pub fn focus_at(node: &LayoutNode, point: Point) -> Option<FocusId> {
    for child in node.children.iter().rev() {
        if let Some(id) = focus_at(child, point) {
            return Some(id);
        }
    }
    if node.focus.is_some() && node.bounds.contains(point) {
        node.focus
    } else {
        None
    }
}

/// Find the top-most draggable node containing `point`, returning its
/// [`DragId`] and bounds (so the caller can compute the drag fraction).
pub fn drag_at(node: &LayoutNode, point: Point) -> Option<(DragId, Rect)> {
    for child in node.children.iter().rev() {
        if let Some(hit) = drag_at(child, point) {
            return Some(hit);
        }
    }
    match node.drag {
        Some(id) if node.bounds.contains(point) => Some((id, node.bounds)),
        _ => None,
    }
}

/// Find the node carrying focus `id`, if present.
pub fn find_focus(node: &LayoutNode, id: FocusId) -> Option<&LayoutNode> {
    if node.focus == Some(id) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_focus(c, id))
}

/// Find the node carrying tap-action `id`, if present (for hover highlight).
pub fn find_action(node: &LayoutNode, id: ActionId) -> Option<&LayoutNode> {
    if node.action == Some(id) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_action(c, id))
}

/// The first text content `(text, size, bounds)` at or under `node`, in tree
/// order. Used to position a caret inside a focused text field.
pub fn first_text(node: &LayoutNode) -> Option<(&str, f64, Rect)> {
    if let NodeContent::Text { text, size, .. } = &node.content {
        return Some((text, *size, node.bounds));
    }
    node.children.iter().find_map(first_text)
}

/// Collect every focusable [`FocusId`] in paint/tree order, for Tab traversal.
pub fn collect_focusables(node: &LayoutNode, out: &mut Vec<FocusId>) {
    if let Some(id) = node.focus {
        out.push(id);
    }
    for child in &node.children {
        collect_focusables(child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(bounds: Rect, action: Option<ActionId>, focus: Option<FocusId>) -> LayoutNode {
        LayoutNode {
            bounds,
            decoration: BoxStyle::default(),
            content: NodeContent::None,
            action,
            focus,
            drag: None,
            children: Vec::new(),
        }
    }

    #[derive(Default)]
    struct St {
        n: i32,
        s: String,
    }

    #[test]
    fn dispatch_tap_and_key() {
        let theme = Theme::light();
        let mut cx = Cx::new(&theme);
        let tap = cx.register(|st: &mut St| st.n += 5);
        let focus = cx.register_key(|st: &mut St, k: &KeyInput| {
            if let KeyInput::Text(t) = k {
                st.s.push_str(t);
            }
        });
        let mut handlers = cx.into_handlers();

        let mut st = St::default();
        assert!(handlers.dispatch(tap, &mut st));
        assert_eq!(st.n, 5);

        assert!(handlers.dispatch_key(focus, &KeyInput::Text("hi".into()), &mut st));
        assert_eq!(st.s, "hi");
        assert!(!handlers.dispatch_key(FocusId(99), &KeyInput::Backspace, &mut st));
    }

    #[test]
    fn hit_test_and_focus_prefer_topmost() {
        let root = LayoutNode {
            bounds: Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
            decoration: BoxStyle::default(),
            content: NodeContent::None,
            action: Some(ActionId(0)),
            focus: None,
            drag: None,
            children: vec![
                leaf(
                    Rect::from_xywh(10.0, 10.0, 30.0, 30.0),
                    Some(ActionId(1)),
                    Some(FocusId(0)),
                ),
                leaf(
                    Rect::from_xywh(20.0, 20.0, 30.0, 30.0),
                    Some(ActionId(2)),
                    Some(FocusId(1)),
                ),
            ],
        };
        assert_eq!(hit_test(&root, Point::new(25.0, 25.0)), Some(ActionId(2)));
        assert_eq!(focus_at(&root, Point::new(12.0, 12.0)), Some(FocusId(0)));

        let mut focusables = Vec::new();
        collect_focusables(&root, &mut focusables);
        assert_eq!(focusables, vec![FocusId(0), FocusId(1)]);
    }
}
