
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use fontdue::{Font, FontSettings};
use log::info;

use crate::render::FontError;

pub struct Glyph {
    pub width: usize,
    pub height: usize,

    pub xmin: i32,

    pub ymin: i32,
    pub advance: f32,
    pub coverage: Vec<u8>,
}

pub struct FontSet {
    font: Font,
    path: PathBuf,
    cache: HashMap<(char, u32), Glyph>,

    digit_advance: HashMap<u32, f32>,
}

impl FontSet {

    pub fn load(configured: &str) -> Result<Self, FontError> {
        let configured = configured.trim();
        let candidates = font_candidates(configured);
        let mut tried: Vec<String> = Vec::new();

        for path in &candidates {
            tried.push(path.display().to_string());
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {

                    if !configured.is_empty() && path.as_path() == Path::new(configured) {
                        return Err(FontError::FontRead {
                            path: path.clone(),
                            source: e,
                        });
                    }
                    continue;
                }
            };
            match Font::from_bytes(bytes, FontSettings::default()) {
                Ok(font) => {
                    info!("loaded font {}", path.display());
                    return Ok(Self {
                        font,
                        path: path.clone(),
                        cache: HashMap::new(),
                        digit_advance: HashMap::new(),
                    });
                }
                Err(reason) => {
                    if !configured.is_empty() && path.as_path() == Path::new(configured) {
                        return Err(FontError::FontParse {
                            path: path.clone(),
                            reason: reason.to_string(),
                        });
                    }
                }
            }
        }

        Err(FontError::NoFont {
            tried: tried.join(", "),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn key(c: char, px: f32) -> (char, u32) {

        (c, (px * 4.0).round() as u32)
    }

    pub fn glyph(&mut self, c: char, px: f32) -> &Glyph {
        let key = Self::key(c, px);

        let font = &self.font;
        self.cache.entry(key).or_insert_with(|| {
            let (metrics, coverage) = font.rasterize(c, px);
            Glyph {
                width: metrics.width,
                height: metrics.height,
                xmin: metrics.xmin,
                ymin: metrics.ymin,
                advance: metrics.advance_width,
                coverage,
            }
        })
    }

    pub fn tabular_digit_advance(&mut self, px: f32) -> f32 {
        let key = (px * 4.0).round() as u32;
        if let Some(v) = self.digit_advance.get(&key) {
            return *v;
        }
        let mut widest = 0.0f32;
        for c in '0'..='9' {
            let m = self.font.metrics(c, px);
            widest = widest.max(m.advance_width);
        }
        self.digit_advance.insert(key, widest);
        widest
    }

    pub fn advance(&mut self, c: char, px: f32, tabular: bool) -> f32 {
        if tabular && c.is_ascii_digit() {
            return self.tabular_digit_advance(px);
        }
        self.font.metrics(c, px).advance_width
    }

    pub fn measure(&mut self, text: &str, px: f32, tabular: bool) -> f32 {
        text.chars().map(|c| self.advance(c, px, tabular)).sum()
    }

}

fn font_candidates(configured: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let configured = configured.trim();
    if !configured.is_empty() {
        out.push(PathBuf::from(configured));
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        out.push(dir.join("assets").join("panel-font.ttf"));
        out.push(dir.join("panel-font.ttf"));
    }

    #[cfg(windows)]
    {
        let fonts = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\Windows"))
            .join("Fonts");

        for name in [
            "seguisb.ttf",
            "segoeuib.ttf",
            "segoeui.ttf",
            "consolab.ttf",
            "consola.ttf",
            "tahomabd.ttf",
            "arialbd.ttf",
            "arial.ttf",
        ] {
            out.push(fonts.join(name));
        }
    }

    #[cfg(not(windows))]
    {
        for p in [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
            "/usr/share/fonts/liberation/LiberationSans-Bold.ttf",
            "/System/Library/Fonts/SFNSDisplay.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
        ] {
            out.push(PathBuf::from(p));
        }
    }

    out
}