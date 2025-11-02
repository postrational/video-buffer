use crate::{
    buffer::TripleBuffer,
    convert::{convert, needs_conversion},
    DisplayBackend, PixelFormat, Renderer, VideoBufferError,
};

/// Handles presentation: reads from buffer, converts format, and displays
///
/// This is useful for parallel rendering where you want the buffer shared
/// between threads but the backend is only accessed from the main thread.
pub struct DisplayPresenter<B: DisplayBackend> {
    backend: B,
    source_format: PixelFormat,
    convert_buffer: Option<Vec<u8>>,
    max_fps: Option<f64>,
    last_present_time_ms: f64,
}

impl<B: DisplayBackend> DisplayPresenter<B> {
    pub fn new(
        mut backend: B,
        width: u32,
        height: u32,
        source_format: PixelFormat,
    ) -> Result<Self, VideoBufferError> {
        backend.init(width, height)?;

        let convert_buffer = if needs_conversion(source_format, B::FORMAT) {
            let size = B::FORMAT.buffer_size(width, height);
            Some(vec![0u8; size])
        } else {
            None
        };

        Ok(Self {
            backend,
            source_format,
            convert_buffer,
            max_fps: None,
            last_present_time_ms: 0.0,
        })
    }

    /// Configure maximum FPS for frame rate limiting
    pub fn with_max_fps(mut self, fps: f64) -> Self {
        self.max_fps = Some(fps);
        self
    }

    /// Present a frame from the given buffer with optional timing control
    ///
    /// Returns `true` if the frame was presented, `false` if it was skipped due to timing.
    pub fn present(
        &mut self,
        buffer: &TripleBuffer,
        now_ms: f64,
    ) -> Result<bool, VideoBufferError> {
        // Check if enough time has elapsed
        if let Some(max_fps) = self.max_fps {
            let min_interval = 1000.0 / max_fps;
            if now_ms - self.last_present_time_ms < min_interval {
                return Ok(false); // Too soon, skip frame
            }
        }

        buffer.commit_present();
        let present_buf = buffer.present_buffer();

        let present_buffer = if let Some(ref mut convert_buf) = self.convert_buffer {
            convert(&present_buf, convert_buf, self.source_format, B::FORMAT);
            convert_buf.as_slice()
        } else {
            &present_buf[..]
        };

        self.backend.present(present_buffer)?;
        self.last_present_time_ms = now_ms;
        Ok(true)
    }

    /// Present a raw frame directly (for use with FrameQueue)
    ///
    /// Returns `true` if the frame was presented, `false` if it was skipped due to timing.
    pub fn present_frame(&mut self, frame: &[u8], now_ms: f64) -> Result<bool, VideoBufferError> {
        // Check if enough time has elapsed
        if let Some(max_fps) = self.max_fps {
            let min_interval = 1000.0 / max_fps;
            if now_ms - self.last_present_time_ms < min_interval {
                return Ok(false); // Too soon, skip frame
            }
        }

        // Convert if needed
        let present_buffer = if let Some(ref mut convert_buf) = self.convert_buffer {
            convert(frame, convert_buf, self.source_format, B::FORMAT);
            convert_buf.as_slice()
        } else {
            frame
        };

        self.backend.present(present_buffer)?;
        self.last_present_time_ms = now_ms;
        Ok(true)
    }
}

pub struct DisplayBridge<B: DisplayBackend> {
    buffer: TripleBuffer,
    backend: B,
    convert_buffer: Option<Vec<u8>>,
}

impl<B: DisplayBackend> DisplayBridge<B> {
    pub fn new(
        mut backend: B,
        width: u32,
        height: u32,
        renderer_format: PixelFormat,
    ) -> Result<Self, VideoBufferError> {
        backend.init(width, height)?;

        let buffer = TripleBuffer::new(width, height, renderer_format);

        let convert_buffer = if needs_conversion(renderer_format, B::FORMAT) {
            let size = B::FORMAT.buffer_size(width, height);
            Some(vec![0u8; size])
        } else {
            None
        };

        Ok(Self {
            buffer,
            backend,
            convert_buffer,
        })
    }

    /// Single-threaded rendering: render → swap → swap → present (all inline)
    ///
    /// This is the simplest API for single-threaded rendering. For parallel
    /// rendering, use `TripleBuffer` + `DisplayPresenter` instead.
    pub fn render_frame<R: Renderer>(&mut self, renderer: &mut R) -> Result<(), VideoBufferError> {
        let width = self.buffer.width();
        let height = self.buffer.height();

        // Render to current render buffer
        {
            let mut render_buf = self.buffer.render_buffer();
            renderer.render(&mut render_buf, width, height);
        }

        // Swap render ↔ ready
        self.buffer.commit_render();

        // Swap ready ↔ present
        self.buffer.commit_present();

        // Present
        let present_buf = self.buffer.present_buffer();

        let present_buffer = if let Some(ref mut convert_buf) = self.convert_buffer {
            convert(&present_buf, convert_buf, self.buffer.format(), B::FORMAT);
            convert_buf.as_slice()
        } else {
            &present_buf[..]
        };

        self.backend.present(present_buffer)?;

        Ok(())
    }

    pub fn width(&self) -> u32 {
        self.buffer.width()
    }

    pub fn height(&self) -> u32 {
        self.buffer.height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRenderer {
        render_count: usize,
    }

    impl MockRenderer {
        fn new() -> Self {
            Self { render_count: 0 }
        }
    }

    impl Renderer for MockRenderer {
        const FORMAT: PixelFormat = PixelFormat::Rgba8;

        fn render(&mut self, frame: &mut [u8], width: u32, height: u32) {
            self.render_count += 1;
            let expected_size = (width * height * 4) as usize;
            assert_eq!(frame.len(), expected_size);

            for i in 0..frame.len() {
                frame[i] = ((self.render_count + i) % 256) as u8;
            }
        }
    }

    struct MockBackend {
        init_called: bool,
        present_count: usize,
        last_frame: Vec<u8>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                init_called: false,
                present_count: 0,
                last_frame: Vec::new(),
            }
        }
    }

    impl DisplayBackend for MockBackend {
        const FORMAT: PixelFormat = PixelFormat::Rgba8;

        fn init(&mut self, _width: u32, _height: u32) -> Result<(), VideoBufferError> {
            self.init_called = true;
            Ok(())
        }

        fn present(&mut self, frame: &[u8]) -> Result<(), VideoBufferError> {
            self.present_count += 1;
            self.last_frame = frame.to_vec();
            Ok(())
        }
    }

    #[test]
    fn test_bridge_creation() {
        let backend = MockBackend::new();
        let bridge = DisplayBridge::new(backend, 320, 200, PixelFormat::Rgba8).unwrap();

        assert_eq!(bridge.width(), 320);
        assert_eq!(bridge.height(), 200);
        assert!(bridge.backend.init_called);
    }

    #[test]
    fn test_render_frame_no_conversion() {
        let backend = MockBackend::new();
        let mut bridge = DisplayBridge::new(backend, 100, 100, PixelFormat::Rgba8).unwrap();
        let mut renderer = MockRenderer::new();

        assert!(bridge.convert_buffer.is_none());

        bridge.render_frame(&mut renderer).unwrap();

        assert_eq!(renderer.render_count, 1);
        assert_eq!(bridge.backend.present_count, 1);
        assert_eq!(bridge.backend.last_frame.len(), 100 * 100 * 4);
    }

    #[test]
    fn test_multiple_frames() {
        let backend = MockBackend::new();
        let mut bridge = DisplayBridge::new(backend, 50, 50, PixelFormat::Rgba8).unwrap();
        let mut renderer = MockRenderer::new();

        for i in 0..10 {
            bridge.render_frame(&mut renderer).unwrap();
            assert_eq!(renderer.render_count, i + 1);
        }

        assert_eq!(bridge.backend.present_count, 10);
    }

    #[test]
    fn test_triple_buffer_cycling() {
        let backend = MockBackend::new();
        let mut bridge = DisplayBridge::new(backend, 10, 10, PixelFormat::Rgba8).unwrap();
        let mut renderer = MockRenderer::new();

        // Render 3 frames to ensure all buffers are cycled
        for _ in 0..3 {
            bridge.render_frame(&mut renderer).unwrap();
        }

        assert_eq!(renderer.render_count, 3);
        assert_eq!(bridge.backend.present_count, 3);
    }
}
