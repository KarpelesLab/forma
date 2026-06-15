//! Forma's runtime core.
//!
//! Defines the declarative [`View`] trait, the [`Element`] IR that views build,
//! and the layout + paint passes ([`render_view`]) that turn a view into a
//! [`Scene`] ready for `forma-render` to rasterize.
//!
//! # What's here vs. what's next
//!
//! The scaffold implements the **build → layout → paint** pipeline end to end:
//! a `View` produces an `Element` tree, [`render_view`] lays it out under a set
//! of bounds and paints it. The **reactive half** — fine-grained state,
//! tree-diff/reconcile between frames, event dispatch + hit-testing, and focus
//! — is the next milestone (ROADMAP.md Phase 1, "forma-core reactivity MVP")
//! and will layer on top of this IR without replacing it.

#![forbid(unsafe_code)]

mod element;
mod render;

pub use element::{Align, BoxStyle, Element, ElementKind, LayoutStyle, SizeOverride};
pub use render::{measure, place};

// Re-export the layout axis so widget crates speak one vocabulary.
pub use forma_layout::Axis;

use forma_geometry::{Rect, Size};
use forma_render::Scene;
use forma_style::Theme;

/// A piece of UI, described declaratively as a function of theme (and, in the
/// reactive milestone, state).
///
/// Implementors return an [`Element`] tree. Widgets in `forma-widgets`
/// implement this; composite views implement it by composing child views.
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
/// fresh [`Scene`].
pub fn render_view(view: &impl View, size: Size, theme: &Theme) -> Scene {
    let element = view.build(theme);
    let mut scene = Scene::new(size);
    place(
        &element,
        Rect::from_xywh(0.0, 0.0, size.width, size.height),
        &mut scene,
    );
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
        let scene = render_view(&root, Size::new(100.0, 100.0), &Theme::light());
        assert_eq!(scene.len(), 1);
        assert_eq!(scene.logical_size(), Size::new(100.0, 100.0));
    }
}
