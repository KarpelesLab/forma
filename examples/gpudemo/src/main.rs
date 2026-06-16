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

    // GPU-native drawing: three solid rectangles tessellated and filled by the
    // GPU (no CPU pixmap), on a dark background.
    let size = forma::geometry::PhysicalSize::new(W as u32, H as u32);
    // (rect, color, corner_radius, border_width): a sharp fill, a rounded fill,
    // a pill fill, and a rounded *outline* (border).
    let rects = [
        (
            Rect::from_xywh(40.0, 40.0, 120.0, 80.0),
            Color::rgb(0xef, 0x68, 0x68),
            0.0,
            0.0,
        ),
        (
            Rect::from_xywh(180.0, 60.0, 120.0, 80.0),
            Color::rgb(0x34, 0xd3, 0x99),
            24.0,
            0.0,
        ),
        (
            Rect::from_xywh(40.0, 180.0, 120.0, 80.0),
            Color::rgb(0x60, 0x9c, 0xff),
            40.0,
            0.0,
        ),
        (
            Rect::from_xywh(190.0, 180.0, 120.0, 80.0),
            Color::rgb(0xf5, 0x9e, 0x0b),
            18.0,
            6.0,
        ),
    ];
    // GPU text: rasterize a label to a white-on-black coverage mask on the CPU,
    // then let the GPU composite it (alpha-blended, recolored) over the boxes.
    let mask = Font::system_default().map(|font| {
        let label = "FORMA - GPU";
        let m = font.measure(label, 30.0);
        let (tw, th) = (m.width.ceil() + 12.0, m.height.ceil() + 6.0);
        let mut scene = forma::render::Scene::new(Size::new(tw, th));
        scene.fill_text(&font, label, Point::new(6.0, 2.0), 30.0, Color::WHITE);
        let pm = forma::render::SoftwareRenderer::new()
            .with_background(Color::BLACK)
            .render(scene, ScaleFactor::IDENTITY);
        (pm, Rect::from_xywh(40.0, 130.0, tw, th))
    });
    let texts: Vec<(&forma::render::Pixmap, Rect, Color)> = mask
        .iter()
        .map(|(pm, dst)| (pm, *dst, Color::rgb(0xec, 0xee, 0xf2)))
        .collect();

    match forma_gpu::render_offscreen(size, Color::rgb(0x14, 0x15, 0x18), &rects, &texts) {
        Ok(out) => {
            std::fs::write("gpu-rects.raw", out.as_bytes()).expect("write raw");
            println!(
                "GPU-native scene ok: {}x{}",
                out.size().width,
                out.size().height
            );
        }
        Err(e) => {
            eprintln!("GPU rects failed: {e}");
            std::process::exit(1);
        }
    }
}
