use std::{cell::RefCell, error::Error, fmt};

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

// X11 clipboard ownership is process-backed: dropping the provider immediately
// after a write can lose the selection when no clipboard manager is running.
// Explicit clipboard commands run on the GUI thread, so retain one text-only
// backend for that thread's lifetime; no polling or hot-path reads are added.
thread_local! {
    static SYSTEM_CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}

fn with_system_clipboard<T>(
    operation: impl FnOnce(&mut arboard::Clipboard) -> Result<T, arboard::Error>,
) -> Result<T, ClipboardError> {
    SYSTEM_CLIPBOARD
        .try_with(|clipboard| {
            let mut clipboard = clipboard
                .try_borrow_mut()
                .map_err(|_| ClipboardError::new("clipboard is already in use"))?;
            if clipboard.is_none() {
                *clipboard = Some(arboard::Clipboard::new().map_err(|error| {
                    ClipboardError::new(format!("clipboard unavailable: {error}"))
                })?);
            }
            let clipboard = clipboard
                .as_mut()
                .ok_or_else(|| ClipboardError::new("clipboard initialization failed"))?;
            operation(clipboard).map_err(|error| ClipboardError::new(error.to_string()))
        })
        .map_err(|_| ClipboardError::new("clipboard thread is shutting down"))?
}

impl ClipboardSink for SystemClipboard {
    fn set_text(&mut self, text: String) -> Result<(), ClipboardError> {
        with_system_clipboard(|clipboard| clipboard.set_text(text))
            .map_err(|error| ClipboardError::new(format!("clipboard write failed: {error}")))
    }

    fn get_text(&mut self) -> Result<String, ClipboardError> {
        with_system_clipboard(arboard::Clipboard::get_text)
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
    use super::{ClipboardError, ClipboardSink, SystemClipboard};

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
    #[ignore = "requires a live desktop clipboard and temporarily replaces its text"]
    fn live_system_clipboard_round_trip() {
        let mut clipboard = SystemClipboard;
        let previous = clipboard.get_text().ok();
        let marker = format!("clay-clipboard-smoke-{}", std::process::id());

        clipboard.set_text(marker.clone()).unwrap();
        assert_eq!(clipboard.get_text().unwrap(), marker);

        if let Some(previous) = previous {
            clipboard.set_text(previous).unwrap();
        }
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
