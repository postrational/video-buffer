use crate::{PixelFormat, VideoBufferError};

pub trait Renderer {
    const FORMAT: PixelFormat;

    fn render(&mut self, frame: &mut [u8], width: u32, height: u32);
}

pub trait DisplayBackend {
    const FORMAT: PixelFormat;

    fn init(&mut self, width: u32, height: u32) -> Result<(), VideoBufferError>;

    fn present(&mut self, frame: &[u8]) -> Result<(), VideoBufferError>;
}
