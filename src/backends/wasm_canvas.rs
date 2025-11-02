use crate::{DisplayBackend, PixelFormat, VideoBufferError};
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, ImageData};

/// Display backend for WASM using HTML Canvas 2D context
///
/// This backend blits RGBA8 pixel data directly to a canvas element
/// using the Canvas 2D API's ImageData and putImageData methods.
pub struct WasmCanvasBackend {
    ctx: CanvasRenderingContext2d,
    width: u32,
    height: u32,
}

impl WasmCanvasBackend {
    /// Create a new WasmCanvasBackend with the given 2D rendering context
    pub fn new(ctx: CanvasRenderingContext2d) -> Self {
        Self {
            ctx,
            width: 0,
            height: 0,
        }
    }
}

impl DisplayBackend for WasmCanvasBackend {
    const FORMAT: PixelFormat = PixelFormat::Rgba8;

    fn init(&mut self, width: u32, height: u32) -> Result<(), VideoBufferError> {
        self.width = width;
        self.height = height;
        Ok(())
    }

    fn present(&mut self, frame: &[u8]) -> Result<(), VideoBufferError> {
        let image_data =
            ImageData::new_with_u8_clamped_array_and_sh(Clamped(frame), self.width, self.height)
                .map_err(|e| {
                    VideoBufferError::PresentFailed(format!("Failed to create ImageData: {:?}", e))
                })?;

        self.ctx
            .put_image_data(&image_data, 0.0, 0.0)
            .map_err(|e| {
                VideoBufferError::PresentFailed(format!("Failed to put ImageData: {:?}", e))
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_format() {
        assert_eq!(WasmCanvasBackend::FORMAT, PixelFormat::Rgba8);
    }
}
