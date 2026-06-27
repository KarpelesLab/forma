//! Opens two real OS windows that share one app state, each with its own view.
//!
//! On Linux with `$DISPLAY` set this uses the native X11 backend, which now
//! drives multiple top-level windows on one connection (the parent `App` owns
//! the global state; each window is a `Pane` with its own view + render state).
//! Used by the visual-test CI job: it positions the two windows side by side and
//! screenshots the root to confirm both painted, with distinct colors.

use stipple::platform::WindowAttributes;
use stipple::prelude::*;

/// Shared application state both windows read from.
struct Shared {
    label_a: &'static str,
    label_b: &'static str,
}

/// A window that fills itself with `color` and centers `text` — the colored
/// fill makes each window unmistakable in the root screenshot.
fn colored_window(color: Color, text: &str, cx: &mut Cx<Shared>) -> Element {
    let theme = *cx.theme();
    let panel = Element::boxed(BoxStyle {
        fill: Some(color),
        radius: 0.0,
        border: None,
    })
    .width(320.0)
    .height(240.0)
    .align(Align::Center, Align::Center);
    column(vec![panel, label(&theme, text)])
        .grow(1.0)
        .align(Align::Center, Align::Center)
}

fn main() {
    let mut app = App::new(
        Shared {
            label_a: "Window A",
            label_b: "Window B",
        },
        |s: &Shared, cx: &mut Cx<Shared>| {
            colored_window(Color::rgb(0xd8, 0x40, 0x40), s.label_a, cx)
        },
    )
    .title("Stipple — A")
    .theme(Theme::dark())
    .logical_size(Size::new(320.0, 240.0));
    if let Some(font) = Font::system_default() {
        app = app.font(font);
    }

    // A second, independent OS window placed to the right of the first.
    app.open_window(
        WindowAttributes::new()
            .with_title("Stipple — B")
            .with_logical_size(Size::new(320.0, 240.0))
            .with_position(340, 0),
        |s: &Shared, cx: &mut Cx<Shared>| {
            colored_window(Color::rgb(0x40, 0x80, 0xd8), s.label_b, cx)
        },
    );

    app.run();
}
