//! Copy/cut/paste in a caret-aware text field. The field starts with "Clip";
//! CI focuses it, selects all (Ctrl+A), copies (Ctrl+C), moves to the end, and
//! pastes (Ctrl+V) — doubling the text to "ClipClip" — then screenshots before
//! and after to confirm the paste changed the field. The copied text is also
//! pushed to the OS clipboard (verified separately via xclip).

use forma::prelude::*;
use forma::widgets::EditBuffer;

#[derive(Default)]
struct State {
    buffer: EditBuffer,
}

fn view(state: &State, cx: &mut Cx<State>) -> Element {
    let theme = *cx.theme();
    let field = text_editor(cx, &theme, &state.buffer, |s: &mut State| &mut s.buffer)
        .width(560.0)
        .height(64.0);
    panel(
        &theme,
        vec![label(&theme, "Clipboard (Tab to focus):"), field],
    )
    .padding(Insets::uniform(32.0))
    .align(Align::Start, Align::Stretch)
}

fn main() {
    let mut app = App::new(
        State {
            buffer: EditBuffer::from_text("Clip"),
        },
        view,
    )
    .title("Forma Clipboard")
    .theme(Theme::dark())
    .logical_size(Size::new(640.0, 240.0));
    if let Some(font) = Font::system_default() {
        app = app.font(font);
    }
    app.run();
}
