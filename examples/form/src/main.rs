//! A text field edited by keyboard input, demonstrating focus + `on_key`.
//!
//! No window here (native event loop is a later phase), so we focus the field
//! and feed it text through the same public path the event loop uses
//! ([`App::focus_next`] / [`App::type_text`]), then render the result.
//!
//! ```text
//! cargo run -p form        # writes form.png
//! ```

use forma::prelude::*;
use forma::render::Pixmap;
use oxideav_png::image::{PngImage, PngPixelFormat};

#[derive(Default)]
struct Form {
    name: String,
}

fn view(state: &Form, cx: &mut Cx<Form>) -> Element {
    let theme = *cx.theme();
    let field = text_field(cx, &theme, &state.name, |s: &mut Form, k| {
        edit_string(&mut s.name, k)
    })
    .width(300.0)
    .height(44.0);
    panel(&theme, vec![label(&theme, "Name"), field])
        .width(360.0)
        .align(Align::Start, Align::Start)
}

fn main() {
    let mut app = App::new(Form::default(), view)
        .title("Forma Form")
        .theme(Theme::dark())
        .logical_size(Size::new(420.0, 180.0))
        .scale(ScaleFactor::new(2.0));
    if let Some(font) = Font::system_default() {
        app = app.font(font);
    }

    // Tab to focus the field, then type.
    app.focus_next();
    app.type_text("Ada Lovelace");

    write_png(&app.render_once(), "form.png").expect("write png");
    println!("Field contents: {:?}", app.state().name);
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
