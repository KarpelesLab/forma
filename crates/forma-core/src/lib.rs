//! Forma's runtime core.
//!
//! Defines the declarative [`View`] trait, the [`Element`] IR that views build,
//! and the layout + paint passes ([`render_view`]) that turn a view into a
//! [`Scene`] ready for `forma-render` to rasterize.
//!
//! # Pipeline
//!
//! `build → layout → paint`, with pointer events routed back through the laid-
//! out tree:
//! - a [`View`] (or an app build closure with a [`Cx`]) produces an [`Element`]
//!   tree, registering `on_tap` handlers in the context;
//! - [`layout`] turns it into a retained [`LayoutNode`] tree;
//! - [`paint`] draws that tree into a [`Scene`], and [`hit_test`] routes
//!   pointer taps to the registered [`Handlers`].
//!
//! Still ahead (ROADMAP.md Phase 1+): fine-grained state and tree-diff
//! reconciliation between frames (today a frame rebuilds the whole tree),
//! keyboard focus traversal, and richer gesture recognition.

#![forbid(unsafe_code)]

mod element;
mod render;
pub mod runtime;

pub use element::{Align, BoxStyle, Element, ElementKind, LayoutStyle, SizeOverride};
pub use render::{layout, measure, paint};
pub use runtime::{ActionId, Cx, Handlers, LayoutNode, NodeContent, hit_test};

// The font type lives in forma-render; re-export so callers of the layout/paint
// passes have one import path for the active font.
pub use forma_render::Font;

// Re-export the layout axis so widget crates speak one vocabulary.
pub use forma_layout::Axis;

use forma_geometry::{Rect, Size};
use forma_render::Scene;
use forma_style::Theme;

/// A piece of UI, described declaratively as a function of theme.
///
/// Implementors return an [`Element`] tree. This is the static-composition
/// entry point; interactive UIs use an app build closure threaded with a
/// [`Cx`] (see the `forma` umbrella crate's `App`) to register handlers.
pub trait View {
    /// Build this view's element tree under the given `theme`.
    fn build(&self, theme: &Theme) -> Element;
}

/// An [`Element`] is itself a (trivial) view.
impl View for Element {
    fn build(&self, _theme: &Theme) -> Element {
        self.clone()
    }
}

/// Build `view`, lay it out to fill `size` logical pixels, and paint it into a
/// fresh [`Scene`]. Text is rendered with `font` (pass `None` to skip text).
/// Interaction handles on the elements are ignored (use [`layout`] +
/// [`hit_test`] directly to route events).
pub fn render_view(view: &impl View, size: Size, theme: &Theme, font: Option<&Font>) -> Scene {
    let element = view.build(theme);
    let tree = layout(
        &element,
        Rect::from_xywh(0.0, 0.0, size.width, size.height),
        font,
    );
    let mut scene = Scene::new(size);
    paint(&tree, &mut scene, font);
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_render::Color;

    #[test]
    fn render_view_paints_a_root_panel() {
        let root = Element::stack(Axis::Vertical, vec![])
            .fill(Color::rgb(10, 20, 30))
            .padding(forma_geometry::Insets::uniform(8.0));
        let scene = render_view(&root, Size::new(100.0, 100.0), &Theme::light(), None);
        assert_eq!(scene.len(), 1);
        assert_eq!(scene.logical_size(), Size::new(100.0, 100.0));
    }
}
