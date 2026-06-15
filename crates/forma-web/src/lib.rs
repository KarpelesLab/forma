//! Forma on the web: a wasm module that renders a UI to a software [`Pixmap`]
//! and exposes its RGBA bytes so a tiny hand-written JS shim can blit them to a
//! `<canvas>` via `putImageData` — no `wasm-bindgen`, no JS framework (the
//! workspace dependency policy in `ROADMAP.md` §1).
//!
//! A Forma `Pixmap` is straight RGBA8, which is exactly the layout
//! `ImageData`/`putImageData` expects, so presentation is a direct copy.
//!
//! ABI: JS calls [`forma_render`] with the canvas size, then reads
//! [`forma_frame_ptr`]/[`forma_frame_len`] out of the wasm memory. Text is
//! skipped for now (no font is bundled); the UI is drawn from shapes.
//!
//! Built for `wasm32-unknown-unknown`; verified by the `Visual` workflow's web
//! job (headless-Chrome screenshot). Input wiring (canvas events → Forma) is a
//! follow-up.

use std::sync::Mutex;

use forma::prelude::*;

/// The most recently rendered frame (straight RGBA8) and its pixel size.
static FRAME: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static SIZE: Mutex<(u32, u32)> = Mutex::new((0, 0));

fn view(_state: &(), cx: &mut Cx<()>) -> Element {
    let theme = *cx.theme();
    // Shapes only (no font on web yet): a card with an accent header bar, a
    // divider, and three setting rows.
    let card = panel(
        &theme,
        vec![
            Element::boxed(BoxStyle {
                fill: Some(theme.palette.primary),
                radius: theme.radius / 2.0,
                border: None,
            })
            .height(28.0),
            divider(&theme),
            setting_row(&theme, Color::rgb(0xef, 0x68, 0x68)),
            setting_row(&theme, Color::rgb(0x34, 0xd3, 0x99)),
            setting_row(&theme, Color::rgb(0xf5, 0x9e, 0x0b)),
        ],
    )
    .width(360.0);
    column(vec![card]).grow(1.0).align(Align::Center, Align::Center)
}

/// Render the UI at `width` × `height` logical pixels (scale 1) and store the
/// resulting RGBA frame for JS to read.
///
/// # Safety
/// Called from JS; no Rust-side invariants beyond the global locks.
#[unsafe(no_mangle)]
pub extern "C" fn forma_render(width: u32, height: u32) {
    let mut app = App::new((), view)
        .theme(Theme::dark())
        .logical_size(Size::new(width.max(1) as f64, height.max(1) as f64));
    let pixmap = app.render_once();
    let size = pixmap.size();
    *FRAME.lock().unwrap() = pixmap.as_bytes().to_vec();
    *SIZE.lock().unwrap() = (size.width, size.height);
}

/// Pointer to the current RGBA frame in wasm linear memory.
#[unsafe(no_mangle)]
pub extern "C" fn forma_frame_ptr() -> *const u8 {
    FRAME.lock().unwrap().as_ptr()
}

/// Length in bytes of the current frame (`width * height * 4`).
#[unsafe(no_mangle)]
pub extern "C" fn forma_frame_len() -> usize {
    FRAME.lock().unwrap().len()
}

#[unsafe(no_mangle)]
pub extern "C" fn forma_width() -> u32 {
    SIZE.lock().unwrap().0
}

#[unsafe(no_mangle)]
pub extern "C" fn forma_height() -> u32 {
    SIZE.lock().unwrap().1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_fills_a_frame() {
        forma_render(200, 120);
        assert_eq!((forma_width(), forma_height()), (200, 120));
        assert_eq!(forma_frame_len(), 200 * 120 * 4);
    }
}
