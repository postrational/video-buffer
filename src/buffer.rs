use crate::PixelFormat;

pub struct DoubleBuffer {
    width: u32,
    height: u32,
    format: PixelFormat,
    front: Vec<u8>,
    back: Vec<u8>,
}

impl DoubleBuffer {
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        assert!(width > 0, "width must be greater than 0");
        assert!(height > 0, "height must be greater than 0");

        let buffer_size = format.buffer_size(width, height);
        let front = vec![0u8; buffer_size];
        let back = vec![0u8; buffer_size];

        Self {
            width,
            height,
            format,
            front,
            back,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> PixelFormat {
        self.format
    }

    pub fn back_mut(&mut self) -> &mut [u8] {
        &mut self.back
    }

    pub fn front(&self) -> &[u8] {
        &self.front
    }

    pub fn swap(&mut self) {
        std::mem::swap(&mut self.front, &mut self.back);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_creation() {
        let mut buffer = DoubleBuffer::new(320, 200, PixelFormat::Rgba8);
        assert_eq!(buffer.width(), 320);
        assert_eq!(buffer.height(), 200);
        assert_eq!(buffer.format(), PixelFormat::Rgba8);
        assert_eq!(buffer.front().len(), 320 * 200 * 4);
        assert_eq!(buffer.back_mut().len(), 320 * 200 * 4);
    }

    #[test]
    fn test_swap_operation() {
        let mut buffer = DoubleBuffer::new(10, 10, PixelFormat::Rgba8);

        // Write a pattern to the back buffer
        buffer.back_mut()[0] = 42;
        buffer.back_mut()[1] = 99;

        // Front should still be zeros
        assert_eq!(buffer.front()[0], 0);
        assert_eq!(buffer.front()[1], 0);

        // Swap
        buffer.swap();

        // Now front should have the pattern
        assert_eq!(buffer.front()[0], 42);
        assert_eq!(buffer.front()[1], 99);

        // And back should be zeros
        assert_eq!(buffer.back_mut()[0], 0);
        assert_eq!(buffer.back_mut()[1], 0);
    }

    #[test]
    fn test_multiple_swaps() {
        let mut buffer = DoubleBuffer::new(10, 10, PixelFormat::Rgba8);

        // First write
        buffer.back_mut()[0] = 1;
        buffer.swap();
        assert_eq!(buffer.front()[0], 1);

        // Second write
        buffer.back_mut()[0] = 2;
        buffer.swap();
        assert_eq!(buffer.front()[0], 2);

        // Third write
        buffer.back_mut()[0] = 3;
        buffer.swap();
        assert_eq!(buffer.front()[0], 3);
    }

    #[test]
    fn test_small_buffer() {
        let mut buffer = DoubleBuffer::new(1, 1, PixelFormat::Rgba8);
        assert_eq!(buffer.width(), 1);
        assert_eq!(buffer.height(), 1);
        assert_eq!(buffer.front().len(), 4);
        assert_eq!(buffer.back_mut().len(), 4);
    }

    #[test]
    fn test_large_buffer() {
        let mut buffer = DoubleBuffer::new(1920, 1080, PixelFormat::Rgba8);
        assert_eq!(buffer.width(), 1920);
        assert_eq!(buffer.height(), 1080);
        assert_eq!(buffer.front().len(), 1920 * 1080 * 4);
        assert_eq!(buffer.back_mut().len(), 1920 * 1080 * 4);
    }

    #[test]
    fn test_prgb8_format() {
        let buffer = DoubleBuffer::new(100, 100, PixelFormat::Prgb8);
        assert_eq!(buffer.format(), PixelFormat::Prgb8);
        assert_eq!(buffer.front().len(), 100 * 100 * 4);
    }

    #[test]
    #[should_panic(expected = "width must be greater than 0")]
    fn test_zero_width() {
        DoubleBuffer::new(0, 100, PixelFormat::Rgba8);
    }

    #[test]
    #[should_panic(expected = "height must be greater than 0")]
    fn test_zero_height() {
        DoubleBuffer::new(100, 0, PixelFormat::Rgba8);
    }
}
