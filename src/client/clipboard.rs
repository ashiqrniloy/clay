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
    fn get_text(&mut self) -> Result<String, ClipboardError>;
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

    fn get_text(&mut self) -> Result<String, ClipboardError> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|error| ClipboardError::new(format!("clipboard unavailable: {error}")))?;
        clipboard
            .get_text()
            .map_err(|error| ClipboardError::new(format!("clipboard read failed: {error}")))
    }
}

pub fn copy_text_to_system_clipboard(text: String) -> Result<(), ClipboardError> {
    let mut clipboard = SystemClipboard;
    clipboard.set_text(text)
}

pub fn read_text_from_system_clipboard() -> Result<String, ClipboardError> {
    let mut clipboard = SystemClipboard;
    clipboard.get_text()
}

#[cfg(test)]
mod tests {
    use super::{ClipboardError, ClipboardSink};

    #[derive(Default)]
    struct MemoryClipboard {
        text: Option<String>,
        fail_set: bool,
        fail_get: bool,
    }

    impl ClipboardSink for MemoryClipboard {
        fn set_text(&mut self, text: String) -> Result<(), ClipboardError> {
            if self.fail_set {
                return Err(ClipboardError::new("no display"));
            }
            self.text = Some(text);
            Ok(())
        }

        fn get_text(&mut self) -> Result<String, ClipboardError> {
            if self.fail_get {
                return Err(ClipboardError::new("no display"));
            }
            Ok(self.text.clone().unwrap_or_default())
        }
    }

    #[test]
    fn clipboard_sink_accepts_utf8_text() {
        let mut clipboard = MemoryClipboard::default();

        clipboard.set_text("hello 🦀".to_string()).unwrap();

        assert_eq!(clipboard.text.as_deref(), Some("hello 🦀"));
    }

    #[test]
    fn clipboard_sink_reads_back_utf8_text() {
        let mut clipboard = MemoryClipboard::default();
        clipboard.set_text("paste 🦀 me".to_string()).unwrap();

        assert_eq!(clipboard.get_text().unwrap(), "paste 🦀 me");
    }

    #[test]
    fn clipboard_sink_get_text_returns_empty_when_unset() {
        let mut clipboard = MemoryClipboard::default();

        assert_eq!(clipboard.get_text().unwrap(), "");
    }

    #[test]
    fn clipboard_sink_get_text_failure_is_sanitized() {
        let mut clipboard = MemoryClipboard {
            fail_get: true,
            ..MemoryClipboard::default()
        };

        let error = clipboard.get_text().unwrap_err();
        assert_eq!(error.to_string(), "no display");
    }

    #[test]
    fn clipboard_sink_set_text_failure_is_sanitized() {
        let mut clipboard = MemoryClipboard {
            fail_set: true,
            ..MemoryClipboard::default()
        };

        let error = clipboard.set_text("x".to_string()).unwrap_err();
        assert_eq!(error.to_string(), "no display");
    }
}
