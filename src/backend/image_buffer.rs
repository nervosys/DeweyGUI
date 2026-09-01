//! Headless image buffer painter.
//!
//! Renders to an in-memory RGBA pixel buffer. Useful for screenshot
//! generation, server-side rendering, and automated visual testing.

use crate::core::style::TextStyle;
use crate::core::{Color, Position, Rect, Size};
use crate::paint::{ImageData, Painter};

/// An in-memory RGBA painter that rasterizes into a pixel buffer.
pub struct ImagePainter {
    width: u32,
    height: u32,
    pixels: Vec<u8>, // RGBA
    clip_stack: Vec<Rect>,
}

impl ImagePainter {
    /// Create a new image buffer with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
            clip_stack: Vec::new(),
        }
    }

    /// Get the width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the raw RGBA pixel data.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Get the pixel color at (x, y).
    pub fn get_pixel(&self, x: u32, y: u32) -> Color {
        if x >= self.width || y >= self.height {
            return Color::TRANSPARENT;
        }
        let idx = ((y * self.width + x) * 4) as usize;
        Color::rgba(
            self.pixels[idx] as f32 / 255.0,
            self.pixels[idx + 1] as f32 / 255.0,
            self.pixels[idx + 2] as f32 / 255.0,
            self.pixels[idx + 3] as f32 / 255.0,
        )
    }

    /// Set a pixel, blending with alpha compositing.
    fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as u32, y as u32);
        if x >= self.width || y >= self.height {
            return;
        }

        // Clip check
        if let Some(clip) = self.clip_stack.last() {
            if (x as f32) < clip.x
                || (x as f32) >= clip.x + clip.width
                || (y as f32) < clip.y
                || (y as f32) >= clip.y + clip.height
            {
                return;
            }
        }

        let idx = ((y * self.width + x) * 4) as usize;
        let alpha = color.a;
        let inv = 1.0 - alpha;

        self.pixels[idx] =
            ((color.r * alpha + self.pixels[idx] as f32 / 255.0 * inv) * 255.0) as u8;
        self.pixels[idx + 1] =
            ((color.g * alpha + self.pixels[idx + 1] as f32 / 255.0 * inv) * 255.0) as u8;
        self.pixels[idx + 2] =
            ((color.b * alpha + self.pixels[idx + 2] as f32 / 255.0 * inv) * 255.0) as u8;
        self.pixels[idx + 3] =
            ((alpha + self.pixels[idx + 3] as f32 / 255.0 * inv) * 255.0).min(255.0) as u8;
    }

    /// Clear the buffer to a solid color.
    pub fn clear(&mut self, color: Color) {
        let r = (color.r * 255.0) as u8;
        let g = (color.g * 255.0) as u8;
        let b = (color.b * 255.0) as u8;
        let a = (color.a * 255.0) as u8;
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = a;
        }
    }

    fn fill_rect_pixels(&mut self, rect: Rect, color: Color) {
        let x0 = rect.x.max(0.0) as i32;
        let y0 = rect.y.max(0.0) as i32;
        let x1 = (rect.x + rect.width).min(self.width as f32) as i32;
        let y1 = (rect.y + rect.height).min(self.height as f32) as i32;

        for y in y0..y1 {
            for x in x0..x1 {
                self.set_pixel(x, y, color);
            }
        }
    }
}

impl Painter for ImagePainter {
    fn fill_rect(&mut self, rect: Rect, color: Color, _corner_radius: f32) {
        self.fill_rect_pixels(rect, color);
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, _corner_radius: f32) {
        // Top
        self.fill_rect_pixels(Rect::new(rect.x, rect.y, rect.width, width), color);
        // Bottom
        self.fill_rect_pixels(
            Rect::new(rect.x, rect.y + rect.height - width, rect.width, width),
            color,
        );
        // Left
        self.fill_rect_pixels(Rect::new(rect.x, rect.y, width, rect.height), color);
        // Right
        self.fill_rect_pixels(
            Rect::new(rect.x + rect.width - width, rect.y, width, rect.height),
            color,
        );
    }

    fn fill_circle(&mut self, center: Position, radius: f32, color: Color) {
        let r2 = radius * radius;
        let x0 = (center.x - radius).max(0.0) as i32;
        let y0 = (center.y - radius).max(0.0) as i32;
        let x1 = (center.x + radius).min(self.width as f32) as i32;
        let y1 = (center.y + radius).min(self.height as f32) as i32;

        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f32 - center.x;
                let dy = y as f32 - center.y;
                if dx * dx + dy * dy <= r2 {
                    self.set_pixel(x, y, color);
                }
            }
        }
    }

    fn stroke_circle(&mut self, center: Position, radius: f32, color: Color, width: f32) {
        let outer2 = radius * radius;
        let inner = (radius - width).max(0.0);
        let inner2 = inner * inner;

        let x0 = (center.x - radius).max(0.0) as i32;
        let y0 = (center.y - radius).max(0.0) as i32;
        let x1 = (center.x + radius).min(self.width as f32) as i32;
        let y1 = (center.y + radius).min(self.height as f32) as i32;

        for y in y0..=y1 {
            for x in x0..=x1 {
                let dx = x as f32 - center.x;
                let dy = y as f32 - center.y;
                let d2 = dx * dx + dy * dy;
                if d2 <= outer2 && d2 >= inner2 {
                    self.set_pixel(x, y, color);
                }
            }
        }
    }

    fn line(&mut self, from: Position, to: Position, color: Color, width: f32) {
        // Bresenham-style line with thickness
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let steps = (dx.abs().max(dy.abs())) as i32;
        if steps == 0 {
            self.set_pixel(from.x as i32, from.y as i32, color);
            return;
        }
        let half_w = (width / 2.0).max(0.5) as i32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = from.x + dx * t;
            let y = from.y + dy * t;
            for wy in -half_w..=half_w {
                for wx in -half_w..=half_w {
                    self.set_pixel(x as i32 + wx, y as i32 + wy, color);
                }
            }
        }
    }

    fn text(&mut self, pos: Position, text: &str, style: &TextStyle) {
        // Simple: draw a colored rectangle representing text bounds
        let size = self.measure_text(text, style);
        self.fill_rect_pixels(
            Rect::new(pos.x, pos.y, size.width, size.height),
            style.color,
        );
    }

    fn measure_text(&self, text: &str, style: &TextStyle) -> Size {
        let w = style.font_size * 0.6 * text.len() as f32;
        Size::new(w, style.font_size * 1.2)
    }

    fn push_clip(&mut self, rect: Rect) {
        self.clip_stack.push(rect);
    }

    fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    fn fill_path(&mut self, points: &[Position], color: Color) {
        // Even-odd scanline fill. Concave and self-intersecting paths are both
        // handled, which is the point of doing it properly here: this backend
        // is what headless tests and server-side rendering compare against, so
        // it should be the most correct implementation rather than the least.
        if points.len() < 3 {
            return;
        }
        let Some(bounds) = crate::paint::bounding_box(points) else {
            return;
        };

        let top = bounds.y.floor().max(0.0) as i32;
        let bottom = (bounds.y + bounds.height).ceil().min(self.height as f32) as i32;
        let mut crossings: Vec<f32> = Vec::with_capacity(points.len());

        for y in top..bottom {
            // Sample through the middle of the row, so a horizontal edge lying
            // exactly on an integer boundary neither doubles nor vanishes.
            let scan = y as f32 + 0.5;
            crossings.clear();

            for index in 0..points.len() {
                let a = points[index];
                let b = points[(index + 1) % points.len()];
                // Half-open in y: counting both endpoints would register a
                // shared vertex twice and invert the fill after it.
                if (a.y <= scan) != (b.y <= scan) {
                    let t = (scan - a.y) / (b.y - a.y);
                    crossings.push(a.x + t * (b.x - a.x));
                }
            }
            crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            for pair in crossings.chunks_exact(2) {
                let (start, end) = (pair[0].ceil() as i32, pair[1].floor() as i32);
                for x in start..=end {
                    self.set_pixel(x, y, color);
                }
            }
        }
    }

    fn draw_image(&mut self, rect: Rect, image: &ImageData<'_>) {
        if image.width == 0 || image.height == 0 || rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let left = rect.x.floor().max(0.0) as i32;
        let top = rect.y.floor().max(0.0) as i32;
        let right = (rect.x + rect.width).ceil().min(self.width as f32) as i32;
        let bottom = (rect.y + rect.height).ceil().min(self.height as f32) as i32;

        // Nearest-neighbour: this buffer exists for correctness checks and
        // screenshots, where a predictable sample beats a smoothed one.
        for y in top..bottom {
            let v = (y as f32 - rect.y) / rect.height;
            let source_y = ((v * image.height as f32) as u32).min(image.height - 1);
            for x in left..right {
                let u = (x as f32 - rect.x) / rect.width;
                let source_x = ((u * image.width as f32) as u32).min(image.width - 1);

                let [r, g, b, a] = image.pixel(source_x, source_y);
                if a == 0 {
                    continue;
                }
                self.set_pixel(
                    x,
                    y,
                    Color::rgba(
                        r as f32 / 255.0,
                        g as f32 / 255.0,
                        b as f32 / 255.0,
                        a as f32 / 255.0,
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_path_fills_a_concave_shape_correctly() {
        // An L: the notch must stay empty. A bounding-box approximation fills
        // it, so this is the test that says the scanline fill is real.
        let mut p = ImagePainter::new(40, 40);
        let l_shape = [
            Position::new(5.0, 5.0),
            Position::new(15.0, 5.0),
            Position::new(15.0, 25.0),
            Position::new(35.0, 25.0),
            Position::new(35.0, 35.0),
            Position::new(5.0, 35.0),
        ];
        p.fill_path(&l_shape, Color::RED);

        assert!(
            p.get_pixel(10, 10).r > 0.9,
            "the upper arm should be filled"
        );
        assert!(
            p.get_pixel(30, 30).r > 0.9,
            "the lower arm should be filled"
        );
        assert!(
            p.get_pixel(30, 10).a < 0.01,
            "the notch must stay empty — a bounding-box fill would cover it"
        );
    }

    #[test]
    fn fill_path_ignores_degenerate_input() {
        let mut p = ImagePainter::new(10, 10);
        p.fill_path(&[], Color::RED);
        p.fill_path(
            &[Position::new(1.0, 1.0), Position::new(2.0, 2.0)],
            Color::RED,
        );
        assert!(p.get_pixel(1, 1).a < 0.01);
    }

    #[test]
    fn draw_image_scales_pixels_into_the_target_rect() {
        // A 2x2 image with one red quadrant, scaled 10x. The red has to land
        // in the right corner, which is what catches a flipped or transposed
        // sample.
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, // top-left red
            0, 0, 255, 255, // top-right blue
            0, 0, 255, 255, // bottom-left blue
            0, 0, 255, 255, // bottom-right blue
        ];
        let image = ImageData::new(2, 2, &pixels);

        let mut p = ImagePainter::new(20, 20);
        p.draw_image(Rect::new(0.0, 0.0, 20.0, 20.0), &image);

        assert!(p.get_pixel(5, 5).r > 0.9, "top-left should be red");
        assert!(p.get_pixel(15, 5).b > 0.9, "top-right should be blue");
        assert!(p.get_pixel(5, 15).b > 0.9, "bottom-left should be blue");
    }

    #[test]
    fn draw_image_tolerates_a_short_pixel_slice() {
        // A truncated decode must degrade, not panic.
        let pixels = [255u8, 0, 0, 255];
        let image = ImageData::new(4, 4, &pixels);
        let mut p = ImagePainter::new(10, 10);
        p.draw_image(Rect::new(0.0, 0.0, 10.0, 10.0), &image);
        assert!(p.get_pixel(0, 0).r > 0.9);
    }

    #[test]
    fn clear_and_read() {
        let mut p = ImagePainter::new(10, 10);
        p.clear(Color::RED);
        let c = p.get_pixel(5, 5);
        assert!((c.r - 1.0).abs() < 0.01);
        assert!(c.g < 0.01);
    }

    #[test]
    fn fill_rect_sets_pixels() {
        let mut p = ImagePainter::new(20, 20);
        p.fill_rect(Rect::new(5.0, 5.0, 10.0, 10.0), Color::GREEN, 0.0);
        let inside = p.get_pixel(10, 10);
        assert!(inside.g > 0.9);
        let outside = p.get_pixel(0, 0);
        assert!(outside.a < 0.01);
    }

    #[test]
    fn fill_circle_sets_pixels() {
        let mut p = ImagePainter::new(50, 50);
        p.fill_circle(Position::new(25.0, 25.0), 10.0, Color::BLUE);
        let center = p.get_pixel(25, 25);
        assert!(center.b > 0.9);
    }
}
