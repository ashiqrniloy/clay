pub(crate) mod accessibility;
pub(crate) mod buffer;
pub(crate) mod composition;
pub(crate) mod cursor;
pub(crate) mod history;
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

/// Normalize clipboard paste text for ordinary local insert/replace.
///
/// Line endings become `\n`. Tabs and newlines are retained; other control
/// characters are rejected by returning `None`. Empty input is a paste no-op.
pub fn normalize_clipboard_paste_text(text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }

    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                normalized.push('\n');
            }
            '\n' | '\t' => normalized.push(character),
            other if other.is_control() => return None,
            other => normalized.push(other),
        }
    }

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::{is_printable_text, normalize_clipboard_paste_text};

    #[test]
    fn printable_text_rejects_controls_and_newlines() {
        assert!(is_printable_text("abc é 🦀"));
        assert!(!is_printable_text(""));
        assert!(!is_printable_text("a\n"));
    }

    #[test]
    fn clipboard_paste_normalizes_line_endings_and_allows_tabs() {
        assert_eq!(
            normalize_clipboard_paste_text("a\r\nb\tc").as_deref(),
            Some("a\nb\tc")
        );
        assert_eq!(normalize_clipboard_paste_text(""), None);
        assert_eq!(normalize_clipboard_paste_text("a\0b"), None);
    }
}
