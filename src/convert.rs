use crate::PixelFormat;

#[inline]
pub fn needs_conversion(src_format: PixelFormat, dst_format: PixelFormat) -> bool {
    src_format != dst_format
}

#[inline]
pub fn convert(src: &[u8], dst: &mut [u8], src_format: PixelFormat, dst_format: PixelFormat) {
    match (src_format, dst_format) {
        (PixelFormat::Prgb8, PixelFormat::Rgba8) => convert_prgb_to_rgba(src, dst),
        (PixelFormat::Rgba8, PixelFormat::Prgb8) => convert_rgba_to_prgb(src, dst),
        _ => unreachable!("convert should only be called when formats differ"),
    }
}

#[inline]
pub fn convert_prgb_to_rgba(src: &[u8], dst: &mut [u8]) {
    assert_eq!(
        src.len(),
        dst.len(),
        "source and destination buffers must have the same length"
    );
    assert_eq!(src.len() % 4, 0, "buffer length must be a multiple of 4");

    for (src_pixel, dst_pixel) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        dst_pixel[0] = src_pixel[1]; // R
        dst_pixel[1] = src_pixel[2]; // G
        dst_pixel[2] = src_pixel[3]; // B
        dst_pixel[3] = src_pixel[0]; // A
    }
}

#[inline]
pub fn convert_rgba_to_prgb(src: &[u8], dst: &mut [u8]) {
    assert_eq!(
        src.len(),
        dst.len(),
        "source and destination buffers must have the same length"
    );
    assert_eq!(src.len() % 4, 0, "buffer length must be a multiple of 4");

    for (src_pixel, dst_pixel) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        dst_pixel[0] = src_pixel[3]; // A
        dst_pixel[1] = src_pixel[0]; // R
        dst_pixel[2] = src_pixel[1]; // G
        dst_pixel[3] = src_pixel[2]; // B
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_conversion() {
        assert!(needs_conversion(PixelFormat::Rgba8, PixelFormat::Prgb8));
        assert!(needs_conversion(PixelFormat::Prgb8, PixelFormat::Rgba8));
        assert!(!needs_conversion(PixelFormat::Rgba8, PixelFormat::Rgba8));
        assert!(!needs_conversion(PixelFormat::Prgb8, PixelFormat::Prgb8));
    }

    #[test]
    fn test_prgb_to_rgba_single_pixel() {
        let src = [255, 128, 64, 32]; // A=255, R=128, G=64, B=32
        let mut dst = [0u8; 4];
        convert_prgb_to_rgba(&src, &mut dst);
        assert_eq!(dst, [128, 64, 32, 255]); // R=128, G=64, B=32, A=255
    }

    #[test]
    fn test_rgba_to_prgb_single_pixel() {
        let src = [128, 64, 32, 255]; // R=128, G=64, B=32, A=255
        let mut dst = [0u8; 4];
        convert_rgba_to_prgb(&src, &mut dst);
        assert_eq!(dst, [255, 128, 64, 32]); // A=255, R=128, G=64, B=32
    }

    #[test]
    fn test_round_trip_prgb_rgba_prgb() {
        let original = [200, 100, 50, 25, 128, 64, 32, 16];
        let mut intermediate = [0u8; 8];
        let mut final_result = [0u8; 8];

        convert_prgb_to_rgba(&original, &mut intermediate);
        convert_rgba_to_prgb(&intermediate, &mut final_result);

        assert_eq!(original, final_result);
    }

    #[test]
    fn test_round_trip_rgba_prgb_rgba() {
        let original = [100, 50, 25, 200, 64, 32, 16, 128];
        let mut intermediate = [0u8; 8];
        let mut final_result = [0u8; 8];

        convert_rgba_to_prgb(&original, &mut intermediate);
        convert_prgb_to_rgba(&intermediate, &mut final_result);

        assert_eq!(original, final_result);
    }

    #[test]
    fn test_multiple_pixels() {
        let src = [
            255, 255, 0, 0, // pixel 1: A=255, R=255, G=0, B=0 (red)
            255, 0, 255, 0, // pixel 2: A=255, R=0, G=255, B=0 (green)
            255, 0, 0, 255, // pixel 3: A=255, R=0, G=0, B=255 (blue)
        ];
        let mut dst = [0u8; 12];
        convert_prgb_to_rgba(&src, &mut dst);

        let expected = [
            255, 0, 0, 255, // R=255, G=0, B=0, A=255
            0, 255, 0, 255, // R=0, G=255, B=0, A=255
            0, 0, 255, 255, // R=0, G=0, B=255, A=255
        ];
        assert_eq!(dst, expected);
    }

    #[test]
    fn test_empty_buffer() {
        let src: [u8; 0] = [];
        let mut dst: [u8; 0] = [];
        convert_prgb_to_rgba(&src, &mut dst);
        convert_rgba_to_prgb(&src, &mut dst);
    }

    #[test]
    fn test_image_round_trip_prgb() {
        // Simulate a 320x200 image with various pixel patterns
        let width = 320;
        let height = 200;
        let size = width * height * 4;

        let mut original = vec![0u8; size];

        // Fill with a pattern: gradient + checkerboard
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 4;
                let checker = ((x / 8) + (y / 8)) % 2;
                original[idx + 0] = (x % 256) as u8; // A: horizontal gradient
                original[idx + 1] = (y % 256) as u8; // R: vertical gradient
                original[idx + 2] = if checker == 0 { 128 } else { 64 }; // G: checkerboard
                original[idx + 3] = ((x + y) % 256) as u8; // B: diagonal gradient
            }
        }

        let mut intermediate = vec![0u8; size];
        let mut final_result = vec![0u8; size];

        // Convert PRGB -> RGBA -> PRGB
        convert_prgb_to_rgba(&original, &mut intermediate);
        convert_rgba_to_prgb(&intermediate, &mut final_result);

        assert_eq!(original, final_result);
    }

    #[test]
    fn test_image_round_trip_rgba() {
        // Simulate a 640x480 image with various pixel patterns
        let width = 640;
        let height = 480;
        let size = width * height * 4;

        let mut original = vec![0u8; size];

        // Fill with a different pattern
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 4;
                original[idx + 0] = ((x * y) % 256) as u8; // R: xy product
                original[idx + 1] = (x % 256) as u8; // G: horizontal gradient
                original[idx + 2] = (y % 256) as u8; // B: vertical gradient
                original[idx + 3] = ((x ^ y) % 256) as u8; // A: XOR pattern
            }
        }

        let mut intermediate = vec![0u8; size];
        let mut final_result = vec![0u8; size];

        // Convert RGBA -> PRGB -> RGBA
        convert_rgba_to_prgb(&original, &mut intermediate);
        convert_prgb_to_rgba(&intermediate, &mut final_result);

        assert_eq!(original, final_result);
    }

    #[test]
    #[should_panic(expected = "source and destination buffers must have the same length")]
    fn test_mismatched_buffer_lengths() {
        let src = [0u8; 8];
        let mut dst = [0u8; 4];
        convert_prgb_to_rgba(&src, &mut dst);
    }

    #[test]
    #[should_panic(expected = "buffer length must be a multiple of 4")]
    fn test_invalid_buffer_length() {
        let src = [0u8; 7];
        let mut dst = [0u8; 7];
        convert_prgb_to_rgba(&src, &mut dst);
    }
}
