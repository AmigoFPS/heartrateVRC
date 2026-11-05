use crate::render::{Color, text::FontSet};

pub struct GraphStyle {
    pub capacity: usize,

    pub full_scale: f32,

    pub line: Color,

    pub fill: Color,
}

fn sample_at(samples: &[Option<f32>], pos: f32) -> Option<f32> {
    match samples.len() {
        0 => None,
        1 => samples[0],
        n => {
            let pos = pos.clamp(0.0, (n - 1) as f32);
            let i0 = (pos.floor() as usize).min(n - 2);
            let frac = pos - i0 as f32;
            match (samples[i0], samples[i0 + 1]) {
                (Some(a), Some(b)) => Some(a * (1.0 - frac) + b * frac),
                (Some(a), None) if frac <= 0.5 => Some(a),
                (None, Some(b)) if frac >= 0.5 => Some(b),
                _ => None,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,

    Center,
}

pub struct Canvas {
    pub width: u32,
    pub height: u32,

    pixels: Vec<u8>,

    upload: Vec<u8>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        let n = (width as usize) * (height as usize) * 4;
        Self {
            width,
            height,
            pixels: vec![0; n],
            upload: vec![0; n],
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }
        *self = Canvas::new(width, height);
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    #[inline]
    fn index(&self, x: u32, y: u32) -> usize {
        ((y as usize) * (self.width as usize) + x as usize) * 4
    }

    #[inline]
    fn blend(&mut self, x: i32, y: i32, color: Color, coverage: f32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let a = (color.a as f32 / 255.0) * coverage.clamp(0.0, 1.0);
        if a <= 0.0 {
            return;
        }
        let i = self.index(x as u32, y as u32);
        let dst_a = self.pixels[i + 3] as f32 / 255.0;
        let out_a = a + dst_a * (1.0 - a);
        if out_a <= 0.0 {
            self.pixels[i..i + 4].fill(0);
            return;
        }
        for c in 0..3 {
            let src = [color.r, color.g, color.b][c] as f32 / 255.0;
            let dst = self.pixels[i + c] as f32 / 255.0;
            let out = (src * a + dst * dst_a * (1.0 - a)) / out_a;
            self.pixels[i + c] = (out.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        self.pixels[i + 3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    #[inline]
    fn rounded_rect_sdf(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
        let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
        let cx = x + w * 0.5;
        let cy = y + h * 0.5;
        let qx = (px - cx).abs() - (w * 0.5 - r);
        let qy = (py - cy).abs() - (h * 0.5 - r);
        let ox = qx.max(0.0);
        let oy = qy.max(0.0);
        (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
    }

    pub fn fill_rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: Color) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let x0 = (x - 1.0).floor().max(0.0) as i32;
        let y0 = (y - 1.0).floor().max(0.0) as i32;
        let x1 = (x + w + 1.0).ceil().min(self.width as f32) as i32;
        let y1 = (y + h + 1.0).ceil().min(self.height as f32) as i32;

        for py in y0..y1 {
            for px in x0..x1 {
                let d = Self::rounded_rect_sdf(px as f32 + 0.5, py as f32 + 0.5, x, y, w, h, radius);

                let coverage = (0.5 - d).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    self.blend(px, py, color, coverage);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn stroke_rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, thickness: f32, color: Color) {
        if w <= 0.0 || h <= 0.0 || thickness <= 0.0 {
            return;
        }
        let x0 = (x - thickness - 1.0).floor().max(0.0) as i32;
        let y0 = (y - thickness - 1.0).floor().max(0.0) as i32;
        let x1 = (x + w + thickness + 1.0).ceil().min(self.width as f32) as i32;
        let y1 = (y + h + thickness + 1.0).ceil().min(self.height as f32) as i32;

        for py in y0..y1 {
            for px in x0..x1 {
                let d = Self::rounded_rect_sdf(px as f32 + 0.5, py as f32 + 0.5, x, y, w, h, radius);

                let ring = d.abs() - thickness * 0.5;
                let coverage = (0.5 - ring).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    self.blend(px, py, color, coverage);
                }
            }
        }
    }

    pub fn fill_circle(&mut self, cx: f32, cy: f32, radius: f32, color: Color) {
        if radius <= 0.0 {
            return;
        }
        let x0 = (cx - radius - 1.0).floor().max(0.0) as i32;
        let y0 = (cy - radius - 1.0).floor().max(0.0) as i32;
        let x1 = (cx + radius + 1.0).ceil().min(self.width as f32) as i32;
        let y1 = (cy + radius + 1.0).ceil().min(self.height as f32) as i32;

        for py in y0..y1 {
            for px in x0..x1 {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                let coverage = (0.5 - ((dx * dx + dy * dy).sqrt() - radius)).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    self.blend(px, py, color, coverage);
                }
            }
        }
    }

    pub fn fill_heart(&mut self, cx: f32, cy: f32, size: f32, color: Color) {
        const X_EXTENT: f32 = 1.138;
        const Y_EXTENT: f32 = 1.118;
        const Y_CENTRE: f32 = 0.118;
        const GRID: i32 = 3;

        if size <= 0.0 {
            return;
        }

        let half = size * 0.5;
        let x0 = (cx - half).floor().max(0.0) as i32;
        let y0 = (cy - half).floor().max(0.0) as i32;
        let x1 = (cx + half).ceil().min(self.width as f32) as i32;
        let y1 = (cy + half).ceil().min(self.height as f32) as i32;

        let inside = |x: f32, y: f32| {
            let u = (x - cx) / half * X_EXTENT;
            let v = (cy - y) / half * Y_EXTENT + Y_CENTRE;
            let t = u * u + v * v - 1.0;
            t * t * t - u * u * v * v * v <= 0.0
        };

        for py in y0..y1 {
            for px in x0..x1 {
                let mut hits = 0;
                for sy in 0..GRID {
                    for sx in 0..GRID {
                        let x = px as f32 + (sx as f32 + 0.5) / GRID as f32;
                        let y = py as f32 + (sy as f32 + 0.5) / GRID as f32;
                        if inside(x, y) {
                            hits += 1;
                        }
                    }
                }
                if hits > 0 {
                    self.blend(px, py, color, hits as f32 / (GRID * GRID) as f32);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &mut self,
        font: &mut FontSet,
        text: &str,
        x: f32,
        baseline_y: f32,
        px: f32,
        color: Color,
        align: Align,
        tabular: bool,
    ) -> f32 {
        let width = font.measure(text, px, tabular);
        let mut pen = match align {
            Align::Left => x,
            Align::Right => x - width,
            Align::Center => x - width * 0.5,
        };

        for ch in text.chars() {
            let advance = font.advance(ch, px, tabular);
            if !ch.is_whitespace() {
                let glyph = font.glyph(ch, px);
                let (gw, gh) = (glyph.width, glyph.height);
                let centering = if tabular && ch.is_ascii_digit() {
                    (advance - glyph.advance) * 0.5
                } else {
                    0.0
                };
                let ox = (pen + centering).round() as i32 + glyph.xmin;
                let oy = baseline_y.round() as i32 - gh as i32 - glyph.ymin;

                for gy in 0..gh {
                    for gx in 0..gw {
                        let a = glyph.coverage[gy * gw + gx];
                        if a != 0 {
                            self.blend(ox + gx as i32, oy + gy as i32, color, a as f32 / 255.0);
                        }
                    }
                }
            }
            pen += advance;
        }
        width
    }

    pub fn draw_graph(&mut self, x: f32, y: f32, w: f32, h: f32, samples: &[Option<f32>], style: &GraphStyle) {
        if w <= 0.0 || h <= 0.0 || samples.is_empty() {
            return;
        }
        let capacity = style.capacity.max(samples.len()).max(2);
        let offset = (capacity - samples.len()) as f32;
        let bottom = y + h;

        let x0 = x.floor().max(0.0) as i32;
        let x1 = ((x + w).ceil() as i32).min(self.width as i32);

        for px in x0..x1 {
            let cx = px as f32 + 0.5;
            if cx < x || cx > x + w {
                continue;
            }
            let axis = ((cx - x) / w) * (capacity - 1) as f32;
            let pos = axis - offset;
            if pos < 0.0 {
                continue;
            }
            let Some(value) = sample_at(samples, pos) else {
                continue;
            };
            let value = value / style.full_scale.max(f32::EPSILON);
            let top = bottom - h * value.clamp(0.0, 1.0);

            let fill_from = top.floor().max(y.floor()) as i32;
            let fill_to = bottom.ceil() as i32;
            for py in fill_from..fill_to {
                let pixel_top = py as f32;
                let covered = ((pixel_top + 1.0).min(bottom) - pixel_top.max(top)).clamp(0.0, 1.0);
                if covered > 0.0 {
                    self.blend(px, py, style.fill, covered);
                }
            }

            let half = 0.8;
            let line_from = (top - half).floor() as i32;
            let line_to = (top + half).ceil() as i32;
            for py in line_from..line_to {
                let pixel_top = py as f32;
                let covered = ((pixel_top + 1.0).min(top + half) - pixel_top.max(top - half)).clamp(0.0, 1.0);
                if covered > 0.0 {
                    self.blend(px, py, style.line, covered);
                }
            }
        }
    }

    pub fn upload_buffer(&mut self, premultiply: bool) -> &[u8] {
        if !premultiply {
            return &self.pixels;
        }
        self.upload.resize(self.pixels.len(), 0);
        for (dst, src) in self.upload.chunks_exact_mut(4).zip(self.pixels.chunks_exact(4)) {
            let a = src[3] as u32;
            dst[0] = ((src[0] as u32 * a + 127) / 255) as u8;
            dst[1] = ((src[1] as u32 * a + 127) / 255) as u8;
            dst[2] = ((src[2] as u32 * a + 127) / 255) as u8;
            dst[3] = src[3];
        }
        &self.upload
    }

    pub fn straight_pixels(&self) -> &[u8] {
        &self.pixels
    }

    #[cfg(test)]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = self.index(x, y);
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_makes_everything_transparent() {
        let mut c = Canvas::new(8, 8);
        c.fill_rounded_rect(0.0, 0.0, 8.0, 8.0, 0.0, Color::rgba(255, 0, 0, 255));
        c.clear();
        assert_eq!(c.pixel(4, 4), [0, 0, 0, 0]);
    }

    #[test]
    fn opaque_fill_covers_the_interior() {
        let mut c = Canvas::new(16, 16);
        c.fill_rounded_rect(2.0, 2.0, 12.0, 12.0, 0.0, Color::rgba(10, 20, 30, 255));
        assert_eq!(c.pixel(8, 8), [10, 20, 30, 255]);
        assert_eq!(c.pixel(0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn rounded_corners_are_cut_away() {
        let mut c = Canvas::new(32, 32);
        c.fill_rounded_rect(0.0, 0.0, 32.0, 32.0, 12.0, Color::rgba(255, 255, 255, 255));
        assert_eq!(c.pixel(16, 16)[3], 255);
        assert_eq!(c.pixel(0, 0)[3], 0);
    }

    #[test]
    fn a_circle_fills_its_centre_and_leaves_the_corners_alone() {
        let mut c = Canvas::new(32, 32);
        c.fill_circle(16.0, 16.0, 10.0, Color::rgba(255, 0, 0, 255));
        assert_eq!(c.pixel(16, 16), [255, 0, 0, 255]);
        assert_eq!(c.pixel(0, 0)[3], 0);
    }

    #[test]
    fn a_heart_has_lobes_a_dimple_and_a_point() {
        let mut c = Canvas::new(64, 64);
        c.fill_heart(32.0, 32.0, 64.0, Color::rgba(255, 0, 0, 255));

        assert_eq!(c.pixel(18, 12)[3], 255, "left lobe");
        assert_eq!(c.pixel(45, 12)[3], 255, "right lobe");
        assert_eq!(c.pixel(32, 1)[3], 0, "the dimple between the lobes");

        assert_eq!(c.pixel(32, 61)[3], 255, "the point");
        assert_eq!(c.pixel(10, 61)[3], 0, "nothing beside the point");

        assert_eq!(c.pixel(32, 32)[3], 255, "solid through the middle");
    }

    #[test]
    fn a_heart_is_symmetric_about_its_centre_line() {
        let mut c = Canvas::new(64, 64);
        c.fill_heart(32.0, 32.0, 64.0, Color::rgba(255, 0, 0, 255));

        for y in 0..64 {
            for x in 0..32 {
                let mirrored = 63 - x;
                assert_eq!(
                    c.pixel(x, y),
                    c.pixel(mirrored, y),
                    "column {x} and {mirrored} differ on row {y}"
                );
            }
        }
    }

    #[test]
    fn a_heart_stays_inside_its_box() {
        let mut c = Canvas::new(40, 40);
        c.fill_heart(20.0, 20.0, 20.0, Color::rgba(255, 0, 0, 255));
        for y in 0..40 {
            for x in 0..40 {
                if x < 10 || x >= 30 || y < 10 || y >= 30 {
                    assert_eq!(c.pixel(x, y)[3], 0, "painted outside the box at {x},{y}");
                }
            }
        }
    }

    #[test]
    fn a_trace_leaves_missing_slots_blank() {
        let mut c = Canvas::new(64, 32);
        let style = GraphStyle {
            capacity: 4,
            full_scale: 1.0,
            line: Color::rgba(255, 0, 0, 255),
            fill: Color::rgba(255, 0, 0, 255),
        };
        c.draw_graph(0.0, 0.0, 64.0, 32.0, &[Some(1.0), None, None, Some(1.0)], &style);
        assert_eq!(c.pixel(0, 31)[3], 255, "the first reading is drawn");
        assert_eq!(c.pixel(63, 31)[3], 255, "the last reading is drawn");
        assert_eq!(c.pixel(32, 31)[3], 0, "the gap is not bridged");
    }

    #[test]
    fn a_short_trace_is_drawn_at_the_recent_end() {
        let mut c = Canvas::new(64, 32);
        let style = GraphStyle {
            capacity: 4,
            full_scale: 1.0,
            line: Color::rgba(0, 255, 0, 255),
            fill: Color::rgba(0, 255, 0, 255),
        };
        c.draw_graph(0.0, 0.0, 64.0, 32.0, &[Some(1.0), Some(1.0)], &style);
        assert_eq!(c.pixel(2, 31)[3], 0, "no history yet on the left");
        assert_eq!(c.pixel(63, 31)[3], 255);
    }

    #[test]
    fn premultiplication_scales_colour_by_alpha() {
        let mut c = Canvas::new(2, 2);
        c.fill_rounded_rect(0.0, 0.0, 2.0, 2.0, 0.0, Color::rgba(255, 255, 255, 128));
        let buf = c.upload_buffer(true).to_vec();
        assert_eq!(buf[3], 128);
        assert!((buf[0] as i32 - 128).abs() <= 1);
    }

    #[test]
    fn straight_alpha_upload_is_untouched() {
        let mut c = Canvas::new(2, 2);
        c.fill_rounded_rect(0.0, 0.0, 2.0, 2.0, 0.0, Color::rgba(255, 255, 255, 128));
        let buf = c.upload_buffer(false).to_vec();
        assert_eq!(&buf[0..4], &[255, 255, 255, 128]);
    }
}
