//! Renders a themed settings panel with Forma and writes it to a PNG.
//!
//! This exercises the whole scaffolded stack end to end — widgets →
//! forma-core layout/paint → forma-render scene → oxideav-raster → Pixmap —
//! and then encodes the result with `oxideav-png` so there's something to look
//! at. No window is opened (the native event loop is a later roadmap phase);
//! we render off-screen via [`forma::prelude::App::render_once`].
//!
//! ```text
//! cargo run -p gallery            # writes forma-gallery.png
//! cargo run -p gallery out.png    # writes out.png
//! ```

use forma::prelude::*;
use forma::render::Pixmap;
use oxideav_png::image::{PngImage, PngPixelFormat};

/// Application state (nothing to track yet in the scaffold).
struct Gallery;

/// The UI: a centered card holding a header bar and a few setting rows.
fn view(_state: &Gallery, theme: &Theme) -> Element {
    let card = panel(
        theme,
        vec![
            // Header bar.
            Element::boxed(BoxStyle {
                fill: Some(theme.palette.primary),
                radius: theme.radius / 2.0,
                border: None,
            })
            .height(28.0),
            divider(theme),
            setting_row(theme, Color::rgb(0xef, 0x68, 0x68)),
            setting_row(theme, Color::rgb(0x34, 0xd3, 0x99)),
            setting_row(theme, Color::rgb(0xf5, 0x9e, 0x0b)),
        ],
    )
    .width(320.0);

    // Center the card in the window.
    column(vec![card])
        .grow(1.0)
        .align(Align::Center, Align::Center)
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "forma-gallery.png".to_string());

    let app = App::new(Gallery, view)
        .title("Forma Gallery")
        .theme(Theme::dark())
        .logical_size(Size::new(380.0, 260.0))
        .scale(ScaleFactor::new(2.0)); // render @2x for crisp output

    let frame = app.render_once();
    write_png(&frame, &out).expect("encode + write PNG");

    let sz = frame.size();
    println!(
        "Rendered {}x{} px (logical 380x260 @2x) -> {out}",
        sz.width, sz.height
    );
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
