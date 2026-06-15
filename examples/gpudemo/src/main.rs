//! Renders a Forma frame on the CPU, routes it through the GPU
//! ([`forma_gpu::present_offscreen`] — EGL + GLES2 upload → draw → read back),
//! and writes the read-back RGBA to `gpu-out.raw`.
//!
//! The CI GPU job builds this with `--features forma-gpu/gl`, runs it against
//! Mesa's software GL, and converts the raw output to a PNG screenshot. Without
//! the `gl` feature, `present_offscreen` returns an error and the demo exits
//! non-zero.

use forma::prelude::*;

// Fixed size so CI knows the raw image dimensions (scale 1 ⇒ 420×300 px).
const W: f64 = 420.0;
const H: f64 = 300.0;

fn view(_state: &(), cx: &mut Cx<()>) -> Element {
    let theme = *cx.theme();
    let card = panel(
        &theme,
        vec![
            label(&theme, "GPU present"),
            divider(&theme),
            setting_row(&theme, Color::rgb(0xef, 0x68, 0x68)),
            setting_row(&theme, Color::rgb(0x34, 0xd3, 0x99)),
            setting_row(&theme, Color::rgb(0xf5, 0x9e, 0x0b)),
        ],
    )
    .width(360.0);
    column(vec![card])
        .grow(1.0)
        .align(Align::Center, Align::Center)
}

fn main() {
    let mut app = App::new((), view)
        .theme(Theme::dark())
        .logical_size(Size::new(W, H));
    if let Some(font) = Font::system_default() {
        app = app.font(font);
    }
    let cpu = app.render_once();

    match forma_gpu::present_offscreen(&cpu) {
        Ok(out) => {
            std::fs::write("gpu-out.raw", out.as_bytes()).expect("write raw");
            println!(
                "GPU round-trip ok: {}x{}",
                out.size().width,
                out.size().height
            );
        }
        Err(e) => {
            eprintln!("GPU present failed: {e}");
            std::process::exit(1);
        }
    }
}
