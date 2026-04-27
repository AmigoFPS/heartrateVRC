use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};
use egui_plot::{Line, Plot};

use crate::BleEvent;
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

const MAX_PTS: usize = 600;
const VIEW_SEC: f64 = 60.0;

enum Status {
    Scanning,
    Connected(String),
    Error(String),
}

pub struct HeartRateApp {
    rx: Receiver<BleEvent>,
    t0: Instant,
    status: Status,
    bpm: i32,
    hrv: Option<HrvMetrics>,
    bpm_hist: VecDeque<[f64; 2]>,
    rmssd_hist: VecDeque<[f64; 2]>,
    last_data_t: f64,
}

impl HeartRateApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rx: Receiver<BleEvent>) -> Self {
        apply_theme(&cc.egui_ctx);
        Self {
            rx,
            t0: Instant::now(),
            status: Status::Scanning,
            bpm: 0,
            hrv: None,
            bpm_hist: VecDeque::with_capacity(MAX_PTS),
            rmssd_hist: VecDeque::with_capacity(MAX_PTS),
            last_data_t: 0.0,
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
                    if let Some(ref m) = hrv {
                        self.rmssd_hist.push_back([t, m.rmssd as f64]);
                    }
                    self.hrv = hrv;
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
}

impl eframe::App for HeartRateApp {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        let t = self.now();

        let plot_t = if self.last_data_t > 0.0 { self.last_data_t } else { t };

        #[allow(deprecated)]
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);

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
                        ui.add_space(4.0);
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

                ui.vertical_centered(|ui| {
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
                });

                card(ui, |ui| {
                    let (rmssd_str, sdnn_str, pnn50_str) = match &self.hrv {
                        Some(m) => (format!("{:.1}", m.rmssd), format!("{:.1}", m.sdnn), format!("{:.1}%", m.pnn50)),
                        None => ("—".into(), "—".into(), "—".into()),
                    };
                    ui.columns(3, |c| {
                        metric(&mut c[0], "RMSSD", &rmssd_str, TEAL);
                        metric(&mut c[1], "SDNN", &sdnn_str, TEAL);
                        metric(&mut c[2], "pNN50", &pnn50_str, PURPLE);
                    });
                });

                ui.label(RichText::new("Heart Rate").color(TEXT_LO).size(10.0));
                let bpm_pts: Vec<[f64; 2]> = self.bpm_hist.iter().copied().collect();
                dark_plot(ui, "bpm", plot_t, 110.0, 40.0, 180.0, |plot_ui| {
                    plot_ui.line(Line::new("BPM", egui_plot::PlotPoints::from(bpm_pts)).color(HEART).width(1.8));
                });

                ui.label(RichText::new("HRV · RMSSD").color(TEXT_LO).size(10.0));
                let rmssd_pts: Vec<[f64; 2]> = self.rmssd_hist.iter().copied().collect();
                dark_plot(ui, "rmssd", plot_t, 110.0, 0.0, 120.0, |plot_ui| {
                    plot_ui.line(Line::new("RMSSD", egui_plot::PlotPoints::from(rmssd_pts)).color(TEAL).width(1.8));
                });
            });

        ctx.request_repaint_after(Duration::from_millis(60));
    }
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
