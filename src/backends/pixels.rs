use crate::{DisplayBackend, PixelFormat, VideoBufferError};
use pixels::{Pixels, SurfaceTexture};
use winit::window::Window;

pub struct PixelsBackend<'win> {
    pixels: Option<Pixels<'win>>,
}

impl<'win> PixelsBackend<'win> {
    pub fn new() -> Self {
        Self { pixels: None }
    }

    pub fn init_with_window(
        &mut self,
        width: u32,
        height: u32,
        window: &'win Window,
    ) -> Result<(), VideoBufferError> {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window);

        let pixels = Pixels::new(width, height, surface_texture)
            .map_err(|e| VideoBufferError::InitFailed(format!("Failed to create Pixels: {}", e)))?;

        self.pixels = Some(pixels);
        Ok(())
    }
}

impl<'win> DisplayBackend for PixelsBackend<'win> {
    const FORMAT: PixelFormat = PixelFormat::Rgba8;

    fn init(&mut self, _width: u32, _height: u32) -> Result<(), VideoBufferError> {
        // Init is idempotent - if already initialized via init_with_window(), do nothing
        if self.pixels.is_some() {
            return Ok(());
        }

        Err(VideoBufferError::InitFailed(
            "PixelsBackend requires init_with_window() to be called before use".to_string(),
        ))
    }

    fn present(&mut self, frame: &[u8]) -> Result<(), VideoBufferError> {
        let pixels = self
            .pixels
            .as_mut()
            .ok_or(VideoBufferError::NotInitialized)?;

        let pixels_frame = pixels.frame_mut();
        pixels_frame.copy_from_slice(frame);

        pixels
            .render()
            .map_err(|e| VideoBufferError::PresentFailed(format!("Render failed: {}", e)))?;

        Ok(())
    }
}

impl<'win> Default for PixelsBackend<'win> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_creation() {
        let backend = PixelsBackend::new();
        assert!(backend.pixels.is_none());
    }

    #[test]
    fn test_init_without_window_fails() {
        let mut backend = PixelsBackend::new();
        let result = backend.init(640, 480);
        assert!(result.is_err());
    }

    #[test]
    fn test_present_without_init_fails() {
        let mut backend = PixelsBackend::new();
        let frame = vec![0u8; 640 * 480 * 4];
        let result = backend.present(&frame);
        assert!(matches!(result, Err(VideoBufferError::NotInitialized)));
    }
}
