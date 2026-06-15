//! Forma's self-drawn widget library.
//!
//! Every widget is composed from the `forma-core` [`Element`] IR and drawn by
//! `oxideav-raster` — there are no native controls. The builders here read
//! their colors and metrics from a [`Theme`], so a single theme swap re-skins
//! the whole UI.
//!
//! Available widgets: structure (`column`, `row`, `spacer`, `panel`),
//! content (`label`, `swatch`, `divider`), and interactive controls
//! (`button`, `button_labeled` with [`Element::on_tap`], and `text_field` with
//! [`Element::on_key`] + [`edit_string`]). Hover states, multi-line/caret text
//! editing, and richer controls (checkbox, slider, …) are still to come.

#![forbid(unsafe_code)]

use forma_core::{Align, Axis, BoxStyle, Cx, Element, KeyInput};
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

/// A primary action surface (the visual basis of an icon-only button). Fixed
/// size; for a text button use [`button_labeled`].
pub fn button(theme: &Theme) -> Element {
    Element::boxed(BoxStyle {
        fill: Some(theme.palette.primary),
        radius: theme.radius,
        border: None,
    })
    .width(96.0)
    .height(36.0)
}

/// A single line of text in the theme's default text color and base size.
pub fn label(theme: &Theme, text: impl Into<String>) -> Element {
    Element::text(text, theme.font_size, theme.palette.text)
}

/// A primary button with a centered text `label`. Sizes to the label plus
/// padding (intrinsic sizing via the active font's measurement).
pub fn button_labeled(theme: &Theme, label: impl Into<String>) -> Element {
    let text = Element::text(label, theme.font_size, theme.palette.on_primary);
    Element::stack(Axis::Horizontal, vec![text])
        .fill(theme.palette.primary)
        .radius(theme.radius)
        .padding(Insets::symmetric(theme.spacing.lg, theme.spacing.sm))
        .align(Align::Center, Align::Center)
}

/// Apply a [`KeyInput`] to an editable string: append committed text, or
/// remove the last character on backspace. Navigation keys are ignored (no
/// caret model yet). Handy as the body of a [`text_field`] handler.
pub fn edit_string(value: &mut String, input: &KeyInput) {
    match input {
        KeyInput::Text(t) => value.push_str(t),
        KeyInput::Backspace => {
            value.pop();
        }
        _ => {}
    }
}

/// A single-line editable text field showing `value` and routing keyboard
/// input to `on_key` while focused (pair with [`edit_string`]). Default width
/// 200 logical px; override with `.width(..)` on the returned element.
pub fn text_field<S>(
    cx: &mut Cx<S>,
    theme: &Theme,
    value: &str,
    on_key: impl FnMut(&mut S, &KeyInput) + 'static,
) -> Element {
    // A leading space keeps an empty field from collapsing to zero height.
    let shown = if value.is_empty() {
        String::from(" ")
    } else {
        value.to_string()
    };
    Element::stack(
        Axis::Horizontal,
        vec![Element::text(shown, theme.font_size, theme.palette.text)],
    )
    .fill(theme.palette.surface)
    .radius(theme.radius)
    .border(theme.palette.border, 1.0)
    .padding(Insets::symmetric(theme.spacing.md, theme.spacing.sm))
    .align(Align::Start, Align::Center)
    .width(200.0)
    .on_key(cx, on_key)
}

/// A toggleable checkbox: a small square showing a check mark when `checked`,
/// calling `on_toggle` when tapped.
pub fn checkbox<S>(
    cx: &mut Cx<S>,
    theme: &Theme,
    checked: bool,
    on_toggle: impl FnMut(&mut S) + 'static,
) -> Element {
    let mark = if checked {
        vec![Element::text(
            "✓",
            theme.font_size,
            theme.palette.on_primary,
        )]
    } else {
        Vec::new()
    };
    Element::stack(Axis::Horizontal, mark)
        .fill(if checked {
            theme.palette.primary
        } else {
            theme.palette.surface
        })
        .border(theme.palette.border, 1.0)
        .radius(theme.radius / 2.0)
        .width(22.0)
        .height(22.0)
        .align(Align::Center, Align::Center)
        .on_tap(cx, on_toggle)
}

/// An on/off switch: a pill track with a knob that sits right when `on`
/// (track tinted with the primary color) and left when off. Tapping calls
/// `on_toggle`.
pub fn switch<S>(
    cx: &mut Cx<S>,
    theme: &Theme,
    on: bool,
    on_toggle: impl FnMut(&mut S) + 'static,
) -> Element {
    let knob = Element::boxed(BoxStyle {
        fill: Some(Color::WHITE),
        radius: 8.0,
        border: None,
    })
    .width(16.0)
    .height(16.0);
    let children = if on {
        vec![spacer(), knob]
    } else {
        vec![knob, spacer()]
    };
    Element::stack(Axis::Horizontal, children)
        .fill(if on {
            theme.palette.primary
        } else {
            theme.palette.border
        })
        .radius(11.0)
        .width(40.0)
        .height(22.0)
        .padding(Insets::uniform(3.0))
        .align(Align::Start, Align::Center)
        .on_tap(cx, on_toggle)
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
            None,
        );
        assert_eq!(scene.len(), 2);
    }
}
