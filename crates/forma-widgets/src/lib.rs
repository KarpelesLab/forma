//! Forma's self-drawn widget library.
//!
//! Every widget is composed from the `forma-core` [`Element`] IR and drawn by
//! `oxideav-raster` — there are no native controls. The builders here read
//! their colors and metrics from a [`Theme`], so a single theme swap re-skins
//! the whole UI.
//!
//! Available widgets: structure (`column`, `row`, `spacer`, `panel`), content
//! (`label`, `heading`, `swatch`, `divider`), and interactive controls —
//! `button_variant` (Primary/Secondary/Ghost/Danger) / `button_labeled`
//! ([`Element::on_tap`]), `text_field` / `text_editor` ([`Element::on_key`]; the
//! latter is caret-aware via [`EditBuffer`]), `checkbox`, `switch`, and `slider`
//! ([`Element::on_drag`]). All colors and metrics come from the [`Theme`], so
//! swapping or customizing the theme reskins everything.

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

/// A single line of text in the theme's default text color and body size.
pub fn label(theme: &Theme, text: impl Into<String>) -> Element {
    Element::text(text, theme.typography.body, theme.palette.text)
}

/// A larger heading in the theme's heading size.
pub fn heading(theme: &Theme, text: impl Into<String>) -> Element {
    Element::text(text, theme.typography.heading, theme.palette.text)
}

/// Visual emphasis level for a button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Filled accent — the primary action.
    Primary,
    /// Outlined surface — a secondary action.
    Secondary,
    /// Text-only, no fill — a low-emphasis action.
    Ghost,
    /// Filled danger color — a destructive action.
    Danger,
}

/// A text button styled per [`Variant`], reading colors from the theme.
pub fn button_variant(theme: &Theme, label: impl Into<String>, variant: Variant) -> Element {
    let p = &theme.palette;
    let (fill, ink, border): (Option<Color>, Color, Option<Color>) = match variant {
        Variant::Primary => (Some(p.primary), p.on_primary, None),
        Variant::Secondary => (Some(p.surface), p.text, Some(p.border)),
        Variant::Ghost => (None, p.primary, None),
        Variant::Danger => (Some(p.danger), p.danger.on_color(), None),
    };
    let mut el = Element::stack(
        Axis::Horizontal,
        vec![Element::text(label, theme.typography.body, ink)],
    )
    .radius(theme.radius)
    .padding(Insets::symmetric(theme.spacing.lg, theme.spacing.sm))
    .align(Align::Center, Align::Center);
    if let Some(fill) = fill {
        el = el.fill(fill);
    }
    if let Some(border) = border {
        el = el.border(border, 1.0);
    }
    el
}

/// A primary button with a centered text `label` (shorthand for
/// [`button_variant`] with [`Variant::Primary`]). Sizes to the label plus
/// padding (intrinsic sizing via the active font's measurement).
pub fn button_labeled(theme: &Theme, label: impl Into<String>) -> Element {
    button_variant(theme, label, Variant::Primary)
}

/// Apply a [`KeyInput`] to an editable string: append committed text, or
/// remove the last character on backspace. Navigation keys are ignored (no
/// caret model). For caret-aware editing use an [`EditBuffer`] with
/// [`text_editor`] instead.
pub fn edit_string(value: &mut String, input: &KeyInput) {
    match input {
        KeyInput::Text(t) => value.push_str(t),
        KeyInput::Backspace => {
            value.pop();
        }
        _ => {}
    }
}

/// A single-line text buffer with a caret, for in-place editing.
///
/// The caret is a byte index into the text, always kept on a UTF-8 char
/// boundary. Text is inserted and deleted at the caret; the arrow / Home / End
/// keys move it. Drive it by feeding [`KeyInput`]s to [`EditBuffer::apply`], and
/// render it (with a positioned caret) via [`text_editor`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditBuffer {
    text: String,
    caret: usize,
}

impl EditBuffer {
    /// An empty buffer with the caret at the start.
    pub fn new() -> Self {
        Self::default()
    }

    /// A buffer holding `text`, caret placed at the end.
    pub fn from_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let caret = text.len();
        Self { text, caret }
    }

    /// The current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The caret position as a byte index into [`text`](Self::text).
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// `true` if there is no text.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Insert `s` at the caret and advance the caret past it.
    pub fn insert(&mut self, s: &str) {
        self.text.insert_str(self.caret, s);
        self.caret += s.len();
    }

    /// Delete the character before the caret (Backspace); no-op at the start.
    pub fn backspace(&mut self) {
        if let Some(prev) = self.prev_boundary(self.caret) {
            self.text.replace_range(prev..self.caret, "");
            self.caret = prev;
        }
    }

    /// Delete the character at the caret (Delete); no-op at the end.
    pub fn delete(&mut self) {
        if let Some(next) = self.next_boundary(self.caret) {
            self.text.replace_range(self.caret..next, "");
        }
    }

    /// Move the caret one character left.
    pub fn move_left(&mut self) {
        if let Some(prev) = self.prev_boundary(self.caret) {
            self.caret = prev;
        }
    }

    /// Move the caret one character right.
    pub fn move_right(&mut self) {
        if let Some(next) = self.next_boundary(self.caret) {
            self.caret = next;
        }
    }

    /// Move the caret to the start.
    pub fn home(&mut self) {
        self.caret = 0;
    }

    /// Move the caret to the end.
    pub fn end(&mut self) {
        self.caret = self.text.len();
    }

    /// Apply a [`KeyInput`]: insert text, delete at the caret, or move it.
    /// Enter/Escape are ignored (single-line).
    pub fn apply(&mut self, input: &KeyInput) {
        match input {
            KeyInput::Text(t) => self.insert(t),
            KeyInput::Backspace => self.backspace(),
            KeyInput::Delete => self.delete(),
            KeyInput::Left => self.move_left(),
            KeyInput::Right => self.move_right(),
            KeyInput::Home => self.home(),
            KeyInput::End => self.end(),
            KeyInput::Enter | KeyInput::Escape => {}
        }
    }

    /// The char boundary strictly before `i`, or `None` at the start.
    fn prev_boundary(&self, i: usize) -> Option<usize> {
        if i == 0 {
            return None;
        }
        let mut p = i - 1;
        while !self.text.is_char_boundary(p) {
            p -= 1;
        }
        Some(p)
    }

    /// The char boundary strictly after `i`, or `None` at the end.
    fn next_boundary(&self, i: usize) -> Option<usize> {
        if i >= self.text.len() {
            return None;
        }
        let mut n = i + 1;
        while n < self.text.len() && !self.text.is_char_boundary(n) {
            n += 1;
        }
        Some(n)
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

/// A single-line editable field backed by an [`EditBuffer`], rendering the text
/// with the caret at the buffer's caret position (shown while focused). Route
/// keyboard input to `on_key`, whose body typically calls
/// [`EditBuffer::apply`]. Default width 200 logical px; override with
/// `.width(..)` on the returned element.
pub fn text_editor<S>(
    cx: &mut Cx<S>,
    theme: &Theme,
    buffer: &EditBuffer,
    on_key: impl FnMut(&mut S, &KeyInput) + 'static,
) -> Element {
    // A leading space keeps an empty field from collapsing to zero height; the
    // caret then sits at index 0 of that placeholder.
    let shown = if buffer.is_empty() {
        String::from(" ")
    } else {
        buffer.text().to_string()
    };
    let text = Element::text(shown, theme.font_size, theme.palette.text).caret(buffer.caret());
    Element::stack(Axis::Horizontal, vec![text])
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

/// A horizontal slider with `value` in 0..=1. Dragging anywhere on the track
/// calls `on_change` with the new fractional position. Default width 160 px;
/// override with `.width(..)`.
pub fn slider<S>(
    cx: &mut Cx<S>,
    theme: &Theme,
    value: f64,
    on_change: impl FnMut(&mut S, f64) + 'static,
) -> Element {
    let v = value.clamp(0.0, 1.0);
    let knob = Element::boxed(BoxStyle {
        fill: Some(theme.palette.primary),
        radius: 8.0,
        border: None,
    })
    .width(16.0)
    .height(16.0);
    Element::stack(
        Axis::Horizontal,
        vec![spacer().grow(v), knob, spacer().grow(1.0 - v)],
    )
    .fill(theme.palette.border)
    .radius(11.0)
    .width(160.0)
    .height(22.0)
    .padding(Insets::symmetric(3.0, 3.0))
    .align(Align::Start, Align::Center)
    .on_drag(cx, on_change)
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

    #[test]
    fn edit_buffer_inserts_and_moves_caret() {
        let mut b = EditBuffer::new();
        b.insert("ab");
        assert_eq!((b.text(), b.caret()), ("ab", 2));
        b.move_left();
        assert_eq!(b.caret(), 1);
        b.insert("X"); // insert mid-string at the caret
        assert_eq!((b.text(), b.caret()), ("aXb", 2));
        b.home();
        assert_eq!(b.caret(), 0);
        b.end();
        assert_eq!(b.caret(), 3);
    }

    #[test]
    fn edit_buffer_backspace_and_delete_at_caret() {
        let mut b = EditBuffer::from_text("abc");
        assert_eq!(b.caret(), 3); // from_text places caret at end
        b.backspace();
        assert_eq!((b.text(), b.caret()), ("ab", 2));
        b.home();
        b.delete();
        assert_eq!((b.text(), b.caret()), ("b", 0));
        // Deletes/backspaces at the edges are no-ops.
        b.backspace();
        assert_eq!(b.text(), "b");
        b.end();
        b.delete();
        assert_eq!(b.text(), "b");
    }

    #[test]
    fn edit_buffer_respects_utf8_boundaries() {
        // "é" and "🦀" are multi-byte; caret motion must land on boundaries.
        let mut b = EditBuffer::from_text("é🦀");
        assert_eq!(b.caret(), "é🦀".len());
        b.move_left(); // skip the whole crab
        assert_eq!(b.caret(), "é".len());
        b.move_left();
        assert_eq!(b.caret(), 0);
        b.delete(); // removes "é", not a partial byte
        assert_eq!(b.text(), "🦀");
    }

    #[test]
    fn edit_buffer_apply_dispatches_keys() {
        let mut b = EditBuffer::new();
        for k in [
            KeyInput::Text("hi".into()),
            KeyInput::Left,
            KeyInput::Text("X".into()),
        ] {
            b.apply(&k);
        }
        assert_eq!((b.text(), b.caret()), ("hXi", 2));
    }

    #[test]
    fn text_editor_carries_caret_on_its_text_leaf() {
        let theme = Theme::light();
        let mut cx = Cx::new(&theme);
        let buf = EditBuffer::from_text("hello");
        let field = text_editor(&mut cx, &theme, &buf, |_: &mut (), _| {});
        // The inner text leaf carries the caret byte index for the focus overlay.
        let ElementKind::Stack { children, .. } = &field.kind else {
            panic!("text_editor should be a stack");
        };
        assert_eq!(children[0].caret, Some(5));
    }
}
