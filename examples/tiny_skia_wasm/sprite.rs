use tiny_skia::{Pixmap, PixmapMut, PixmapPaint, Transform};

pub struct Airplane {
    x: f32,
    y: f32,
    velocity_x: f32,
    velocity_y: f32,
    time: f32,
    speed: f32,
    arc_center_x: f32,
    arc_center_y: f32,
    arc_radius: f32,
}

impl Airplane {
    pub(crate) fn new(x: f32, y: f32, angle: f32, seed: u32) -> Self {
        // Deterministic random arc_radius between 50 and 2000
        let arc_radius = 50.0 + ((seed * 769) % 1951) as f32;

        // Adjust speed inversely to arc_radius to maintain constant linear speed
        // Linear speed = arc_radius * angular_speed
        let target_linear_speed = 2.0;
        let speed = target_linear_speed / arc_radius;

        let arc_center_x = x - arc_radius * angle.cos();
        let arc_center_y = y - arc_radius * angle.sin();

        Self {
            x,
            y,
            velocity_x: 0.0,
            velocity_y: 0.0,
            time: angle,
            speed,
            arc_center_x,
            arc_center_y,
            arc_radius,
        }
    }

    pub(crate) fn update(&mut self, frame_no: u64) {
        // Calculate time based on frame number for deterministic animation
        let angle = self.time + self.speed * frame_no as f32;

        // Calculate position based on the circular path
        self.x = self.arc_center_x + self.arc_radius * angle.cos();
        self.y = self.arc_center_y + self.arc_radius * angle.sin();

        // Calculate velocity for rotation
        self.velocity_x = -self.arc_radius * self.speed * angle.sin();
        self.velocity_y = self.arc_radius * self.speed * angle.cos();
    }

    fn rotation_angle(&self) -> f32 {
        self.velocity_y.atan2(self.velocity_x) + std::f32::consts::FRAC_PI_2
    }

    pub(crate) fn draw(&self, pixmap: &mut PixmapMut, sprite: &Pixmap) {
        let sprite_center_x = sprite.width() as f32 / 2.0;
        let sprite_center_y = sprite.height() as f32 / 2.0;

        let transform = Transform::from_translate(-sprite_center_x, -sprite_center_y)
            .post_concat(Transform::from_rotate(self.rotation_angle().to_degrees()))
            .post_concat(Transform::from_translate(self.x, self.y));

        pixmap.draw_pixmap(
            0,
            0,
            sprite.as_ref(),
            &PixmapPaint::default(),
            transform,
            None,
        );
    }
}
