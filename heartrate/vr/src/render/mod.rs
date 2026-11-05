pub mod canvas;
pub mod gpu;
pub mod png;
pub mod text;

use std::path::PathBuf;

use canvas::Canvas;
use text::FontSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn fade(self, scale: f32) -> Self {
        Self {
            a: (self.a as f32 * scale.clamp(0.0, 1.0)).round() as u8,
            ..self
        }
    }

    pub fn mix(self, other: Color, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Self {
            r: lerp(self.r, other.r),
            g: lerp(self.g, other.g),
            b: lerp(self.b, other.b),
            a: lerp(self.a, other.a),
        }
    }
}

#[derive(Debug)]
pub enum FontError {
    NoFont { tried: String },
    FontRead { path: PathBuf, source: std::io::Error },
    FontParse { path: PathBuf, reason: String },
}

impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FontError::NoFont { tried } => write!(f, "no usable font found for the SteamVR panel. Tried: {tried}"),
            FontError::FontRead { path, source } => {
                write!(f, "could not read font {}: {source}", path.display())
            }
            FontError::FontParse { path, reason } => {
                write!(f, "{} could not be parsed as a font: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for FontError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FontError::FontRead { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub struct PanelRenderer {
    canvas: Canvas,
    font:   FontSet,
}

impl PanelRenderer {
    pub fn new(width: u32, height: u32) -> Result<Self, FontError> {
        Ok(Self {
            canvas: Canvas::new(width, height),
            font:   FontSet::load("")?,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.canvas.width, self.canvas.height)
    }

    pub fn font_path(&self) -> String {
        self.font.path().display().to_string()
    }

    pub fn straight_alpha_pixels(&self) -> &[u8] {
        self.canvas.straight_pixels()
    }

    pub fn render(&mut self, panel: &crate::panel::Panel) -> (&[u8], u32, u32) {
        crate::panel::draw(&mut self.canvas, &mut self.font, panel);
        let (w, h) = (self.canvas.width, self.canvas.height);
        (self.canvas.upload_buffer(true), w, h)
    }
}

pub fn tab_icon(size: u32) -> Vec<u8> {
    let mut canvas = Canvas::new(size, size);
    canvas.clear();

    let s = size as f32;
    canvas.fill_rounded_rect(0.0, 0.0, s, s, s * 0.22, Color::rgb(0x16, 0x16, 0x2B));
    canvas.fill_heart(s * 0.5, s * 0.5, s * 0.56, crate::panel::HEART);

    canvas.straight_pixels().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fading_scales_only_the_alpha() {
        let c = Color::rgba(10, 20, 30, 200);
        let faded = c.fade(0.5);
        assert_eq!((faded.r, faded.g, faded.b), (10, 20, 30));
        assert_eq!(faded.a, 100);
        assert_eq!(c.fade(2.0).a, 200, "the scale is clamped");
        assert_eq!(c.fade(-1.0).a, 0);
    }

    #[test]
    fn mixing_interpolates_and_clamps() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(100, 200, 50);
        assert_eq!(a.mix(b, 0.0), a);
        assert_eq!(a.mix(b, 1.0), b);
        assert_eq!(a.mix(b, 0.5), Color::rgb(50, 100, 25));
        assert_eq!(a.mix(b, 5.0), b);
    }

    #[test]
    fn the_tab_icon_is_square_rgba_with_something_drawn_on_it() {
        let icon = tab_icon(64);
        assert_eq!(icon.len(), 64 * 64 * 4);
        assert!(icon.chunks_exact(4).any(|p| p[3] > 0), "the icon must not be fully transparent");
    }
}
