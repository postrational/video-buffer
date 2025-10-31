use crate::{
    convert::{convert, needs_conversion},
    DisplayBackend, DoubleBuffer, DoubleBufferError, PixelFormat, Renderer,
};

pub struct DisplayBridge<B: DisplayBackend> {
    buffer: DoubleBuffer,
    backend: B,
    convert_buffer: Option<Vec<u8>>,
}

impl<B: DisplayBackend> DisplayBridge<B> {
    pub fn new(
        mut backend: B,
        width: u32,
        height: u32,
        renderer_format: PixelFormat,
    ) -> Result<Self, DoubleBufferError> {
        backend.init(width, height)?;

        let buffer = DoubleBuffer::new(width, height, renderer_format);

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

    pub fn render_frame<R: Renderer>(&mut self, renderer: &mut R) -> Result<(), DoubleBufferError> {
        let width = self.buffer.width();
        let height = self.buffer.height();

        renderer.render(self.buffer.back_mut(), width, height);

        self.buffer.swap();

        let present_buffer = if let Some(ref mut convert_buf) = self.convert_buffer {
            convert(
                self.buffer.front(),
                convert_buf,
                self.buffer.format(),
                B::FORMAT,
            );
            convert_buf.as_slice()
        } else {
            self.buffer.front()
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

        fn init(&mut self, _width: u32, _height: u32) -> Result<(), DoubleBufferError> {
            self.init_called = true;
            Ok(())
        }

        fn present(&mut self, frame: &[u8]) -> Result<(), DoubleBufferError> {
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
}
