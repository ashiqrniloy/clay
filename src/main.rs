use std::error::Error;
use std::sync::Arc;

use vello::kurbo::{Circle, Rect};
use vello::peniko::{Color, Fill};
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const WINDOW_TITLE: &str = "Clay Phase 0";
const WINDOW_WIDTH: f64 = 900.0;
const WINDOW_HEIGHT: f64 = 600.0;
const BACKGROUND_COLOR: Color = Color::from_rgb8(0x18, 0x18, 0x18);
const PANEL_COLOR: Color = Color::from_rgb8(0x24, 0x24, 0x24);
const ACCENT_COLOR: Color = Color::from_rgb8(0x8a, 0x6f, 0xff);

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

struct VelloState {
    render_context: RenderContext,
    surface: RenderSurface<'static>,
    renderer: Renderer,
    scene: Scene,
}

impl VelloState {
    fn new(window: Arc<Window>, size: PhysicalSize<u32>) -> Result<Self, Box<dyn Error>> {
        let mut render_context = RenderContext::new();
        let surface = pollster::block_on(render_context.create_surface(
            window,
            size.width.max(1),
            size.height.max(1),
            vello::wgpu::PresentMode::AutoVsync,
        ))?;

        let device = &render_context.devices[surface.dev_id].device;
        let renderer = Renderer::new(
            device,
            RendererOptions {
                antialiasing_support: AaSupport::area_only(),
                ..Default::default()
            },
        )?;

        Ok(Self {
            render_context,
            surface,
            renderer,
            scene: Scene::new(),
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.render_context
            .resize_surface(&mut self.surface, size.width, size.height);
    }

    fn render(&mut self) {
        let width = self.surface.config.width;
        let height = self.surface.config.height;
        if width == 0 || height == 0 {
            return;
        }

        self.build_scene(width, height);

        let device_handle = &self.render_context.devices[self.surface.dev_id];
        let surface_texture = match self.surface.surface.get_current_texture() {
            Ok(surface_texture) => surface_texture,
            Err(vello::wgpu::SurfaceError::Lost | vello::wgpu::SurfaceError::Outdated) => {
                self.render_context
                    .resize_surface(&mut self.surface, width, height);
                return;
            }
            Err(vello::wgpu::SurfaceError::Timeout) => return,
            Err(error) => {
                eprintln!("failed to acquire Vello surface texture: {error}");
                return;
            }
        };

        if let Err(error) = self.renderer.render_to_texture(
            &device_handle.device,
            &device_handle.queue,
            &self.scene,
            &self.surface.target_view,
            &RenderParams {
                base_color: BACKGROUND_COLOR,
                width,
                height,
                antialiasing_method: AaConfig::Area,
            },
        ) {
            eprintln!("failed to render Vello scene: {error}");
            surface_texture.present();
            return;
        }

        let surface_view = surface_texture
            .texture
            .create_view(&vello::wgpu::TextureViewDescriptor::default());
        let mut encoder =
            device_handle
                .device
                .create_command_encoder(&vello::wgpu::CommandEncoderDescriptor {
                    label: Some("Vello surface blit encoder"),
                });
        self.surface.blitter.copy(
            &device_handle.device,
            &mut encoder,
            &self.surface.target_view,
            &surface_view,
        );
        device_handle.queue.submit([encoder.finish()]);
        surface_texture.present();
    }

    fn build_scene(&mut self, width: u32, height: u32) {
        self.scene.reset();

        let canvas = Rect::new(
            24.0,
            24.0,
            (width as f64 - 24.0).max(24.0),
            (height as f64 - 24.0).max(24.0),
        );
        self.scene.fill(
            Fill::NonZero,
            vello::kurbo::Affine::IDENTITY,
            PANEL_COLOR,
            None,
            &canvas,
        );

        let radius = (width.min(height) as f64 * 0.12).clamp(32.0, 96.0);
        let circle = Circle::new((width as f64 * 0.5, height as f64 * 0.5), radius);
        self.scene.fill(
            Fill::NonZero,
            vello::kurbo::Affine::IDENTITY,
            ACCENT_COLOR,
            None,
            &circle,
        );
    }
}

#[derive(Default)]
struct App {
    buffer: TextBuffer,
    renderer: Option<VelloState>,
    window: Option<Arc<Window>>,
}

impl App {
    fn create_window(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = WindowAttributes::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .with_visible(false)
            .with_transparent(false);

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("failed to create window: {error}");
                event_loop.exit();
                return;
            }
        };

        let renderer = match VelloState::new(window.clone(), window.inner_size()) {
            Ok(renderer) => renderer,
            Err(error) => {
                eprintln!("failed to create Vello renderer: {error}");
                event_loop.exit();
                return;
            }
        };

        window.set_visible(true);
        window.request_redraw();

        self.renderer = Some(renderer);
        self.window = Some(window);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            self.create_window(event_loop);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.renderer = None;
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
            WindowEvent::RedrawRequested => {
                let _current_text = self.buffer.as_str();
                if let Some(renderer) = &mut self.renderer {
                    renderer.render();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size);
                }
                window.request_redraw();
            }
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
