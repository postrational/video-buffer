use crate::PixelFormat;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

pub struct TripleBuffer {
    buffers: [Mutex<Vec<u8>>; 3],
    render_idx: AtomicUsize,
    ready_idx: AtomicUsize,
    present_idx: AtomicUsize,
    width: u32,
    height: u32,
    format: PixelFormat,
}

impl TripleBuffer {
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        assert!(width > 0, "width must be greater than 0");
        assert!(height > 0, "height must be greater than 0");

        let size = format.buffer_size(width, height);
        Self {
            buffers: [
                Mutex::new(vec![0u8; size]),
                Mutex::new(vec![0u8; size]),
                Mutex::new(vec![0u8; size]),
            ],
            render_idx: AtomicUsize::new(0),
            ready_idx: AtomicUsize::new(1),
            present_idx: AtomicUsize::new(2),
            width,
            height,
            format,
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

    /// Get the buffer for rendering
    pub fn render_buffer(&self) -> std::sync::MutexGuard<'_, Vec<u8>> {
        let idx = self.render_idx.load(Ordering::Acquire);
        self.buffers[idx].lock().unwrap()
    }

    /// Commit the rendered buffer
    pub fn commit_render(&self) {
        let render = self.render_idx.load(Ordering::Acquire);
        let ready = self.ready_idx.load(Ordering::Acquire);
        self.render_idx.store(ready, Ordering::Release);
        self.ready_idx.store(render, Ordering::Release);
    }

    /// Get the buffer for presentation
    pub fn present_buffer(&self) -> std::sync::MutexGuard<'_, Vec<u8>> {
        let idx = self.present_idx.load(Ordering::Acquire);
        self.buffers[idx].lock().unwrap()
    }

    /// Commit the presentation completed
    pub fn commit_present(&self) {
        let ready = self.ready_idx.load(Ordering::Acquire);
        let present = self.present_idx.load(Ordering::Acquire);
        self.ready_idx.store(present, Ordering::Release);
        self.present_idx.store(ready, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triple_buffer_creation() {
        let tb = TripleBuffer::new(320, 200, PixelFormat::Rgba8);
        assert_eq!(tb.width(), 320);
        assert_eq!(tb.height(), 200);
        assert_eq!(tb.format(), PixelFormat::Rgba8);
    }

    #[test]
    fn test_triple_buffer_swapping() {
        let tb = TripleBuffer::new(100, 100, PixelFormat::Rgba8);

        // Write to render buffer
        {
            let mut render = tb.render_buffer();
            render[0] = 42;
        }
        tb.commit_render();

        // Swap to present
        tb.commit_present();

        // Verify we can read from present
        let present = tb.present_buffer();
        assert_eq!(present[0], 42);
    }

    #[test]
    fn test_triple_buffer_cycling() {
        let tb = TripleBuffer::new(10, 10, PixelFormat::Rgba8);

        // Frame 1
        {
            let mut render = tb.render_buffer();
            render[0] = 1;
        }
        tb.commit_render();
        tb.commit_present();

        // Frame 2
        {
            let mut render = tb.render_buffer();
            render[0] = 2;
        }
        tb.commit_render();
        tb.commit_present();

        // Frame 3
        {
            let mut render = tb.render_buffer();
            render[0] = 3;
        }
        tb.commit_render();
        tb.commit_present();

        // All three buffers should have been used
        let present = tb.present_buffer();
        assert_eq!(present[0], 3);
    }

    #[test]
    fn test_prgb8_format() {
        let tb = TripleBuffer::new(100, 100, PixelFormat::Prgb8);
        assert_eq!(tb.format(), PixelFormat::Prgb8);
        let render = tb.render_buffer();
        assert_eq!(render.len(), 100 * 100 * 4);
    }

    #[test]
    #[should_panic(expected = "width must be greater than 0")]
    fn test_zero_width() {
        TripleBuffer::new(0, 100, PixelFormat::Rgba8);
    }

    #[test]
    #[should_panic(expected = "height must be greater than 0")]
    fn test_zero_height() {
        TripleBuffer::new(100, 0, PixelFormat::Rgba8);
    }

    #[test]
    fn test_large_buffer() {
        let tb = TripleBuffer::new(1920, 1080, PixelFormat::Rgba8);
        assert_eq!(tb.width(), 1920);
        assert_eq!(tb.height(), 1080);
        let render = tb.render_buffer();
        assert_eq!(render.len(), 1920 * 1080 * 4);
    }
}
