use crate::sprite::Airplane;
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::Font;
use tiny_skia::{Color, Pixmap, PixmapMut, PixmapPaint, Transform};

pub struct TinySkiaRenderer {
    font: Font,
    airplanes: Vec<Airplane>,
    sprite_rotations: Vec<Pixmap>,
    num_workers: usize,
}

impl TinySkiaRenderer {
    pub(crate) fn new(num_workers: usize, canvas_width: u32, canvas_height: u32) -> Self {
        let font_data = include_bytes!("./assets/Roboto-Regular.ttf");
        let font = Font::from_bytes(font_data as &[u8], fontdue::FontSettings::default())
            .expect("Failed to load font");

        let airplane_data = include_bytes!("./assets/airplane.png");
        let mut sprite = Pixmap::decode_png(airplane_data).expect("Failed to load airplane.png");

        // Apply additional transparency effect by double-premultiplying alpha
        let data = sprite.data_mut();
        for i in 0..(data.len() / 4) {
            let idx = i * 4;
            let a = data[idx] as f32 / 255.0;
            data[idx + 1] = (data[idx + 1] as f32 * a) as u8; // R
            data[idx + 2] = (data[idx + 2] as f32 * a) as u8; // G
            data[idx + 3] = (data[idx + 3] as f32 * a) as u8; // B
        }

        // Pre-render 120 rotated versions (every 3 degrees)
        let sprite_center_x = sprite.width() as f32 / 2.0;
        let sprite_center_y = sprite.height() as f32 / 2.0;
        let mut sprite_rotations = Vec::with_capacity(120);

        for i in 0..120 {
            let angle_degrees = i as f32 * 3.0;
            let mut rotated = Pixmap::new(sprite.width(), sprite.height())
                .expect("Failed to create rotated sprite pixmap");

            let transform = Transform::from_translate(-sprite_center_x, -sprite_center_y)
                .post_concat(Transform::from_rotate(angle_degrees))
                .post_concat(Transform::from_translate(sprite_center_x, sprite_center_y));

            rotated.as_mut().draw_pixmap(
                0,
                0,
                sprite.as_ref(),
                &PixmapPaint::default(),
                transform,
                None,
            );

            sprite_rotations.push(rotated);
        }

        // Generate ALL airplanes with deterministic seeded positions
        // Use actual canvas dimensions for positioning
        let mut airplanes = Vec::new();
        for i in 0..10000 {
            // Use deterministic "random" values based on index
            // This ensures all workers generate identical initial states
            let x = ((i * 137) % canvas_width as usize) as f32;
            let y = ((i * 193) % canvas_height as usize) as f32;
            let angle = ((i * 77) % 360) as f32 * std::f32::consts::PI / 180.0;
            airplanes.push(Airplane::new(x, y, angle, i as u32));
        }

        Self {
            font,
            airplanes,
            sprite_rotations,
            num_workers,
        }
    }

    fn draw_text(&self, pixmap: &mut PixmapMut, text: &str, x: f32, y: f32, size: f32) {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings::default());
        layout.append(&[&self.font], &TextStyle::new(text, size, 0));

        for glyph in layout.glyphs() {
            let (metrics, bitmap) = self.font.rasterize_config(glyph.key);

            for (i, &alpha) in bitmap.iter().enumerate() {
                if alpha == 0 {
                    continue;
                }

                let glyph_x = (x + glyph.x + (i % metrics.width) as f32) as i32;
                let glyph_y = (y + glyph.y + (i / metrics.width) as f32) as i32;

                if glyph_x >= 0
                    && glyph_x < pixmap.width() as i32
                    && glyph_y >= 0
                    && glyph_y < pixmap.height() as i32
                {
                    let idx = (glyph_y as usize * pixmap.width() as usize + glyph_x as usize) * 4;
                    let data = pixmap.data_mut();

                    if idx + 3 < data.len() {
                        let alpha_f = alpha as f32 / 255.0;
                        data[idx] = 255;
                        data[idx + 1] = (255.0 * alpha_f) as u8;
                        data[idx + 2] = (255.0 * alpha_f) as u8;
                        data[idx + 3] = (255.0 * alpha_f) as u8;
                    }
                }
            }
        }
    }

    pub(crate) fn render_to_rgba(
        &mut self,
        frame: &mut [u8],
        width: u32,
        height: u32,
        frame_no: u64,
        fps: f64,
    ) {
        // Render to ARGB premultiplied first
        let mut pixmap = Pixmap::new(width, height).expect("Failed to create pixmap");
        let mut pixmap_mut = pixmap.as_mut();

        pixmap_mut.fill(Color::from_rgba8(0, 0, 0, 255));

        for airplane in &mut self.airplanes {
            airplane.update(frame_no);
            airplane.draw(&mut pixmap_mut, &self.sprite_rotations);
        }

        // Render frame number and FPS
        let text = format!(
            "Frame: {}  FPS: {:.1}  Planes: {} ({} workers)",
            frame_no,
            fps,
            self.airplanes.len(),
            self.num_workers,
        );
        self.draw_text(&mut pixmap_mut, &text, 10.0, height as f32 - 36.0, 20.0);

        // Convert ARGB premultiplied to RGBA for canvas
        // Simple channel swap
        let argb_data = pixmap.data();
        for i in 0..(frame.len() / 4) {
            let idx = i * 4;
            frame[idx] = argb_data[idx + 1]; // R
            frame[idx + 1] = argb_data[idx + 2]; // G
            frame[idx + 2] = argb_data[idx + 3]; // B
            frame[idx + 3] = argb_data[idx]; // A
        }
    }
}
