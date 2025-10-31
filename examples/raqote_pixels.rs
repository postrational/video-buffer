use font_kit::family_name::FamilyName;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use raqote::*;
use std::sync::Arc;
use std::time::Instant;
use video_buffer::{backends::PixelsBackend, DisplayBridge, PixelFormat, Renderer};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

struct RaqoteRenderer {
    frame_count: u64,
    last_time: Instant,
    fps: f64,
    font: Arc<font_kit::font::Font>,
}

impl RaqoteRenderer {
    fn new() -> Self {
        let font = SystemSource::new()
            .select_best_match(&[FamilyName::SansSerif], &Properties::new())
            .unwrap()
            .load()
            .unwrap();

        Self {
            frame_count: 0,
            last_time: Instant::now(),
            fps: 0.0,
            font: Arc::new(font),
        }
    }
}

impl Renderer for RaqoteRenderer {
    const FORMAT: PixelFormat = PixelFormat::Prgb8;

    fn render(&mut self, frame: &mut [u8], width: u32, height: u32) {
        // Update FPS
        let now = Instant::now();
        let delta = now.duration_since(self.last_time).as_secs_f64();
        if delta > 0.0 {
            self.fps = 1.0 / delta;
        }
        self.last_time = now;

        let mut dt = DrawTarget::new(width as i32, height as i32);

        dt.clear(SolidSource::from_unpremultiplied_argb(255, 20, 20, 30));

        let x = ((self.frame_count as f32 * 3.0) % width as f32) as f32;
        let y = height as f32 / 2.0;

        let mut pb = PathBuilder::new();
        pb.arc(x, y, 40.0, 0.0, std::f32::consts::PI * 2.0);
        let circle = pb.finish();

        dt.fill(
            &circle,
            &Source::Solid(SolidSource::from_unpremultiplied_argb(255, 0, 200, 255)),
            &DrawOptions::new(),
        );

        // Draw FPS and frame counter text
        let text = format!("FPS: {:.0}  Frames: {}", self.fps, self.frame_count);
        dt.draw_text(
            &self.font,
            16.0,
            &text,
            Point::new(10.0, height as f32 - 10.0),
            &Source::Solid(SolidSource::from_unpremultiplied_argb(255, 255, 255, 255)),
            &DrawOptions::new(),
        );

        let src = dt.get_data();
        frame.copy_from_slice(bytemuck::cast_slice(src));

        self.frame_count += 1;
    }
}

struct App {
    window: Option<Box<Window>>,
    bridge: Option<DisplayBridge<PixelsBackend<'static>>>,
    renderer: RaqoteRenderer,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            bridge: None,
            renderer: RaqoteRenderer::new(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attrs = WindowAttributes::default()
                .with_title("Raqote + Pixels Video Buffer Demo")
                .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

            let window = event_loop
                .create_window(window_attrs)
                .expect("Failed to create window");

            let window_box = Box::new(window);
            let window_ref: &'static Window = unsafe { std::mem::transmute(window_box.as_ref()) };

            let mut backend = PixelsBackend::new();
            backend
                .init_with_window(800, 600, window_ref)
                .expect("Failed to init backend");

            let bridge = DisplayBridge::new(backend, 800, 600, PixelFormat::Prgb8)
                .expect("Failed to create bridge");

            self.window = Some(window_box);
            self.bridge = Some(bridge);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                // Drop bridge before exiting to prevent use-after-free
                self.bridge = None;
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(ref mut bridge) = self.bridge {
                    if let Err(e) = bridge.render_frame(&mut self.renderer) {
                        eprintln!("Render error: {}", e);
                        event_loop.exit();
                    }
                }
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref window) = self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
