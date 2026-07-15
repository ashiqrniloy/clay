pub(crate) mod buffer;
pub(crate) mod cursor;
mod layout;
pub(crate) mod selection;
pub(crate) mod snippet;
pub mod surface;
pub mod theme;
pub(crate) mod typography;
mod viewport;

pub(crate) use surface::EditorCompletionRequestEvent;
pub(crate) use surface::EditorLanguageIntelligenceRequestEvent;
pub use surface::{EditorCommand, EditorCommandOutcome, EditorEditEvent, EditorSurface};

pub fn is_printable_text(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|character| !character.is_control())
}
