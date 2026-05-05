use std::error::Error;

use masonry::core::{ErasedAction, NewWidget, WidgetId};
use masonry::theme::default_property_set;
use masonry_winit::app::{AppDriver, DriverCtx, NewWindow, WindowId};
use masonry_winit::winit::dpi::LogicalSize;
use masonry_winit::winit::event_loop::EventLoop;
use masonry_winit::winit::window::Window;

mod editor;
mod masonry_editor;

use masonry_editor::{EditorAction, EditorWidget};

const WINDOW_TITLE: &str = "Clay Phase 0";
const WINDOW_WIDTH: f64 = 900.0;
const WINDOW_HEIGHT: f64 = 600.0;

struct Driver {
    editor_widget_id: WidgetId,
}

impl AppDriver for Driver {
    fn on_start(&mut self, state: &mut masonry_winit::app::MasonryState<'_>) {
        for root in state.roots() {
            root.set_focus_fallback(Some(self.editor_widget_id));
        }
    }

    fn on_action(
        &mut self,
        _window_id: WindowId,
        ctx: &mut DriverCtx<'_, '_>,
        _widget_id: WidgetId,
        action: ErasedAction,
    ) {
        if action.downcast::<EditorAction>().is_ok() {
            ctx.exit();
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let root_widget = NewWidget::new(EditorWidget::default());
    let editor_widget_id = root_widget.id();
    let window_attributes = Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));

    masonry_winit::app::run_with(
        EventLoop::with_user_event().build()?,
        vec![NewWindow::new(window_attributes, root_widget.erased())],
        Driver { editor_widget_id },
        default_property_set(),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::editor::{EditorBuffer, is_printable_text};

    #[test]
    fn editor_buffer_appends_input() {
        let mut buffer = EditorBuffer::default();

        buffer.insert_str("Hello");
        buffer.insert_str(", Clay");

        assert_eq!(buffer.visible_text(), "Hello, Clay");
    }

    #[test]
    fn editor_buffer_backspace_removes_last_scalar() {
        let mut buffer = EditorBuffer::default();
        buffer.insert_str("aé🦀");

        buffer.backspace();
        assert_eq!(buffer.visible_text(), "aé");

        buffer.backspace();
        assert_eq!(buffer.visible_text(), "a");

        buffer.backspace();
        assert_eq!(buffer.visible_text(), "");

        buffer.backspace();
        assert_eq!(buffer.visible_text(), "");
    }

    #[test]
    fn printable_text_filter_accepts_plain_text_and_rejects_controls() {
        assert!(is_printable_text("abc é 🦀"));
        assert!(!is_printable_text(""));
        assert!(!is_printable_text("\r"));
        assert!(!is_printable_text("a\n"));
    }
}
