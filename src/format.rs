#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8-bit channels in R, G, B, A order.
    Rgba8,
    /// 8-bit channels in premultiplied A, R, G, B order (P = Premultiplied Alpha).
    Prgb8,
}

impl PixelFormat {
    /// Returns the number of bytes per pixel for this format.
    #[inline]
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Rgba8 | PixelFormat::Prgb8 => 4,
        }
    }

    /// Calculates the stride (bytes per row) for the given width.
    #[inline]
    pub const fn stride(self, width: u32) -> usize {
        width as usize * self.bytes_per_pixel()
    }

    /// Calculates the total buffer size needed for the given dimensions.
    #[inline]
    pub const fn buffer_size(self, width: u32, height: u32) -> usize {
        self.stride(width) * height as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_per_pixel() {
        assert_eq!(PixelFormat::Rgba8.bytes_per_pixel(), 4);
        assert_eq!(PixelFormat::Prgb8.bytes_per_pixel(), 4);
    }

    #[test]
    fn test_stride() {
        assert_eq!(PixelFormat::Rgba8.stride(320), 1280);
        assert_eq!(PixelFormat::Prgb8.stride(100), 400);
    }

    #[test]
    fn test_buffer_size() {
        assert_eq!(PixelFormat::Rgba8.buffer_size(320, 200), 256_000);
        assert_eq!(PixelFormat::Prgb8.buffer_size(640, 480), 1_228_800);
    }
}
