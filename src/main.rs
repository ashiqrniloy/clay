use std::error::Error;
use std::sync::Arc;

use crop::Rope;
use parley::style::{LineHeight, StyleProperty};
use parley::{
    Alignment, AlignmentOptions, FontContext, Layout, LayoutContext, PositionedLayoutItem,
};
use vello::kurbo::{Affine, Circle, Rect};
use vello::peniko::{Color, Fill};
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, AaSupport, Glyph, RenderParams, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const WINDOW_TITLE: &str = "Clay Phase 0";
const WINDOW_WIDTH: f64 = 900.0;
const WINDOW_HEIGHT: f64 = 600.0;
const BACKGROUND_COLOR: Color = Color::from_rgb8(0x18, 0x18, 0x18);
const PANEL_COLOR: Color = Color::from_rgb8(0x24, 0x24, 0x24);
const ACCENT_COLOR: Color = Color::from_rgb8(0x8a, 0x6f, 0xff);
const TEXT_COLOR: [u8; 4] = [0xf4, 0xf1, 0xff, 0xff];
const PLACEHOLDER_COLOR: [u8; 4] = [0x8d, 0x86, 0xa3, 0xff];
const TEXT_INSET: f64 = 48.0;
const TEXT_FONT_SIZE: f32 = 20.0;
const PLACEHOLDER_TEXT: &str = "Start typing in the Clay native text canvas…";

#[derive(Default)]
pub struct EditorBuffer {
    rope: Rope,
}

impl EditorBuffer {
    pub fn insert_str(&mut self, text: &str) {
        self.rope.insert(self.rope.byte_len(), text);
    }

    pub fn backspace(&mut self) {
        let Some(last_char) = self.rope.chars().next_back() else {
            return;
        };

        let end = self.rope.byte_len();
        self.rope.delete(end - last_char.len_utf8()..end);
    }

    pub fn visible_text(&self) -> String {
        self.rope.to_string()
    }
}

struct TextLayoutState {
    font_context: FontContext,
    layout_context: LayoutContext<[u8; 4]>,
}

impl TextLayoutState {
    fn new() -> Self {
        Self {
            font_context: FontContext::new(),
            layout_context: LayoutContext::new(),
        }
    }

    fn draw_text(&mut self, scene: &mut Scene, text: &str, max_width: f32, origin: (f64, f64)) {
        let (display_text, brush) = if text.is_empty() {
            (PLACEHOLDER_TEXT, PLACEHOLDER_COLOR)
        } else {
            (text, TEXT_COLOR)
        };

        let mut builder =
            self.layout_context
                .ranged_builder(&mut self.font_context, display_text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(TEXT_FONT_SIZE));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.4)));
        builder.push_default(StyleProperty::Brush(brush));

        let mut layout: Layout<[u8; 4]> = builder.build(display_text);
        layout.break_all_lines(Some(max_width));
        layout.align(
            Some(max_width),
            Alignment::Start,
            AlignmentOptions::default(),
        );

        let transform = Affine::translate(origin);
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };

                let run = glyph_run.run();
                let mut x = glyph_run.offset();
                let y = glyph_run.baseline();
                scene
                    .draw_glyphs(run.font())
                    .brush(Color::from_rgba8(
                        glyph_run.style().brush[0],
                        glyph_run.style().brush[1],
                        glyph_run.style().brush[2],
                        glyph_run.style().brush[3],
                    ))
                    .hint(true)
                    .transform(transform)
                    .font_size(run.font_size())
                    .normalized_coords(run.normalized_coords())
                    .draw(
                        Fill::NonZero,
                        glyph_run.glyphs().map(|glyph| {
                            let positioned = Glyph {
                                id: glyph.id,
                                x: x + glyph.x,
                                y: y + glyph.y,
                            };
                            x += glyph.advance;
                            positioned
                        }),
                    );
            }
        }
    }
}

struct VelloState {
    render_context: RenderContext,
    surface: RenderSurface<'static>,
    renderer: Renderer,
    scene: Scene,
    text_layout: TextLayoutState,
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
            text_layout: TextLayoutState::new(),
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        self.render_context
            .resize_surface(&mut self.surface, size.width, size.height);
    }

    fn render(&mut self, text: &str) {
        let width = self.surface.config.width;
        let height = self.surface.config.height;
        if width == 0 || height == 0 {
            return;
        }

        self.build_scene(width, height, text);

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

    fn build_scene(&mut self, width: u32, height: u32, text: &str) {
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
        let circle = Circle::new((width as f64 - 72.0, height as f64 - 72.0), radius);
        self.scene
            .fill(Fill::NonZero, Affine::IDENTITY, ACCENT_COLOR, None, &circle);

        let max_text_width = (width as f32 - (TEXT_INSET as f32 * 2.0)).max(1.0);
        self.text_layout.draw_text(
            &mut self.scene,
            text,
            max_text_width,
            (TEXT_INSET, TEXT_INSET),
        );
    }
}

#[derive(Default)]
struct App {
    buffer: EditorBuffer,
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

    fn handle_key_press(&mut self, event_loop: &ActiveEventLoop, event: &KeyEvent) -> bool {
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                event_loop.exit();
                false
            }
            Key::Named(NamedKey::Backspace) => {
                self.buffer.backspace();
                true
            }
            _ => {
                let Some(text) = event.text.as_deref() else {
                    return false;
                };

                if is_printable_text(text) {
                    self.buffer.insert_str(text);
                    true
                } else {
                    false
                }
            }
        }
    }
}

fn is_printable_text(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|character| !character.is_control())
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
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if self.handle_key_press(event_loop, &event) {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let current_text = self.buffer.visible_text();
                if let Some(renderer) = &mut self.renderer {
                    renderer.render(&current_text);
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
    use super::{EditorBuffer, is_printable_text};

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
