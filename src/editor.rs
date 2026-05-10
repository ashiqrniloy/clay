pub(crate) mod buffer;
pub(crate) mod cursor;
mod layout;
pub(crate) mod selection;
mod surface;
mod viewport;

use masonry::peniko::Color;

pub use surface::{EditorCommand, EditorCommandOutcome, EditorEditEvent, EditorSurface};

const BACKGROUND_COLOR: Color = Color::from_rgb8(0x18, 0x18, 0x18);

pub fn is_printable_text(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|character| !character.is_control())
}

pub fn background_color() -> Color {
    BACKGROUND_COLOR
}
