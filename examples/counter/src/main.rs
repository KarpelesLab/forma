//! An interactive counter, demonstrating Forma's `on_tap` handlers and text.
//!
//! There's no window here (the native event loop is a later roadmap phase), so
//! instead of waiting for real clicks we drive the same public dispatch path
//! ([`App::click_at`]) at the buttons' locations and render the UI before and
//! after, proving state → view updates end to end.
//!
//! ```text
//! cargo run -p counter        # writes counter-before.png and counter-after.png
//! ```

use forma::prelude::*;
use forma::render::Pixmap;
use oxideav_png::image::{PngImage, PngPixelFormat};

struct Counter {
    n: i64,
}

// Fixed control geometry (plus the column's padding) so the demo knows where
// to "click".
const PAD: f64 = 16.0; // theme.spacing.lg
const BTN_W: f64 = 72.0;
const BTN_H: f64 = 56.0;
const MINUS_CENTER: Point = Point {
    x: PAD + BTN_W * 0.5,
    y: PAD + BTN_H * 0.5,
};
const PLUS_CENTER: Point = Point {
    x: PAD + BTN_W * 1.5,
    y: PAD + BTN_H * 0.5,
};

fn key_button(theme: &Theme, glyph: &str, cx: &mut Cx<Counter>, delta: i64) -> Element {
    let surface = if delta < 0 {
        theme.palette.surface
    } else {
        theme.palette.primary
    };
    let ink = if delta < 0 {
        theme.palette.text
    } else {
        theme.palette.on_primary
    };
    Element::stack(Axis::Horizontal, vec![Element::text(glyph, 28.0, ink)])
        .fill(surface)
        .radius(theme.radius)
        .border(theme.palette.border, 1.0)
        .width(BTN_W)
        .height(BTN_H)
        .align(Align::Center, Align::Center)
        .on_tap(cx, move |s: &mut Counter| s.n = (s.n + delta).max(0))
}

fn view(state: &Counter, cx: &mut Cx<Counter>) -> Element {
    let theme = *cx.theme();
    let controls = row(vec![
        key_button(&theme, "−", cx, -1),
        key_button(&theme, "+", cx, 1),
    ]);
    let count = label(&theme, format!("Count: {}", state.n));
    column(vec![controls, count])
        .gap(theme.spacing.lg)
        .padding(Insets::uniform(PAD))
}

fn main() {
    let mut app = App::new(Counter { n: 1 }, view)
        .title("Forma Counter")
        .theme(Theme::dark())
        .logical_size(Size::new(420.0, 200.0))
        .scale(ScaleFactor::new(2.0));
    if let Some(font) = Font::system_default() {
        app = app.font(font);
    }

    write_png(&app.render_once(), "counter-before.png").expect("write before");
    println!("Before: n = {}", app.state().n);

    for _ in 0..3 {
        app.click_at(PLUS_CENTER);
    }
    app.click_at(MINUS_CENTER);

    write_png(&app.render_once(), "counter-after.png").expect("write after");
    println!("After 3×(+) and 1×(−): n = {}", app.state().n);
}

fn write_png(frame: &Pixmap, path: &str) -> std::io::Result<()> {
    let size = frame.size();
    let image = PngImage {
        width: size.width,
        height: size.height,
        pixel_format: PngPixelFormat::Rgba,
        stride: frame.stride(),
        data: frame.as_bytes().to_vec(),
        palette: Vec::new(),
    };
    let bytes = oxideav_png::encoder::encode_png_image(&image)
        .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
    std::fs::write(path, bytes)
}
