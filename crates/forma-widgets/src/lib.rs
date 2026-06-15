//! Forma's self-drawn widget library.
//!
//! Every widget is composed from the `forma-core` [`Element`] IR and drawn by
//! `oxideav-raster` — there are no native controls. The builders here read
//! their colors and metrics from a [`Theme`], so a single theme swap re-skins
//! the whole UI.
//!
//! These are the structural and surface widgets available in the scaffold
//! (panels, rows/columns, buttons, dividers, swatches, spacers). **Text** and
//! **interaction** (`on_tap`, hover, focus) are intentionally absent here: text
//! needs the `oxideav-scribe` shaping bridge and interaction needs the
//! `forma-core` reactive runtime — both are the next roadmap milestones. Until
//! then a "button" is a styled surface, not yet a clickable control.

#![forbid(unsafe_code)]

use forma_core::{Align, Axis, BoxStyle, Element};
use forma_geometry::Insets;
use forma_render::Color;
use forma_style::Theme;

/// A vertical container.
pub fn column(children: Vec<Element>) -> Element {
    Element::stack(Axis::Vertical, children)
}

/// A horizontal container.
pub fn row(children: Vec<Element>) -> Element {
    Element::stack(Axis::Horizontal, children)
}

/// A flexible gap that pushes siblings apart (grow = 1).
pub fn spacer() -> Element {
    Element::stack(Axis::Horizontal, Vec::new()).grow(1.0)
}

/// A raised surface card: themed fill, hairline border, rounded corners, and
/// comfortable padding. Lays its `children` vertically.
pub fn panel(theme: &Theme, children: Vec<Element>) -> Element {
    Element::stack(Axis::Vertical, children)
        .fill(theme.palette.surface)
        .radius(theme.radius)
        .border(theme.palette.border, 1.0)
        .padding(Insets::uniform(theme.spacing.lg))
        .gap(theme.spacing.md)
        // Stretch children across the card width so rows and dividers fill it.
        .align(Align::Start, Align::Stretch)
}

/// A primary action surface (the visual basis of a button). Fixed size for
/// now; intrinsic sizing arrives with text layout.
pub fn button(theme: &Theme) -> Element {
    Element::boxed(BoxStyle {
        fill: Some(theme.palette.primary),
        radius: theme.radius,
        border: None,
    })
    .width(96.0)
    .height(36.0)
}

/// A 1px themed divider line spanning the cross axis.
pub fn divider(theme: &Theme) -> Element {
    Element::boxed(BoxStyle {
        fill: Some(theme.palette.border),
        radius: 0.0,
        border: None,
    })
    .height(1.0)
}

/// A small square color sample with rounded corners.
pub fn swatch(color: Color, size: f64, radius: f64) -> Element {
    Element::boxed(BoxStyle {
        fill: Some(color),
        radius,
        border: None,
    })
    .width(size)
    .height(size)
}

/// A horizontal row with a leading swatch, a flexible spacer, and a trailing
/// button — a compact "setting row" used to show real composition.
pub fn setting_row(theme: &Theme, accent: Color) -> Element {
    row(vec![
        swatch(accent, 24.0, theme.radius / 2.0),
        spacer(),
        button(theme),
    ])
    .gap(theme.spacing.md)
    .align(Align::Start, Align::Center)
    .height(40.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_core::{ElementKind, render_view};
    use forma_geometry::Size;

    #[test]
    fn panel_is_a_decorated_vertical_stack() {
        let p = panel(&Theme::light(), vec![divider(&Theme::light())]);
        assert!(p.decoration.fill.is_some());
        assert!(matches!(
            p.kind,
            ElementKind::Stack {
                axis: Axis::Vertical,
                ..
            }
        ));
    }

    #[test]
    fn setting_row_paints_swatch_and_button() {
        // Two filled leaves (swatch + button); the spacer is empty.
        let theme = Theme::light();
        let scene = render_view(
            &setting_row(&theme, Color::rgb(200, 80, 80)),
            Size::new(300.0, 40.0),
            &theme,
        );
        assert_eq!(scene.len(), 2);
    }
}
