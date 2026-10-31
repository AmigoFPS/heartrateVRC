
use std::time::{Duration, Instant};

use heartrate_core::hrv::{HrvMetrics, SignalQuality};
use heartrate_core::runtime::{DeviceStatus, Snapshot};

use crate::render::canvas::{Align, Canvas, GraphStyle};
use crate::render::text::FontSet;
use crate::render::Color;

pub const DESIGN_WIDTH: u32 = 1024;
pub const DESIGN_HEIGHT: u32 = 576;

pub const WIDTH_METERS: f32 = 2.2;

const WINDOW_SECS: f64 = 60.0;
const SLOTS: usize = 120;

const BPM_RANGE: (f32, f32) = (40.0, 180.0);
const RMSSD_RANGE: (f32, f32) = (0.0, 120.0);

const HOLD_SECS: f32 = 1.0;

const STALE_AFTER: Duration = Duration::from_secs(10);

const NOTICE_LIFETIME: Duration = Duration::from_millis(4000);

pub const BG: Color = Color::rgb(0x0D, 0x0D, 0x17);
pub const CARD: Color = Color::rgb(0x16, 0x16, 0x2B);
pub const CARD_STROKE: Color = Color::rgb(0x26, 0x26, 0x41);
pub const PURPLE: Color = Color::rgb(0x8C, 0x6A, 0xDB);
pub const TEAL: Color = Color::rgb(0x4B, 0xA8, 0xA8);
pub const HEART: Color = Color::rgb(0xD6, 0x4D, 0x73);
pub const TEXT_HI: Color = Color::rgb(0xE0, 0xE0, 0xF0);
pub const TEXT_LO: Color = Color::rgb(0x8A, 0x8A, 0xAD);
pub const GREEN: Color = Color::rgb(0x4A, 0xBA, 0x7A);
pub const AMBER: Color = Color::rgb(0xD4, 0xA8, 0x43);
pub const RED: Color = Color::rgb(0xC3, 0x41, 0x41);
pub const WATER: Color = Color::rgb(0x4D, 0x91, 0xD6);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.w * 0.5
    }

    pub fn center_y(&self) -> f32 {
        self.y + self.h * 0.5
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub width: f32,
    pub height: f32,
    pub s: f32,
    pub header: Rect,
    pub hero: Rect,
    pub hrv: Rect,
    pub bpm_label: Rect,
    pub bpm_plot: Rect,
    pub rmssd_label: Rect,
    pub rmssd_plot: Rect,
    pub footer: Rect,
    pub reset: Rect,
    pub save: Rect,
}

impl Metrics {
    pub fn new(width: f32, height: f32) -> Self {
        let s = height / DESIGN_HEIGHT as f32;
        let m = 28.0 * s;
        let inner = width - m * 2.0;
        let right = m + inner;

        let header = Rect::new(m, m, inner, 54.0 * s);

        let band_y = 94.0 * s;
        let band_h = 196.0 * s;
        let hero_w = 330.0 * s;
        let hero = Rect::new(m, band_y, hero_w, band_h);
        let hrv_x = hero.right() + 16.0 * s;
        let hrv = Rect::new(hrv_x, band_y, right - hrv_x, band_h);

        let bpm_label = Rect::new(m, 304.0 * s, inner, 18.0 * s);
        let bpm_plot = Rect::new(m, 324.0 * s, inner, 64.0 * s);
        let rmssd_label = Rect::new(m, 400.0 * s, inner, 18.0 * s);
        let rmssd_plot = Rect::new(m, 420.0 * s, inner, 64.0 * s);

        let footer = Rect::new(m, 498.0 * s, inner, 50.0 * s);
        let button_h = 44.0 * s;
        let button_y = footer.center_y() - button_h * 0.5;
        let save_w = 130.0 * s;
        let reset_w = 200.0 * s;
        let save = Rect::new(right - save_w, button_y, save_w, button_h);
        let reset = Rect::new(save.x - 14.0 * s - reset_w, button_y, reset_w, button_h);

        Self {
            width,
            height,
            s,
            header,
            hero,
            hrv,
            bpm_label,
            bpm_plot,
            rmssd_label,
            rmssd_plot,
            footer,
            reset,
            save,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Reset,
    Save,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    Move { x: f32, y: f32 },
    Press { x: f32, y: f32 },
    Release,
    Leave,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Outcome {
    pub reset_hrv: bool,
    pub save_log: bool,
    pub haptic: bool,
}

impl Outcome {
    pub fn merge(&mut self, other: Outcome) {
        self.reset_hrv |= other.reset_hrv;
        self.save_log |= other.save_log;
        self.haptic |= other.haptic;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Live {
    pub status: String,
    pub status_color: Color,
    pub bpm: Option<i32>,
    pub rmssd: Option<f32>,
    pub sdnn: Option<f32>,
    pub pnn50: Option<f32>,
    pub quality: Option<(SignalQuality, f32)>,
    pub bpm_series: Vec<Option<f32>>,
    pub rmssd_series: Vec<Option<f32>>,
    pub osc: String,
    pub osc_color: Color,
    pub reset_cooldown: u32,
    pub reset_recharge: f32,
}

impl Default for Live {
    fn default() -> Self {
        Self {
            status: "starting…".to_owned(),
            status_color: TEXT_LO,
            bpm: None,
            rmssd: None,
            sdnn: None,
            pnn50: None,
            quality: None,
            bpm_series: Vec::new(),
            rmssd_series: Vec::new(),
            osc: "VRChat: looking…".to_owned(),
            osc_color: TEXT_LO,
            reset_cooldown: 0,
            reset_recharge: 1.0,
        }
    }
}

impl Live {
    pub fn from_snapshot(snap: &Snapshot, now: Instant, now_t: f64) -> Self {
        let stale = snap.last_data_t <= 0.0
            || now_t - snap.last_data_t > STALE_AFTER.as_secs_f64()
            || !snap.device.is_connected();

        let (status, status_color) = match &snap.device {
            DeviceStatus::Connected(name) if stale => (
                format!("{}: no data", short_device(name)),
                AMBER,
            ),
            DeviceStatus::Connected(name) => (short_device(name), GREEN),
            DeviceStatus::Scanning => ("searching…".to_owned(), AMBER),
            DeviceStatus::Error(message) => (short_error(message), RED),
        };

        let hrv = if stale { None } else { snap.hrv };
        let bpm = (!stale && snap.bpm > 0).then_some(snap.bpm);

        let (osc, osc_color) = match (&snap.osc_target, snap.osc_connected) {
            (Some(target), true) => (format!("VRChat {target}"), GREEN),
            (Some(target), false) => (format!("VRChat? {target}"), AMBER),
            (None, _) => ("VRChat: looking…".to_owned(), TEXT_LO),
        };

        Self {
            status,
            status_color,
            bpm,
            rmssd: hrv.map(|m| round1(m.rmssd)),
            sdnn: hrv.map(|m| round1(m.sdnn)),
            pnn50: hrv.map(|m| round1(m.pnn50)),
            quality: hrv.map(|m: HrvMetrics| (m.quality, m.artifact_pct.round())),
            bpm_series: series(&snap.bpm_history, now_t, BPM_RANGE),
            rmssd_series: series(&snap.rmssd_history, now_t, RMSSD_RANGE),
            osc,
            osc_color,
            reset_cooldown: snap.reset_cooldown_secs(now).ceil() as u32,
            reset_recharge: quantise(snap.reset_recharge(now)),
        }
    }
}

fn short_device(name: &str) -> String {
    const MAX: usize = 28;
    if name.chars().count() <= MAX {
        return name.to_owned();
    }
    name.chars().take(MAX - 1).collect::<String>() + "…"
}

fn short_error(message: &str) -> String {
    const MAX: usize = 36;
    let first = message.split(';').next().unwrap_or(message).trim();
    if first.chars().count() <= MAX {
        return first.to_owned();
    }
    first.chars().take(MAX - 1).collect::<String>() + "…"
}

fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

fn quantise(v: f32) -> f32 {
    (v.clamp(0.0, 1.0) * 255.0).round() / 255.0
}

const HOLD_SLOTS: usize = 4;

fn series(
    history: &std::collections::VecDeque<[f64; 2]>,
    now_t: f64,
    range: (f32, f32),
) -> Vec<Option<f32>> {
    if history.is_empty() {
        return Vec::new();
    }

    let span = (range.1 - range.0).max(f32::EPSILON);
    let slot_secs = WINDOW_SECS / SLOTS as f64;
    let start = now_t - WINDOW_SECS;

    let points: Vec<[f64; 2]> = history.iter().copied().collect();
    let normalise = |v: f64| quantise(((v as f32 - range.0) / span).clamp(0.0, 1.0));

    let mut slots: Vec<Option<f32>> = vec![None; SLOTS];
    let mut cursor = 0usize;
    let mut held: Option<f32> = None;
    let mut held_for = 0usize;
    let mut any = false;

    while cursor < points.len() && points[cursor][0] <= start {
        let t = points[cursor][0];
        if t >= start - HOLD_SLOTS as f64 * slot_secs {
            held = Some(normalise(points[cursor][1]));
            held_for = ((start - t) / slot_secs).floor().max(0.0) as usize;
        }
        cursor += 1;
    }
    if held_for > HOLD_SLOTS {
        held = None;
    }

    for (i, slot) in slots.iter_mut().enumerate() {
        let edge = start + (i + 1) as f64 * slot_secs;
        let mut fresh = false;
        while cursor < points.len() && points[cursor][0] <= edge {
            held = Some(normalise(points[cursor][1]));
            cursor += 1;
            fresh = true;
        }

        if fresh {
            held_for = 0;
        } else {
            held_for += 1;
            if held_for > HOLD_SLOTS {
                held = None;
            }
        }

        *slot = held;
        any |= held.is_some();
    }

    if any { slots } else { Vec::new() }
}

pub struct Panel {
    metrics: Metrics,
    live: Live,
    hover: Option<Target>,
    pressed: Option<Target>,
    hold_started: Option<Instant>,
    hold_progress: f32,
    notice: Option<(String, bool, Instant)>,
    notice_seq: u64,
    dirty: bool,
}

impl Panel {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            metrics: Metrics::new(width as f32, height as f32),
            live: Live::default(),
            hover: None,
            pressed: None,
            hold_started: None,
            hold_progress: 0.0,
            notice: None,
            notice_seq: 0,
            dirty: true,
        }
    }

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    pub fn is_animating(&self) -> bool {
        self.hold_started.is_some() || self.live.reset_cooldown > 0 || self.notice.is_some()
    }

    pub fn set_live(&mut self, live: Live) {
        if self.live != live {
            self.live = live;
            self.dirty = true;
        }
    }

    pub fn set_notice(&mut self, seq: u64, text: &str, is_error: bool) {
        if seq == 0 || seq == self.notice_seq {
            return;
        }
        self.notice_seq = seq;
        self.notice = Some((text.to_owned(), is_error, Instant::now()));
        self.dirty = true;
    }

    pub fn tick(&mut self, now: Instant) -> Outcome {
        let mut out = Outcome::default();

        if let Some((_, _, shown_at)) = self.notice
            && now.duration_since(shown_at) > NOTICE_LIFETIME
        {
            self.notice = None;
            self.dirty = true;
        }

        if let Some(started) = self.hold_started {
            let progress =
                (now.saturating_duration_since(started).as_secs_f32() / HOLD_SECS).clamp(0.0, 1.0);
            if quantise(progress) != quantise(self.hold_progress) {
                self.hold_progress = progress;
                self.dirty = true;
            }
            if progress >= 1.0 {
                self.hold_started = None;
                self.hold_progress = 0.0;
                self.pressed = None;
                out.reset_hrv = true;
                out.haptic = true;
                self.dirty = true;
            }
        }

        out
    }

    pub fn handle(&mut self, input: Input, now: Instant) -> Outcome {
        let mut out = Outcome::default();

        match input {
            Input::Move { x, y } => {
                let target = self.target_at(x, y);
                if target != self.hover {
                    self.hover = target;
                    self.dirty = true;
                }
                if self.pressed.is_some() && target != self.pressed {
                    self.cancel_press();
                }
            }
            Input::Press { x, y } => {
                let target = self.target_at(x, y);
                self.hover = target;
                self.pressed = target;
                self.dirty = true;

                match target {
                    Some(Target::Reset) => {
                        if self.live.reset_cooldown == 0 {
                            self.hold_started = Some(now);
                            self.hold_progress = 0.0;
                            out.haptic = true;
                        }
                    }
                    Some(Target::Save) => out.haptic = true,
                    None => {}
                }
            }
            Input::Release => {
                if self.pressed == Some(Target::Save) {
                    out.save_log = true;
                }
                self.cancel_press();
            }
            Input::Leave => {
                self.hover = None;
                self.cancel_press();
            }
        }

        out
    }

    fn cancel_press(&mut self) {
        if self.pressed.is_some() || self.hold_started.is_some() || self.hold_progress != 0.0 {
            self.pressed = None;
            self.hold_started = None;
            self.hold_progress = 0.0;
            self.dirty = true;
        }
    }

    fn target_at(&self, x: f32, y: f32) -> Option<Target> {
        if self.metrics.reset.contains(x, y) {
            Some(Target::Reset)
        } else if self.metrics.save.contains(x, y) {
            Some(Target::Save)
        } else {
            None
        }
    }
}

fn baseline(rect: Rect, px: f32) -> f32 {
    rect.center_y() + px * 0.34
}

pub fn draw(canvas: &mut Canvas, font: &mut FontSet, panel: &Panel) {
    canvas.clear();

    let m = panel.metrics;
    let s = m.s;
    let radius = 20.0 * s;

    canvas.fill_rounded_rect(0.0, 0.0, m.width, m.height, radius, Color { a: 0xF7, ..BG });
    canvas.stroke_rounded_rect(
        s,
        s,
        m.width - 2.0 * s,
        m.height - 2.0 * s,
        radius,
        2.0 * s,
        CARD_STROKE,
    );

    draw_header(canvas, font, panel);
    draw_hero(canvas, font, panel);
    draw_hrv(canvas, font, panel);
    draw_traces(canvas, font, panel);
    draw_footer(canvas, font, panel);
    draw_notice(canvas, font, panel);
}

fn draw_header(canvas: &mut Canvas, font: &mut FontSet, panel: &Panel) {
    let m = panel.metrics;
    let s = m.s;
    let px = 30.0 * s;
    let base = baseline(m.header, px);

    let title = "HEARTRATE";
    canvas.draw_text(font, title, m.header.x, base, px, TEXT_HI, Align::Left, false);
    let after = m.header.x + font.measure(title, px, false) + 10.0 * s;
    canvas.draw_text(font, "OSC", after, base, px, PURPLE, Align::Left, false);

    let text_px = 24.0 * s;
    let right = m.header.right();
    let width = font.measure(&panel.live.status, text_px, false);
    canvas.draw_text(
        font,
        &panel.live.status,
        right,
        baseline(m.header, text_px),
        text_px,
        panel.live.status_color,
        Align::Right,
        false,
    );
    canvas.fill_circle(
        right - width - 16.0 * s,
        m.header.center_y(),
        6.0 * s,
        panel.live.status_color,
    );
}

fn draw_hero(canvas: &mut Canvas, font: &mut FontSet, panel: &Panel) {
    let m = panel.metrics;
    let s = m.s;
    card(canvas, m.hero, s);

    let cx = m.hero.center_x();
    canvas.fill_heart(cx, m.hero.y + 42.0 * s, 44.0 * s, HEART);

    let (text, px, color) = match panel.live.bpm {
        Some(bpm) => (bpm.to_string(), 100.0 * s, TEXT_HI),
        None => ("—".to_owned(), 56.0 * s, TEXT_LO),
    };
    canvas.draw_text(
        font,
        &text,
        cx,
        m.hero.y + 150.0 * s,
        px,
        color,
        Align::Center,
        true,
    );
    canvas.draw_text(
        font,
        "BPM",
        cx,
        m.hero.y + 180.0 * s,
        20.0 * s,
        TEXT_LO,
        Align::Center,
        false,
    );
}

fn draw_hrv(canvas: &mut Canvas, font: &mut FontSet, panel: &Panel) {
    let m = panel.metrics;
    let s = m.s;
    let rect = m.hrv;
    card(canvas, rect, s);

    let pad = 22.0 * s;
    canvas.draw_text(
        font,
        "HRV",
        rect.x + pad,
        rect.y + 30.0 * s,
        20.0 * s,
        TEXT_LO,
        Align::Left,
        false,
    );

    let columns = [
        ("RMSSD", panel.live.rmssd, TEAL, false),
        ("SDNN", panel.live.sdnn, TEAL, false),
        ("pNN50", panel.live.pnn50, PURPLE, true),
    ];
    let column_w = (rect.w - pad * 2.0) / columns.len() as f32;

    for (i, (label, value, color, percent)) in columns.into_iter().enumerate() {
        let cx = rect.x + pad + column_w * (i as f32 + 0.5);
        canvas.draw_text(
            font,
            label,
            cx,
            rect.y + 76.0 * s,
            22.0 * s,
            TEXT_LO,
            Align::Center,
            false,
        );
        let text = match value {
            Some(v) if percent => format!("{v:.1}%"),
            Some(v) => format!("{v:.1}"),
            None => "—".to_owned(),
        };
        let shown = if value.is_some() { color } else { TEXT_LO };
        canvas.draw_text(
            font,
            &text,
            cx,
            rect.y + 124.0 * s,
            44.0 * s,
            shown,
            Align::Center,
            true,
        );
    }

    canvas.fill_rounded_rect(
        rect.x + pad,
        rect.y + 146.0 * s,
        rect.w - pad * 2.0,
        1.0 * s,
        0.0,
        CARD_STROKE,
    );

    let (bars, color, text) = match panel.live.quality {
        Some((quality, artifacts)) => {
            let (bars, color) = match quality {
                SignalQuality::Good => (3, GREEN),
                SignalQuality::Fair => (2, AMBER),
                SignalQuality::Poor => (1, RED),
            };
            let text = if artifacts < 0.5 {
                quality.label().to_owned()
            } else {
                format!("{} · {artifacts:.0}% rejected", quality.label())
            };
            (bars, color, text)
        }
        None => (0, TEXT_LO, "no clean beats yet".to_owned()),
    };

    let bar_x = rect.x + pad;
    let bar_bottom = rect.y + 178.0 * s;
    draw_signal_bars(canvas, bar_x, bar_bottom, s, bars, color);
    canvas.draw_text(
        font,
        &text,
        bar_x + 42.0 * s,
        bar_bottom,
        20.0 * s,
        color,
        Align::Left,
        false,
    );
}

fn draw_signal_bars(canvas: &mut Canvas, x: f32, bottom: f32, s: f32, filled: u32, color: Color) {
    let bar_w = 7.0 * s;
    let gap = 4.0 * s;
    for i in 0..3u32 {
        let h = (8.0 + i as f32 * 5.0) * s;
        let shown = if i < filled {
            color
        } else {
            CARD_STROKE.mix(TEXT_LO, 0.45)
        };
        canvas.fill_rounded_rect(
            x + (bar_w + gap) * i as f32,
            bottom - h,
            bar_w,
            h,
            1.5 * s,
            shown,
        );
    }
}

fn draw_traces(canvas: &mut Canvas, font: &mut FontSet, panel: &Panel) {
    let m = panel.metrics;
    draw_trace(
        canvas,
        font,
        m.s,
        m.bpm_label,
        m.bpm_plot,
        "HEART RATE · 60 s",
        &panel.live.bpm_series,
        BPM_RANGE,
        HEART,
    );
    draw_trace(
        canvas,
        font,
        m.s,
        m.rmssd_label,
        m.rmssd_plot,
        "HRV · RMSSD · 60 s",
        &panel.live.rmssd_series,
        RMSSD_RANGE,
        TEAL,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_trace(
    canvas: &mut Canvas,
    font: &mut FontSet,
    s: f32,
    label: Rect,
    plot: Rect,
    title: &str,
    samples: &[Option<f32>],
    range: (f32, f32),
    color: Color,
) {
    let px = 18.0 * s;
    canvas.draw_text(font, title, label.x, label.bottom(), px, TEXT_LO, Align::Left, false);
    canvas.draw_text(
        font,
        &format!("{:.0}–{:.0}", range.0, range.1),
        label.right(),
        label.bottom(),
        px,
        TEXT_LO,
        Align::Right,
        true,
    );

    canvas.fill_rounded_rect(plot.x, plot.y, plot.w, plot.h, 6.0 * s, CARD);
    canvas.stroke_rounded_rect(plot.x, plot.y, plot.w, plot.h, 6.0 * s, 1.0 * s, CARD_STROKE);

    if samples.is_empty() {
        canvas.draw_text(
            font,
            "waiting for data…",
            plot.center_x(),
            baseline(plot, px),
            px,
            TEXT_LO,
            Align::Center,
            false,
        );
        return;
    }

    let inset = 3.0 * s;
    canvas.draw_graph(
        plot.x + inset,
        plot.y + inset,
        plot.w - inset * 2.0,
        plot.h - inset * 2.0,
        samples,
        &GraphStyle {
            capacity: SLOTS,
            full_scale: 1.0,
            line: color,
            fill: color.fade(0.22),
        },
    );
}

fn draw_footer(canvas: &mut Canvas, font: &mut FontSet, panel: &Panel) {
    let m = panel.metrics;
    let s = m.s;

    canvas.draw_text(
        font,
        &panel.live.osc,
        m.footer.x,
        baseline(m.footer, 20.0 * s),
        20.0 * s,
        panel.live.osc_color,
        Align::Left,
        false,
    );

    let cooling = panel.live.reset_cooldown > 0;
    let reset_label = if cooling {
        format!("RESET HRV · {}s", panel.live.reset_cooldown)
    } else if panel.hold_started.is_some() {
        "KEEP HOLDING".to_owned()
    } else {
        "HOLD TO RESET HRV".to_owned()
    };

    draw_button(
        canvas,
        font,
        s,
        m.reset,
        &reset_label,
        ButtonState {
            hover: panel.hover == Some(Target::Reset),
            pressed: panel.pressed == Some(Target::Reset),
            enabled: !cooling,
            fill: if cooling {
                Some((panel.live.reset_recharge, PURPLE.fade(0.7)))
            } else if panel.hold_progress > 0.0 {
                Some((panel.hold_progress, WATER))
            } else {
                None
            },
        },
    );

    draw_button(
        canvas,
        font,
        s,
        m.save,
        "SAVE",
        ButtonState {
            hover: panel.hover == Some(Target::Save),
            pressed: panel.pressed == Some(Target::Save),
            enabled: true,
            fill: None,
        },
    );
}

struct ButtonState {
    hover: bool,
    pressed: bool,
    enabled: bool,
    fill: Option<(f32, Color)>,
}

fn draw_button(
    canvas: &mut Canvas,
    font: &mut FontSet,
    s: f32,
    rect: Rect,
    label: &str,
    state: ButtonState,
) {
    let radius = 8.0 * s;
    let base = match (state.enabled, state.pressed, state.hover) {
        (false, _, _) => Color::rgb(0x1A, 0x1A, 0x30),
        (true, true, _) => CARD_STROKE.mix(PURPLE, 0.45),
        (true, false, true) => Color::rgb(0x2A, 0x2A, 0x49),
        (true, false, false) => Color::rgb(0x1E, 0x1E, 0x38),
    };
    canvas.fill_rounded_rect(rect.x, rect.y, rect.w, rect.h, radius, base);

    if let Some((progress, color)) = state.fill {
        let w = rect.w * progress.clamp(0.0, 1.0);
        if w > 0.0 {
            canvas.fill_rounded_rect(rect.x, rect.y, w, rect.h, radius, color);
        }
    }

    let border = if state.hover && state.enabled {
        PURPLE
    } else {
        CARD_STROKE
    };
    canvas.stroke_rounded_rect(rect.x, rect.y, rect.w, rect.h, radius, 1.5 * s, border);

    let px = 20.0 * s;
    let color = if state.enabled { TEXT_HI } else { TEXT_LO };
    canvas.draw_text(
        font,
        label,
        rect.center_x(),
        baseline(rect, px),
        px,
        color,
        Align::Center,
        false,
    );
}

fn draw_notice(canvas: &mut Canvas, font: &mut FontSet, panel: &Panel) {
    let Some((text, is_error, shown_at)) = panel.notice.as_ref() else {
        return;
    };
    let m = panel.metrics;
    let s = m.s;

    let age = shown_at.elapsed().as_secs_f32();
    let alpha = (1.0 - (age / NOTICE_LIFETIME.as_secs_f32()).clamp(0.0, 1.0)).powf(0.5);
    let fg = if *is_error { RED } else { GREEN };

    let px = 20.0 * s;
    let width = font.measure(text, px, false) + 28.0 * s;
    let height = 40.0 * s;
    let rect = Rect::new(
        m.footer.x,
        m.footer.y - height - 8.0 * s,
        width.min(m.footer.w),
        height,
    );

    canvas.fill_rounded_rect(rect.x, rect.y, rect.w, rect.h, 8.0 * s, CARD.fade(alpha));
    canvas.stroke_rounded_rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        8.0 * s,
        1.0 * s,
        CARD_STROKE.fade(alpha),
    );
    canvas.draw_text(
        font,
        text,
        rect.x + 14.0 * s,
        baseline(rect, px),
        px,
        fg.fade(alpha),
        Align::Left,
        false,
    );
}

fn card(canvas: &mut Canvas, rect: Rect, s: f32) {
    let radius = 12.0 * s;
    canvas.fill_rounded_rect(rect.x, rect.y, rect.w, rect.h, radius, CARD);
    canvas.stroke_rounded_rect(rect.x, rect.y, rect.w, rect.h, radius, 1.0 * s, CARD_STROKE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    fn panel() -> Panel {
        Panel::new(DESIGN_WIDTH, DESIGN_HEIGHT)
    }

    fn snapshot_with_history(points: &[(f64, f64)], now_t: f64) -> Snapshot {
        let mut snap = Snapshot {
            device: DeviceStatus::Connected("Polar H10".into()),
            bpm: 72,
            last_data_t: now_t,
            ..Snapshot::default()
        };
        snap.bpm_history = points.iter().map(|&(t, v)| [t, v]).collect();
        snap
    }

    #[test]
    fn the_layout_stays_inside_the_texture() {
        let m = Metrics::new(DESIGN_WIDTH as f32, DESIGN_HEIGHT as f32);
        for rect in [
            m.header, m.hero, m.hrv, m.bpm_label, m.bpm_plot, m.rmssd_label, m.rmssd_plot,
            m.footer, m.reset, m.save,
        ] {
            assert!(rect.x >= 0.0 && rect.y >= 0.0, "{rect:?} starts off-texture");
            assert!(
                rect.right() <= DESIGN_WIDTH as f32 && rect.bottom() <= DESIGN_HEIGHT as f32,
                "{rect:?} runs past the texture"
            );
        }
        assert!(m.hero.right() < m.hrv.x, "the two cards overlap");
        assert!(m.reset.right() < m.save.x, "the two buttons overlap");
    }

    #[test]
    fn the_layout_scales_with_the_texture() {
        let small = Metrics::new(512.0, 288.0);
        let big = Metrics::new(2048.0, 1152.0);
        assert!((small.s - 0.5).abs() < 1e-6);
        assert!((big.s - 2.0).abs() < 1e-6);
        assert!((big.reset.h / small.reset.h - 4.0).abs() < 1e-3);
    }

    #[test]
    fn hit_testing_finds_the_buttons_and_nothing_else() {
        let p = panel();
        let m = *p.metrics();
        assert_eq!(
            p.target_at(m.reset.center_x(), m.reset.center_y()),
            Some(Target::Reset)
        );
        assert_eq!(
            p.target_at(m.save.center_x(), m.save.center_y()),
            Some(Target::Save)
        );
        assert_eq!(p.target_at(m.hero.center_x(), m.hero.center_y()), None);
    }

    #[test]
    fn save_fires_on_release_over_the_button() {
        let mut p = panel();
        let m = *p.metrics();
        let now = Instant::now();

        let press = p.handle(
            Input::Press {
                x: m.save.center_x(),
                y: m.save.center_y(),
            },
            now,
        );
        assert!(!press.save_log, "a press alone must not save");
        assert!(press.haptic);

        let release = p.handle(Input::Release, now);
        assert!(release.save_log);
    }

    #[test]
    fn dragging_off_the_button_cancels_it() {
        let mut p = panel();
        let m = *p.metrics();
        let now = Instant::now();

        p.handle(
            Input::Press {
                x: m.save.center_x(),
                y: m.save.center_y(),
            },
            now,
        );
        p.handle(
            Input::Move {
                x: m.hero.center_x(),
                y: m.hero.center_y(),
            },
            now,
        );
        assert!(!p.handle(Input::Release, now).save_log);
    }

    #[test]
    fn reset_needs_the_whole_hold() {
        let mut p = panel();
        let m = *p.metrics();
        let start = Instant::now();

        p.handle(
            Input::Press {
                x: m.reset.center_x(),
                y: m.reset.center_y(),
            },
            start,
        );
        let early = p.tick(start + Duration::from_millis(400));
        assert!(!early.reset_hrv, "a short hold must not reset");
        assert!(p.hold_progress > 0.0);

        let done = p.tick(start + Duration::from_millis(1100));
        assert!(done.reset_hrv);
        assert_eq!(p.hold_progress, 0.0, "the gesture resets after firing");

        assert!(!p.tick(start + Duration::from_millis(2000)).reset_hrv);
    }

    #[test]
    fn releasing_early_abandons_the_hold() {
        let mut p = panel();
        let m = *p.metrics();
        let start = Instant::now();

        p.handle(
            Input::Press {
                x: m.reset.center_x(),
                y: m.reset.center_y(),
            },
            start,
        );
        p.tick(start + Duration::from_millis(500));
        p.handle(Input::Release, start + Duration::from_millis(500));
        assert!(!p.tick(start + Duration::from_secs(5)).reset_hrv);
    }

    #[test]
    fn a_cooling_down_reset_ignores_the_press() {
        let mut p = panel();
        let m = *p.metrics();
        let now = Instant::now();
        p.set_live(Live {
            reset_cooldown: 7,
            ..Live::default()
        });

        p.handle(
            Input::Press {
                x: m.reset.center_x(),
                y: m.reset.center_y(),
            },
            now,
        );
        assert!(!p.tick(now + Duration::from_secs(3)).reset_hrv);
    }

    #[test]
    fn identical_data_does_not_mark_the_panel_dirty() {
        let mut p = panel();
        let live = Live {
            bpm: Some(72),
            ..Live::default()
        };
        p.set_live(live.clone());
        p.clear_dirty();

        p.set_live(live);
        assert!(!p.is_dirty(), "redrawing unchanged values wastes frame time");

        p.set_live(Live {
            bpm: Some(73),
            ..Live::default()
        });
        assert!(p.is_dirty());
    }

    #[test]
    fn an_idle_panel_is_not_animating() {
        let mut p = panel();
        assert!(!p.is_animating());

        p.set_live(Live {
            reset_cooldown: 4,
            ..Live::default()
        });
        assert!(p.is_animating(), "the recharge bar has to keep moving");
    }

    #[test]
    fn a_notice_is_shown_once_and_then_expires() {
        let mut p = panel();
        p.set_notice(1, "Log saved: a.json", false);
        assert!(p.notice.is_some());

        p.clear_dirty();
        p.set_notice(1, "Log saved: a.json", false);
        assert!(!p.is_dirty(), "the same notice must not redraw");

        p.notice = Some(("old".into(), false, Instant::now() - NOTICE_LIFETIME * 2));
        p.tick(Instant::now());
        assert!(p.notice.is_none());
    }

    #[test]
    fn stale_readings_are_not_presented_as_current() {
        let now = Instant::now();
        let snap = Snapshot {
            device: DeviceStatus::Connected("Polar H10".into()),
            bpm: 72,
            hrv: Some(HrvMetrics {
                rmssd: 40.0,
                sdnn: 50.0,
                pnn50: 20.0,
                mean_hr: 72.0,
                artifact_pct: 1.0,
                quality: SignalQuality::Good,
            }),
            last_data_t: 10.0,
            ..Snapshot::default()
        };

        let fresh = Live::from_snapshot(&snap, now, 12.0);
        assert_eq!(fresh.bpm, Some(72));
        assert!(fresh.rmssd.is_some());
        assert_eq!(fresh.status, "Polar H10");

        let stale = Live::from_snapshot(&snap, now, 10.0 + STALE_AFTER.as_secs_f64() + 1.0);
        assert_eq!(stale.bpm, None, "an old reading must not look live");
        assert_eq!(stale.rmssd, None);
        assert!(stale.status.contains("no data"));
    }

    #[test]
    fn a_disconnected_device_shows_no_reading() {
        let now = Instant::now();
        let snap = Snapshot {
            device: DeviceStatus::Scanning,
            bpm: 72,
            last_data_t: 9.0,
            ..Snapshot::default()
        };
        let live = Live::from_snapshot(&snap, now, 10.0);
        assert_eq!(live.bpm, None);
        assert_eq!(live.status, "searching…");
    }

    #[test]
    fn the_osc_line_distinguishes_discovered_from_fallback() {
        let now = Instant::now();
        let mut snap = Snapshot::default();
        assert!(Live::from_snapshot(&snap, now, 1.0).osc.contains("looking"));

        snap.osc_target = Some("127.0.0.1:9000".into());
        snap.osc_connected = true;
        assert_eq!(
            Live::from_snapshot(&snap, now, 1.0).osc,
            "VRChat 127.0.0.1:9000"
        );

        snap.osc_connected = false;
        assert!(Live::from_snapshot(&snap, now, 1.0).osc.starts_with("VRChat?"));
    }

    #[test]
    fn a_trace_normalises_into_its_range_and_fills_from_the_left() {
        let snap = snapshot_with_history(&[(59.0, 40.0), (59.6, 180.0)], 60.0);
        let s = series(&snap.bpm_history, 60.0, BPM_RANGE);

        assert_eq!(s.len(), SLOTS);
        assert_eq!(s[0], None, "nothing was recorded a minute ago");
        assert_eq!(s.last().unwrap().unwrap(), 1.0, "180 bpm tops the range");
        assert!(s.contains(&Some(0.0)), "40 bpm bottoms the range");
    }

    #[test]
    fn a_trace_clamps_values_outside_the_range() {
        let snap = snapshot_with_history(&[(59.5, 300.0)], 60.0);
        let s = series(&snap.bpm_history, 60.0, BPM_RANGE);
        assert!(s.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
        assert_eq!(s.last().unwrap().unwrap(), 1.0);
    }

    #[test]
    fn a_dropout_leaves_a_gap_rather_than_a_flat_line() {
        let empty: VecDeque<[f64; 2]> = VecDeque::new();
        assert!(series(&empty, 100.0, BPM_RANGE).is_empty());

        let snap = snapshot_with_history(&[(28.0, 60.0), (29.0, 62.0), (30.0, 61.0)], 60.0);
        let s = series(&snap.bpm_history, 60.0, BPM_RANGE);
        assert_eq!(s.len(), SLOTS);
        assert!(s.iter().any(Option::is_some), "the readings are there");
        assert_eq!(s.last().unwrap(), &None, "the silence is left blank");

        let old = snapshot_with_history(&[(1.0, 60.0)], 500.0);
        assert!(series(&old.bpm_history, 500.0, BPM_RANGE).is_empty());
    }

    #[test]
    fn ordinary_sampling_jitter_does_not_punch_holes_in_the_trace() {
        let points: Vec<(f64, f64)> = (0..60).map(|i| (i as f64, 70.0)).collect();
        let snap = snapshot_with_history(&points, 60.0);
        let s = series(&snap.bpm_history, 60.0, BPM_RANGE);

        let drawn = s.iter().filter(|v| v.is_some()).count();
        assert!(drawn > SLOTS - 6, "only {drawn} of {SLOTS} slots were drawn");
    }

    #[test]
    fn long_device_names_and_errors_are_trimmed_to_one_line() {
        let long = "A Very Long Heart Rate Monitor Name That Will Not Fit";
        assert!(short_device(long).chars().count() <= 28);
        assert!(short_device("Polar H10") == "Polar H10");

        let err = "Heart rate transmission timed out; returning to discovery";
        assert_eq!(short_error(err), "Heart rate transmission timed out");
    }

    #[test]
    fn drawing_fills_the_texture_without_panicking() {
        let Ok(mut renderer) =
            crate::render::PanelRenderer::new(DESIGN_WIDTH, DESIGN_HEIGHT)
        else {
            return;
        };

        let mut p = panel();
        p.set_live(Live {
            bpm: Some(72),
            rmssd: Some(42.1),
            sdnn: Some(58.3),
            pnn50: Some(18.4),
            quality: Some((SignalQuality::Fair, 6.0)),
            bpm_series: vec![Some(0.5); SLOTS],
            rmssd_series: vec![Some(0.3); SLOTS],
            ..Live::default()
        });

        let (pixels, w, h) = renderer.render(&p);
        assert_eq!(pixels.len(), (w * h * 4) as usize);
        assert!(
            pixels.chunks_exact(4).filter(|p| p[3] > 0).count() > (w * h / 2) as usize,
            "the panel body should be opaque"
        );
    }
}
