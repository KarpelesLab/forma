//! The reactive runtime: interaction context, retained layout tree, and event
//! dispatch.
//!
//! The piece that turns the static [`Element`](crate::Element) IR into an
//! interactive UI. A [`Cx`] is threaded through the view-building closure so
//! widgets can register `on_tap` handlers; building yields an [`Element`] tree
//! plus a parallel handler table. Laying the tree out produces a retained
//! [`LayoutNode`] tree that [`hit_test`] queries to route pointer events back
//! to the registered handlers.
//!
//! Handlers are kept out of the [`Element`] IR itself (they live in the `Cx`'s
//! table, addressed by [`ActionId`]) so `Element` stays `Clone`/`Debug` and the
//! IR remains diff-friendly for a future reconciler.

use crate::element::BoxStyle;
use forma_geometry::{Point, Rect};
use forma_style::Theme;

/// A boxed event handler that mutates the app state `S`.
type HandlerFn<S> = Box<dyn FnMut(&mut S)>;

/// An opaque handle to a registered event handler, stamped onto the element
/// that owns it and resolved against the [`Cx`] handler table on dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionId(pub(crate) u32);

/// Build context threaded through a view closure.
///
/// Carries the active [`Theme`] and accumulates event handlers. Each
/// registered handler gets an [`ActionId`] the caller stamps onto an element
/// (see [`Element::on_tap`](crate::Element::on_tap)).
pub struct Cx<'a, S> {
    theme: &'a Theme,
    handlers: Vec<HandlerFn<S>>,
}

impl<'a, S> Cx<'a, S> {
    /// Create a context borrowing `theme`.
    pub fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            handlers: Vec::new(),
        }
    }

    /// The active theme.
    pub fn theme(&self) -> &Theme {
        self.theme
    }

    /// Register a handler, returning its [`ActionId`].
    pub fn register(&mut self, handler: impl FnMut(&mut S) + 'static) -> ActionId {
        let id = ActionId(self.handlers.len() as u32);
        self.handlers.push(Box::new(handler));
        id
    }

    /// Consume the context, yielding the accumulated handler table (indexed by
    /// each handler's [`ActionId`]).
    pub fn into_handlers(self) -> Handlers<S> {
        Handlers(self.handlers)
    }
}

impl<S> core::fmt::Debug for Cx<'_, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Cx")
            .field("handlers", &self.handlers.len())
            .finish_non_exhaustive()
    }
}

/// The handler table produced by building a frame. Dispatch resolves an
/// [`ActionId`] (from [`hit_test`]) to its handler and invokes it against the
/// app state.
pub struct Handlers<S>(Vec<HandlerFn<S>>);

impl<S> Handlers<S> {
    /// Invoke the handler for `id` against `state`. Returns `true` if a handler
    /// existed and ran.
    pub fn dispatch(&mut self, id: ActionId, state: &mut S) -> bool {
        if let Some(handler) = self.0.get_mut(id.0 as usize) {
            handler(state);
            true
        } else {
            false
        }
    }

    /// Number of registered handlers.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<S> Default for Handlers<S> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<S> core::fmt::Debug for Handlers<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Handlers")
            .field("len", &self.0.len())
            .finish()
    }
}

/// A laid-out, retained node: absolute bounds, paint decoration, the optional
/// action it routes to, and laid-out children. Produced by
/// [`layout`](crate::layout) and consumed by paint and [`hit_test`].
#[derive(Clone, Debug)]
pub struct LayoutNode {
    pub bounds: Rect,
    pub decoration: BoxStyle,
    pub action: Option<ActionId>,
    pub children: Vec<LayoutNode>,
}

/// Find the [`ActionId`] of the top-most actionable node containing `point`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use forma_geometry::Size;

    fn leaf(bounds: Rect, action: Option<ActionId>) -> LayoutNode {
        LayoutNode {
            bounds,
            decoration: BoxStyle::default(),
            action,
            children: Vec::new(),
        }
    }

    #[test]
    fn dispatch_invokes_registered_handler() {
        let theme = Theme::light();
        let mut cx = Cx::new(&theme);
        let id = cx.register(|n: &mut i32| *n += 5);
        let mut handlers = cx.into_handlers();
        let mut state = 0;
        assert!(handlers.dispatch(id, &mut state));
        assert_eq!(state, 5);
        assert!(!handlers.dispatch(ActionId(99), &mut state));
    }

    #[test]
    fn hit_test_prefers_topmost_child() {
        let root = LayoutNode {
            bounds: Rect::from_xywh(0.0, 0.0, 100.0, 100.0),
            decoration: BoxStyle::default(),
            action: Some(ActionId(0)),
            children: vec![
                leaf(Rect::from_xywh(10.0, 10.0, 30.0, 30.0), Some(ActionId(1))),
                leaf(Rect::from_xywh(20.0, 20.0, 30.0, 30.0), Some(ActionId(2))),
            ],
        };
        // Overlap region (25,25) belongs to the later (top) child: action 2.
        assert_eq!(hit_test(&root, Point::new(25.0, 25.0)), Some(ActionId(2)));
        // Only first child.
        assert_eq!(hit_test(&root, Point::new(12.0, 12.0)), Some(ActionId(1)));
        // Background (root) action.
        assert_eq!(hit_test(&root, Point::new(90.0, 90.0)), Some(ActionId(0)));
        // Outside everything.
        let _ = Size::ZERO;
    }
}
