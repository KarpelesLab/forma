//! An interactive text field driven by the real event loop (`App::run`).
//!
//! Used by the CI X11 interaction job to verify keyboard text input: the runner
//! launches this under Xvfb, focuses the field with `xdotool key Tab`, types
//! with `xdotool type`, and screenshots — confirming the
//! keysym → text → [`EditBuffer`] → re-present path on a real X server. The
//! buffer is caret-aware, so arrow keys move the caret and insert mid-string.

use stipple::prelude::*;

#[derive(Default)]
struct Form {
    buffer: EditBuffer,
}

fn view(state: &Form, cx: &mut Cx<Form>) -> Element {
    let theme = *cx.theme();
    let field = text_editor(cx, &theme, &state.buffer, |s: &mut Form| &mut s.buffer)
        .width(560.0)
        .height(64.0);
    panel(
        &theme,
        vec![label(&theme, "Type here (Tab to focus):"), field],
    )
    .padding(Insets::uniform(32.0))
    .align(Align::Start, Align::Stretch)
}

fn main() {
    let mut app = App::new(Form::default(), view)
        .title("Stipple Text Input")
        .theme(Theme::dark())
        .logical_size(Size::new(640.0, 240.0));
    if let Some(font) = Font::system_default() {
        app = app.font(font);
    }
    app.run();
}
