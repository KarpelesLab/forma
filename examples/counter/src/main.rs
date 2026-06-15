//! An interactive counter, demonstrating Forma's `on_tap` handlers.
//!
//! There's no window here (the native event loop is a later roadmap phase), so
//! instead of waiting for real clicks we drive the same public dispatch path
//! ([`App::click_at`]) at the "+" button's location and render the UI before
//! and after, proving state → view updates end to end.
//!
//! The count is shown as a row of `n` swatches (text rendering is a later
//! milestone). Controls sit at a fixed position so the click points are known.
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

// Fixed control geometry so the demo knows where to "click".
const BTN_W: f64 = 72.0;
const BTN_H: f64 = 56.0;
const MINUS_CENTER: Point = Point {
    x: BTN_W * 0.5,
    y: BTN_H * 0.5,
};
const PLUS_CENTER: Point = Point {
    x: BTN_W * 1.5,
    y: BTN_H * 0.5,
};

fn view(state: &Counter, cx: &mut Cx<Counter>) -> Element {
    let theme = *cx.theme();

    // Controls row at the top-left: [ − ][ + ], each BTN_W × BTN_H, no gap.
    let minus = button(&theme)
        .width(BTN_W)
        .height(BTN_H)
        .fill(theme.palette.surface)
        .border(theme.palette.border, 1.0)
        .on_tap(cx, |s: &mut Counter| s.n = (s.n - 1).max(0));
    let plus = button(&theme)
        .width(BTN_W)
        .height(BTN_H)
        .on_tap(cx, |s: &mut Counter| s.n += 1);
    let controls = row(vec![minus, plus]);

    // A row of `n` swatches visualizing the count.
    let swatches: Vec<Element> = (0..state.n.max(0))
        .map(|_| swatch(theme.palette.primary, 24.0, theme.radius / 2.0))
        .collect();
    let count_row = row(swatches).gap(theme.spacing.sm).height(40.0);

    column(vec![controls, count_row])
        .gap(theme.spacing.lg)
        .padding(Insets::uniform(theme.spacing.lg))
}

fn main() {
    let mut app = App::new(Counter { n: 1 }, view)
        .title("Forma Counter")
        .theme(Theme::dark())
        .logical_size(Size::new(420.0, 200.0))
        .scale(ScaleFactor::new(2.0));

    write_png(&app.render_once(), "counter-before.png").expect("write before");
    println!("Before: n = {}", app.state().n);

    // The controls row sits inside the column's padding (spacing.lg = 16), so
    // offset the click points by that padding.
    let pad = 16.0;
    let offset = forma::geometry::Vec2::new(pad, pad);
    for _ in 0..3 {
        app.click_at(PLUS_CENTER + offset);
    }
    app.click_at(MINUS_CENTER + offset);

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
