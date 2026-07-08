use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardError {
    message: String,
}

impl ClipboardError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ClipboardError {}

pub trait ClipboardSink {
    fn set_text(&mut self, text: String) -> Result<(), ClipboardError>;
}

#[derive(Debug, Default)]
pub struct SystemClipboard;

impl ClipboardSink for SystemClipboard {
    fn set_text(&mut self, text: String) -> Result<(), ClipboardError> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|error| ClipboardError::new(format!("clipboard unavailable: {error}")))?;
        clipboard
            .set_text(text)
            .map_err(|error| ClipboardError::new(format!("clipboard write failed: {error}")))
    }
}

pub fn copy_text_to_system_clipboard(text: String) -> Result<(), ClipboardError> {
    let mut clipboard = SystemClipboard;
    clipboard.set_text(text)
}

#[cfg(test)]
mod tests {
    use super::{ClipboardError, ClipboardSink};

    #[derive(Default)]
    struct MemoryClipboard {
        text: Option<String>,
    }

    impl ClipboardSink for MemoryClipboard {
        fn set_text(&mut self, text: String) -> Result<(), ClipboardError> {
            self.text = Some(text);
            Ok(())
        }
    }

    #[test]
    fn clipboard_sink_accepts_utf8_text() {
        let mut clipboard = MemoryClipboard::default();

        clipboard.set_text("hello 🦀".to_string()).unwrap();

        assert_eq!(clipboard.text.as_deref(), Some("hello 🦀"));
    }
}
