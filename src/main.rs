use std::error::Error;
use std::num::NonZeroU32;
use std::rc::Rc;

use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const WINDOW_TITLE: &str = "Clay Phase 0";
const WINDOW_WIDTH: f64 = 900.0;
const WINDOW_HEIGHT: f64 = 600.0;
const BACKGROUND_COLOR: u32 = 0xff181818;

#[derive(Default)]
pub struct TextBuffer {
    text: String,
}

impl TextBuffer {
    pub fn insert_str(&mut self, text: &str) {
        self.text.push_str(text);
    }

    pub fn backspace(&mut self) {
        self.text.pop();
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

struct App {
    buffer: TextBuffer,
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    context: Option<Context<Rc<Window>>>,
    window: Option<Rc<Window>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            buffer: TextBuffer::default(),
            surface: None,
            context: None,
            window: None,
        }
    }
}

impl App {
    fn create_window(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = WindowAttributes::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .with_visible(false)
            .with_transparent(false);

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Rc::new(window),
            Err(error) => {
                eprintln!("failed to create window: {error}");
                event_loop.exit();
                return;
            }
        };

        let context = match Context::new(window.clone()) {
            Ok(context) => context,
            Err(error) => {
                eprintln!("failed to create softbuffer context: {error}");
                event_loop.exit();
                return;
            }
        };

        let surface = match Surface::new(&context, window.clone()) {
            Ok(surface) => surface,
            Err(error) => {
                eprintln!("failed to create softbuffer surface: {error}");
                event_loop.exit();
                return;
            }
        };

        window.set_visible(true);
        window.request_redraw();

        self.surface = Some(surface);
        self.context = Some(context);
        self.window = Some(window);
    }

    fn fill_window(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let Some(surface) = &mut self.surface else {
            return;
        };
        let _current_text = self.buffer.as_str();

        let size = window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };

        if let Err(error) = surface.resize(width, height) {
            eprintln!("failed to resize softbuffer surface: {error}");
            return;
        }

        let mut buffer = match surface.buffer_mut() {
            Ok(buffer) => buffer,
            Err(error) => {
                eprintln!("failed to acquire softbuffer buffer: {error}");
                return;
            }
        };

        buffer.fill(BACKGROUND_COLOR);

        if let Err(error) = buffer.present() {
            eprintln!("failed to present softbuffer buffer: {error}");
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            self.create_window(event_loop);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.surface = None;
        self.context = None;
        self.window = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self
            .window
            .as_ref()
            .filter(|window| window.id() == window_id)
            .cloned()
        else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed
                    && matches!(event.logical_key, Key::Named(NamedKey::Escape)) =>
            {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => self.fill_window(),
            WindowEvent::Resized(_) => window.request_redraw(),
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::TextBuffer;

    #[test]
    fn text_buffer_appends_input() {
        let mut buffer = TextBuffer::default();

        buffer.insert_str("Hello");
        buffer.insert_str(", Clay");

        assert_eq!(buffer.as_str(), "Hello, Clay");
    }

    #[test]
    fn text_buffer_backspace_removes_last_char() {
        let mut buffer = TextBuffer::default();
        buffer.insert_str("aé🦀");

        buffer.backspace();
        buffer.backspace();
        buffer.backspace();
        buffer.backspace();

        assert_eq!(buffer.as_str(), "");
    }
}
