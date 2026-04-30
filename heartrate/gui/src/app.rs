use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Sense, Stroke, Vec2, ViewportCommand};
use egui_plot::{Line, Plot};

use crate::{BleEvent, GuiCommand};
use heartrate_core::hrv::HrvMetrics;

const BG: Color32 = Color32::from_rgb(13, 13, 23);
const CARD: Color32 = Color32::from_rgb(22, 22, 43);
const CARD_STROKE: Color32 = Color32::from_rgb(38, 38, 65);
const PURPLE: Color32 = Color32::from_rgb(140, 106, 219);
const TEAL: Color32 = Color32::from_rgb(75, 168, 168);
const HEART: Color32 = Color32::from_rgb(214, 77, 115);
const TEXT_HI: Color32 = Color32::from_rgb(224, 224, 240);
const TEXT_LO: Color32 = Color32::from_rgb(110, 110, 145);
const GREEN: Color32 = Color32::from_rgb(74, 186, 122);
const AMBER: Color32 = Color32::from_rgb(212, 168, 67);
const RED: Color32 = Color32::from_rgb(195, 65, 65);
const WATER: Color32 = Color32::from_rgb(77, 145, 214);
const RECHARGE: Color32 = Color32::from_rgb(140, 106, 219);
const ICON_BG: Color32 = Color32::from_rgb(28, 28, 49);
const ICON_BG_HOVER: Color32 = Color32::from_rgb(38, 38, 65);

const MAX_PTS: usize = 600;
const VIEW_SEC: f64 = 60.0;
const RESET_HOLD_SECS: f32 = 1.0;
const RESET_SETTLE_SECS: f32 = 1.0;
const RESET_BUTTON_COOLDOWN_SECS: f32 = 11.0;

const ICON_SIZE: f32 = 26.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LayoutMode {
    Full,
    Lite,
    Compact,
}

impl LayoutMode {
    fn size(self) -> Vec2 {
        match self {
            LayoutMode::Full => Vec2::new(320.0, 520.0),
            LayoutMode::Lite => Vec2::new(340.0, 210.0),
            LayoutMode::Compact => Vec2::new(200.0, 200.0),
        }
    }
}

enum Status {
    Scanning,
    Connected(String),
    Error(String),
}

pub struct HeartRateApp {
    rx: Receiver<BleEvent>,
    tx_cmd: Sender<GuiCommand>,
    t0: Instant,
    status: Status,
    bpm: i32,
    hrv: Option<HrvMetrics>,
    bpm_hist: VecDeque<[f64; 2]>,
    rmssd_hist: VecDeque<[f64; 2]>,
    last_data_t: f64,
    reset_fill_started: Option<Instant>,
    reset_suppress_until: Option<Instant>,
    reset_button_cooldown_until: Option<Instant>,
    mode: LayoutMode,
    pending_resize: bool,
}

impl HeartRateApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rx: Receiver<BleEvent>, tx_cmd: Sender<GuiCommand>) -> Self {
        apply_theme(&cc.egui_ctx);
        Self {
            rx,
            tx_cmd,
            t0: Instant::now(),
            status: Status::Scanning,
            bpm: 0,
            hrv: None,
            bpm_hist: VecDeque::with_capacity(MAX_PTS),
            rmssd_hist: VecDeque::with_capacity(MAX_PTS),
            last_data_t: 0.0,
            reset_fill_started: None,
            reset_suppress_until: None,
            reset_button_cooldown_until: None,
            mode: LayoutMode::Full,
            pending_resize: false,
        }
    }

    fn now(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    fn poll(&mut self) {
        let t = self.now();
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                BleEvent::Scanning => {
                    self.status = Status::Scanning;
                    self.bpm = 0;
                }
                BleEvent::Connected(name) => self.status = Status::Connected(name),
                BleEvent::Disconnected => {
                    self.status = Status::Scanning;
                    self.bpm = 0;
                }
                BleEvent::Data { bpm, hrv } => {
                    self.bpm = bpm;
                    self.last_data_t = t;
                    self.bpm_hist.push_back([t, bpm as f64]);
                    let suppress_hrv = self.reset_suppress_until.is_some_and(|until| Instant::now() < until);
                    let shown_hrv = if suppress_hrv { None } else { hrv };
                    if let Some(ref m) = shown_hrv {
                        self.rmssd_hist.push_back([t, m.rmssd as f64]);
                    }
                    self.hrv = shown_hrv;
                    while self.bpm_hist.len() > MAX_PTS {
                        self.bpm_hist.pop_front();
                    }
                    while self.rmssd_hist.len() > MAX_PTS {
                        self.rmssd_hist.pop_front();
                    }
                }
                BleEvent::FatalError(msg) => self.status = Status::Error(msg),
            }
        }
    }

    fn on_hrv_reset(&mut self) {
        self.hrv = None;
        self.rmssd_hist.clear();
        self.reset_suppress_until = Some(Instant::now() + Duration::from_secs_f32(RESET_SETTLE_SECS));
        self.reset_button_cooldown_until = Some(Instant::now() + Duration::from_secs_f32(RESET_BUTTON_COOLDOWN_SECS));
    }

    fn set_mode(&mut self, mode: LayoutMode) {
        if self.mode != mode {
            self.mode = mode;
            self.pending_resize = true;
        }
    }
}

impl eframe::App for HeartRateApp {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        let t = self.now();
        let plot_t = if self.last_data_t > 0.0 { self.last_data_t } else { t };

        if self.pending_resize {
            let size = self.mode.size();
            ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(size));
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
            self.pending_resize = false;
        }

        let mode = self.mode;

        #[allow(deprecated)]
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG).inner_margin(8.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);

                let next_mode = match mode {
                    LayoutMode::Full => self.render_full(ui, t, plot_t),
                    LayoutMode::Lite => self.render_lite(ui, t),
                    LayoutMode::Compact => self.render_compact(ui, t),
                };

                if let Some(target) = next_mode {
                    self.set_mode(target);
                }
            });

        ctx.request_repaint_after(Duration::from_millis(60));
    }
}

impl HeartRateApp {
    fn render_full(&mut self, ui: &mut egui::Ui, t: f64, plot_t: f64) -> Option<LayoutMode> {
        let mut next = self.render_top_bar(ui, None);

        self.render_bpm_block(ui, t);
        self.render_status_dot(ui);
        self.render_hrv_card_horizontal(ui);

        let mut request_lite = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Heart Rate").color(TEXT_LO).size(10.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if chevron_btn(ui, ChevronDir::Up).clicked() {
                    request_lite = true;
                }
            });
        });
        if request_lite {
            next = Some(LayoutMode::Lite);
        }

        let bpm_pts: Vec<[f64; 2]> = self.bpm_hist.iter().copied().collect();
        dark_plot(ui, "bpm", plot_t, 110.0, 40.0, 180.0, |plot_ui| {
            plot_ui.line(Line::new("BPM", egui_plot::PlotPoints::from(bpm_pts)).color(HEART).width(1.8));
        });

        ui.label(RichText::new("HRV · RMSSD").color(TEXT_LO).size(10.0));
        let rmssd_pts: Vec<[f64; 2]> = self.rmssd_hist.iter().copied().collect();
        dark_plot(ui, "rmssd", plot_t, 110.0, 0.0, 120.0, |plot_ui| {
            plot_ui.line(Line::new("RMSSD", egui_plot::PlotPoints::from(rmssd_pts)).color(TEAL).width(1.8));
        });

        next
    }

    fn render_lite(&mut self, ui: &mut egui::Ui, t: f64) -> Option<LayoutMode> {
        let mut next: Option<LayoutMode> = None;

        ui.horizontal(|ui| {
            self.render_reset_icon(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if chevron_btn(ui, ChevronDir::Left).clicked() {
                    next = Some(LayoutMode::Compact);
                }
            });
        });

        let middle_h = (ui.available_height() - 26.0).max(80.0);
        let bpm_block_h = 94.0;
        let card_h = 100.0;
        let pad_bpm = ((middle_h - bpm_block_h) / 2.0).max(0.0);
        let pad_card = ((middle_h - card_h) / 2.0).max(0.0);

        ui.horizontal(|ui| {
            let left_w = ui.available_width() * 0.42;
            ui.allocate_ui_with_layout(
                Vec2::new(left_w, middle_h),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.add_space(pad_bpm);
                    self.render_bpm_block(ui, t);
                },
            );

            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), middle_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.add_space(pad_card);
                    self.render_hrv_card_vertical_wide(ui);
                },
            );
        });

        ui.horizontal(|ui| {
            self.render_status_dot(ui);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if chevron_btn(ui, ChevronDir::Down).clicked() {
                    next = Some(LayoutMode::Full);
                }
            });
        });

        next
    }

    fn render_compact(&mut self, ui: &mut egui::Ui, t: f64) -> Option<LayoutMode> {
        let next = self.render_top_bar(ui, Some((ChevronDir::Right, LayoutMode::Lite)));
        self.render_bpm_block(ui, t);
        self.render_status_dot(ui);
        next
    }

    fn render_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        right_chevron: Option<(ChevronDir, LayoutMode)>,
    ) -> Option<LayoutMode> {
        let mut next: Option<LayoutMode> = None;
        ui.horizontal(|ui| {
            self.render_reset_icon(ui);
            if let Some((dir, target)) = right_chevron {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if chevron_btn(ui, dir).clicked() {
                        next = Some(target);
                    }
                });
            }
        });
        next
    }

    fn render_bpm_block(&self, ui: &mut egui::Ui, t: f64) {
        let beat_p = if self.bpm > 30 { 60.0 / self.bpm as f64 } else { 1.0 };
        let phase = (t % beat_p) / beat_p;
        let pulse = if phase < 0.12 {
            1.0 + 0.08 * (phase / 0.12 * std::f64::consts::PI).sin()
        } else {
            1.0
        };
        let heart_alpha = (160.0 + 95.0 * pulse as f32).min(255.0) as u8;

        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 84.0),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                ui.add_space(2.0);
                ui.label(
                    RichText::new("♥")
                        .color(Color32::from_rgba_unmultiplied(HEART.r(), HEART.g(), HEART.b(), heart_alpha))
                        .size(22.0),
                );
                let bpm_text = if self.bpm > 0 {
                    format!("{}", self.bpm)
                } else {
                    "—".into()
                };
                ui.label(RichText::new(bpm_text).color(TEXT_HI).size(46.0).strong());
                ui.label(RichText::new("BPM").color(TEXT_LO).size(11.0));
            },
        );
    }

    fn render_status_dot(&self, ui: &mut egui::Ui) {
        let (dot, txt) = match &self.status {
            Status::Scanning => (AMBER, "searching…".to_owned()),
            Status::Connected(n) => {
                let label = if n.is_empty() { "connected".into() } else { n.clone() };
                (GREEN, label)
            }
            Status::Error(m) => (RED, m.clone()),
        };
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 3.5, dot);
            ui.label(RichText::new(txt).color(TEXT_LO).size(11.0));
        });
    }

    fn render_hrv_card_horizontal(&self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            let (rmssd_str, sdnn_str, pnn50_str) = self.hrv_strings();
            ui.columns(3, |c| {
                metric(&mut c[0], "RMSSD", &rmssd_str, TEAL);
                metric(&mut c[1], "SDNN", &sdnn_str, TEAL);
                metric(&mut c[2], "pNN50", &pnn50_str, PURPLE);
            });
        });
    }

    fn render_hrv_card_vertical_wide(&self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            let (rmssd_str, sdnn_str, pnn50_str) = self.hrv_strings();
            metric_row_large(ui, "RMSSD", &rmssd_str, TEAL);
            ui.add_space(2.0);
            metric_row_large(ui, "SDNN", &sdnn_str, TEAL);
            ui.add_space(2.0);
            metric_row_large(ui, "pNN50", &pnn50_str, PURPLE);
        });
    }

    fn hrv_strings(&self) -> (String, String, String) {
        match &self.hrv {
            Some(m) => (
                format!("{:.1}", m.rmssd),
                format!("{:.1}", m.sdnn),
                format!("{:.1}%", m.pnn50),
            ),
            None => ("—".into(), "—".into(), "—".into()),
        }
    }

    fn render_reset_icon(&mut self, ui: &mut egui::Ui) {
        let size = Vec2::new(ICON_SIZE, ICON_SIZE);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());

        let cooldown_left = self
            .reset_button_cooldown_until
            .map(|until| (until - Instant::now()).as_secs_f32())
            .unwrap_or(0.0)
            .max(0.0);
        let is_in_cooldown = cooldown_left > 0.0;
        let recharge_progress = if is_in_cooldown {
            1.0 - (cooldown_left / RESET_BUTTON_COOLDOWN_SECS).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let is_holding = response.hovered() && ui.input(|i| i.pointer.primary_down());
        if is_in_cooldown {
            self.reset_fill_started = None;
        } else if is_holding {
            if self.reset_fill_started.is_none() {
                self.reset_fill_started = Some(Instant::now());
            }
        } else {
            self.reset_fill_started = None;
        }

        let fill = if let Some(started) = self.reset_fill_started {
            (started.elapsed().as_secs_f32() / RESET_HOLD_SECS).clamp(0.0, 1.0)
        } else {
            0.0
        };

        if fill >= 1.0 {
            let _ = self.tx_cmd.send(GuiCommand::ResetHrv);
            self.on_hrv_reset();
            self.reset_fill_started = None;
        }

        let painter = ui.painter();
        let radius = CornerRadius::same(6);
        let bg_color = if is_in_cooldown {
            Color32::from_rgb(34, 34, 54)
        } else if response.hovered() {
            ICON_BG_HOVER
        } else {
            ICON_BG
        };

        painter.rect_filled(rect, radius, bg_color);

        if fill > 0.0 {
            let clip = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * fill, rect.height()));
            painter.with_clip_rect(clip).rect_filled(rect, radius, WATER);
        }
        if is_in_cooldown {
            let clip = egui::Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width() * recharge_progress, rect.height()),
            );
            painter.with_clip_rect(clip).rect_filled(rect, radius, RECHARGE.linear_multiply(0.7));
        }
        painter.rect_stroke(rect, radius, Stroke::new(1.0, CARD_STROKE), egui::StrokeKind::Outside);

        let icon_color = if is_in_cooldown { TEXT_LO } else { TEXT_HI };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "↻",
            FontId::proportional(14.0),
            icon_color,
        );

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            let tooltip = if is_in_cooldown {
                format!("HRV reset · recharging {:.1}s", cooldown_left)
            } else {
                "Hold to reset HRV".to_owned()
            };
            response.on_hover_text(tooltip);
        }
    }
}

enum ChevronDir {
    Up,
    Down,
    Left,
    Right,
}

fn chevron_btn(ui: &mut egui::Ui, dir: ChevronDir) -> egui::Response {
    let size = Vec2::new(ICON_SIZE, ICON_SIZE);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let bg = if response.hovered() { ICON_BG_HOVER } else { ICON_BG };
    let radius = CornerRadius::same(6);
    painter.rect_filled(rect, radius, bg);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, CARD_STROKE), egui::StrokeKind::Outside);

    let c = rect.center();
    let s = 4.5_f32;
    let stroke = Stroke::new(1.6, TEXT_HI);
    let (tip, wing_a, wing_b) = match dir {
        ChevronDir::Up => (
            egui::pos2(c.x, c.y - s),
            egui::pos2(c.x - s, c.y + s * 0.5),
            egui::pos2(c.x + s, c.y + s * 0.5),
        ),
        ChevronDir::Down => (
            egui::pos2(c.x, c.y + s),
            egui::pos2(c.x - s, c.y - s * 0.5),
            egui::pos2(c.x + s, c.y - s * 0.5),
        ),
        ChevronDir::Left => (
            egui::pos2(c.x - s, c.y),
            egui::pos2(c.x + s * 0.5, c.y - s),
            egui::pos2(c.x + s * 0.5, c.y + s),
        ),
        ChevronDir::Right => (
            egui::pos2(c.x + s, c.y),
            egui::pos2(c.x - s * 0.5, c.y - s),
            egui::pos2(c.x - s * 0.5, c.y + s),
        ),
    };
    painter.line_segment([wing_a, tip], stroke);
    painter.line_segment([tip, wing_b], stroke);

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .fill(CARD)
        .corner_radius(CornerRadius::same(8))
        .stroke(Stroke::new(1.0, CARD_STROKE))
        .inner_margin(egui::Margin::symmetric(10, 7))
        .show(ui, content);
}

fn metric(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(label).color(TEXT_LO).size(10.0));
        ui.label(RichText::new(value).color(color).size(17.0).strong());
    });
}

fn metric_row_large(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(TEXT_LO).size(10.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).color(color).size(17.0).strong());
        });
    });
}

fn dark_plot(
    ui: &mut egui::Ui,
    id: &str,
    now: f64,
    height: f32,
    y_lo: f64,
    y_hi: f64,
    lines: impl FnOnce(&mut egui_plot::PlotUi),
) {
    Plot::new(id)
        .height(height)
        .show_axes([false, true])
        .show_grid(true)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .include_x(now - VIEW_SEC)
        .include_x(now)
        .include_y(y_lo)
        .include_y(y_hi)
        .show(ui, lines);
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = BG;
    style.visuals.override_text_color = Some(TEXT_HI);
    style.visuals.widgets.noninteractive.bg_fill = CARD;
    style.visuals.extreme_bg_color = Color32::from_rgb(16, 16, 28);
    style.visuals.faint_bg_color = Color32::from_rgb(20, 20, 35);
    style.visuals.window_corner_radius = CornerRadius::same(10);
    style.visuals.menu_corner_radius = CornerRadius::same(6);
    ctx.set_global_style(style);
}
