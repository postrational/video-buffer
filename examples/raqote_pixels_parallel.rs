use font_kit::family_name::FamilyName;
use font_kit::properties::Properties;
use font_kit::source::SystemSource;
use raqote::*;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use video_buffer::{
    backends::PixelsBackend, DisplayPresenter, PixelFormat, Renderer, TripleBuffer,
};
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

        let text = format!(
            "FPS: {:.0}  Frames: {} (Parallel)",
            self.fps, self.frame_count
        );
        dt.draw_text(
            &self.font,
            16.0,
            &text,
            Point::new(10.0, height as f32 - 10.0),
            &Source::Solid(SolidSource::from_unpremultiplied_argb(255, 255, 255, 255)),
            &DrawOptions::new(),
        );

        frame.copy_from_slice(bytemuck::cast_slice(dt.get_data()));
        self.frame_count += 1;
    }
}

struct App {
    window: Option<Box<Window>>,
    presenter: Option<DisplayPresenter<PixelsBackend<'static>>>,
    buffer: Option<Arc<TripleBuffer>>,
    worker: Option<thread::JoinHandle<()>>,
    stop_tx: Option<std::sync::mpsc::Sender<()>>,
    start_time: Instant,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            presenter: None,
            buffer: None,
            worker: None,
            stop_tx: None,
            start_time: Instant::now(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_title("Parallel Rendering")
                    .with_inner_size(winit::dpi::LogicalSize::new(800, 600)),
            )
            .unwrap();

        let window_box = Box::new(window);
        let window_ref: &'static Window = unsafe { std::mem::transmute(window_box.as_ref()) };

        let mut backend = PixelsBackend::new();
        backend.init_with_window(800, 600, window_ref).unwrap();

        let presenter = DisplayPresenter::new(backend, 800, 600, PixelFormat::Prgb8).unwrap();
        let buffer = Arc::new(TripleBuffer::new(800, 600, PixelFormat::Prgb8));

        // Start worker thread
        let buffer_clone = Arc::clone(&buffer);
        let (tx, rx) = channel();
        let worker = thread::spawn(move || {
            let mut renderer = RaqoteRenderer::new();
            let target = std::time::Duration::from_secs_f64(1.0 / 120.0);

            loop {
                if rx.try_recv().is_ok() {
                    break;
                }

                let start = Instant::now();
                renderer.render(&mut buffer_clone.render_buffer(), 800, 600);
                buffer_clone.commit_render();

                if let Some(sleep) = target.checked_sub(start.elapsed()) {
                    thread::sleep(sleep);
                }
            }
        });

        self.window = Some(window_box);
        self.presenter = Some(presenter);
        self.buffer = Some(buffer);
        self.worker = Some(worker);
        self.stop_tx = Some(tx);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // Stop worker thread first
                if let Some(tx) = self.stop_tx.take() {
                    let _ = tx.send(());
                }

                // Wait for worker to finish
                if let Some(worker) = self.worker.take() {
                    worker.join().ok();
                }

                // Drop presenter and buffer before exiting
                self.presenter = None;
                self.buffer = None;

                // Now safe to exit
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let (Some(ref buffer), Some(ref mut presenter)) =
                    (&self.buffer, &mut self.presenter)
                {
                    let now_ms = self.start_time.elapsed().as_secs_f64() * 1000.0;
                    presenter.present(buffer, now_ms).unwrap();
                }

                self.window.as_ref().unwrap().request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(ref w) = self.window {
            w.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    EventLoop::new()?.run_app(&mut App::new())?;
    Ok(())
}
