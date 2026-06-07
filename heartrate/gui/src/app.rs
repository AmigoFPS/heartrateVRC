use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, CornerRadius, FontId, RichText, Sense, Stroke, Vec2, ViewportCommand};
use egui_plot::{Line, Plot};

use crate::{BleEvent, GuiCommand};
use heartrate_core::hrv::HrvMetrics;
use heartrate_core::logger::{self, FullStats, SessionLog, SessionLogger};

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
    Logs,
}

impl LayoutMode {
    fn size(self) -> Vec2 {
        match self {
            LayoutMode::Full => Vec2::new(320.0, 520.0),
            LayoutMode::Lite => Vec2::new(340.0, 210.0),
            LayoutMode::Compact => Vec2::new(200.0, 200.0),
            LayoutMode::Logs => Vec2::new(720.0, 560.0),
        }
    }
}

#[derive(Default)]
struct LogsView {
    files: Vec<PathBuf>,
    selected: SelectedLog,
    loaded: Option<SessionLog>,
    load_error: Option<String>,
    scanned: bool,
    export_status: Option<(Instant, String, bool)>,
}

#[derive(Clone, PartialEq, Eq, Default)]
enum SelectedLog {
    #[default]
    None,
    Live,
    File(PathBuf),
}

struct Toast {
    text: String,
    is_error: bool,
    shown_at: Instant,
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
    previous_mode: LayoutMode,
    pending_resize: bool,
    logs: LogsView,
    toast: Option<Toast>,
    live_logger: Option<SessionLogger>,
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
            previous_mode: LayoutMode::Full,
            pending_resize: false,
            logs: LogsView::default(),
            toast: None,
            live_logger: None,
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
                BleEvent::Connected(name) => {
                    self.status = Status::Connected(name.clone());
                    self.live_logger = Some(SessionLogger::new(name));
                }
                BleEvent::Disconnected => {
                    self.status = Status::Scanning;
                    self.bpm = 0;
                    self.live_logger = None;
                    if self.logs.selected == SelectedLog::Live {
                        self.logs.selected = SelectedLog::None;
                    }
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
                    if let Some(ref mut logger) = self.live_logger {
                        logger.record(bpm.max(0) as u16, shown_hrv.as_ref());
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
                BleEvent::LogSaved(path) => {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("session")
                        .to_owned();
                    self.show_toast(format!("Log saved: {}", name), false);
                    self.logs.scanned = false;
                }
                BleEvent::LogError(msg) => {
                    self.show_toast(format!("Log error: {}", msg), true);
                }
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
            self.previous_mode = self.mode;
            self.mode = mode;
            self.pending_resize = true;
        }
    }

    fn show_toast(&mut self, text: String, is_error: bool) {
        self.toast = Some(Toast {
            text,
            is_error,
            shown_at: Instant::now(),
        });
    }

    fn rescan_logs(&mut self) {
        let dir = logger::default_log_dir();
        match SessionLog::list_in_dir(&dir) {
            Ok(files) => {
                self.logs.files = files;
                self.logs.selected = match std::mem::take(&mut self.logs.selected) {
                    SelectedLog::Live if self.live_logger.is_some() => SelectedLog::Live,
                    SelectedLog::File(p) if self.logs.files.iter().any(|f| f == &p) => SelectedLog::File(p),
                    _ => {
                        if self.live_logger.is_some() {
                            SelectedLog::Live
                        } else if let Some(first) = self.logs.files.first() {
                            SelectedLog::File(first.clone())
                        } else {
                            SelectedLog::None
                        }
                    }
                };
                self.logs.scanned = true;
                self.load_selected_log();
            }
            Err(e) => {
                self.logs.files.clear();
                self.logs.selected = SelectedLog::None;
                self.logs.loaded = None;
                self.logs.load_error = Some(format!("{e}"));
                self.logs.scanned = true;
            }
        }
    }

    fn load_selected_log(&mut self) {
        self.logs.loaded = None;
        self.logs.load_error = None;
        let SelectedLog::File(ref path) = self.logs.selected else {
            return;
        };
        match SessionLog::load_from_file(path) {
            Ok(log) => self.logs.loaded = Some(log),
            Err(e) => self.logs.load_error = Some(format!("{e}")),
        }
    }

    fn current_log_for_view(&self) -> Option<SessionLog> {
        match &self.logs.selected {
            SelectedLog::Live => self.live_logger.as_ref().map(|l| l.snapshot()),
            SelectedLog::File(_) => self.logs.loaded.clone(),
            SelectedLog::None => None,
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
                    LayoutMode::Logs => self.render_logs(ui),
                };

                if let Some(target) = next_mode {
                    self.set_mode(target);
                }

                self.render_toast(ui);
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
            if icon_btn(ui, IconKind::Logs, "Session logs").clicked() {
                next = Some(LayoutMode::Logs);
            }
            if icon_btn(ui, IconKind::Save, "Save current session").clicked() {
                let _ = self.tx_cmd.send(GuiCommand::SaveLog);
            }
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
            if icon_btn(ui, IconKind::Logs, "Session logs").clicked() {
                next = Some(LayoutMode::Logs);
            }
            if icon_btn(ui, IconKind::Save, "Save current session").clicked() {
                let _ = self.tx_cmd.send(GuiCommand::SaveLog);
            }
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

    fn render_logs(&mut self, ui: &mut egui::Ui) -> Option<LayoutMode> {
        if !self.logs.scanned {
            self.rescan_logs();
        }

        let mut next: Option<LayoutMode> = None;
        let mut want_close = false;
        let mut want_rescan = false;
        let mut want_open_dir = false;
        let mut want_delete = false;
        let mut want_export = false;
        let mut want_save_live = false;

        ui.horizontal(|ui| {
            ui.label(RichText::new("Session Logs").color(TEXT_HI).size(14.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_btn(ui, IconKind::Close, "Close").clicked() {
                    want_close = true;
                }
                if icon_btn(ui, IconKind::Refresh, "Refresh list").clicked() {
                    want_rescan = true;
                }
                if icon_btn(ui, IconKind::Folder, "Open log folder").clicked() {
                    want_open_dir = true;
                }
            });
        });

        if want_close {
            next = Some(if self.previous_mode == LayoutMode::Logs {
                LayoutMode::Full
            } else {
                self.previous_mode
            });
        }
        if want_rescan {
            self.rescan_logs();
        }
        if want_open_dir {
            let dir = logger::default_log_dir();
            let _ = std::fs::create_dir_all(&dir);
            open_path(&dir);
        }

        ui.separator();

        ui.horizontal_top(|ui| {
            let list_w = (ui.available_width() * 0.32).clamp(180.0, 260.0);
            ui.allocate_ui_with_layout(
                Vec2::new(list_w, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.render_logs_list(ui);
                },
            );

            ui.separator();

            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let (delete_clicked, export_clicked, save_live_clicked) = self.render_log_details(ui);
                    want_delete = delete_clicked;
                    want_export = export_clicked;
                    want_save_live = save_live_clicked;
                },
            );
        });

        if want_delete {
            self.delete_selected_log();
        }
        if want_export {
            self.export_selected_csv();
        }
        if want_save_live {
            let _ = self.tx_cmd.send(GuiCommand::SaveLog);
        }

        next
    }

    fn render_logs_list(&mut self, ui: &mut egui::Ui) {
        let live_active = self.live_logger.is_some();
        let total = self.logs.files.len() + if live_active { 1 } else { 0 };
        ui.label(
            RichText::new(format!("{} session(s)", total))
                .color(TEXT_LO)
                .size(10.0),
        );
        ui.add_space(4.0);

        if let Some(err) = &self.logs.load_error {
            ui.label(RichText::new(err.clone()).color(RED).size(10.0));
        }

        let mut new_selection: Option<SelectedLog> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt("logs_list_scroll")
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 6.0);

                if live_active {
                    let snapshot = self.live_logger.as_ref().map(|l| l.snapshot());
                    let (subtitle, sample_count) = match snapshot {
                        Some(s) => (
                            format!(
                                "{}{}{} samples",
                                if s.device_name.is_empty() {
                                    String::new()
                                } else {
                                    format!("{} · ", s.device_name)
                                },
                                format!("{} · ", logger::format_duration(s.duration_ms)),
                                s.samples.len()
                            ),
                            s.samples.len(),
                        ),
                        None => ("waiting for data…".to_owned(), 0),
                    };
                    let selected = self.logs.selected == SelectedLog::Live;
                    let pulse = (self.now() * 2.5).sin() as f32 * 0.5 + 0.5;
                    let resp = session_row(
                        ui,
                        "Live session",
                        &subtitle,
                        selected,
                        HEART,
                        true,
                        pulse,
                        sample_count > 0,
                    );
                    if resp.clicked() {
                        new_selection = Some(SelectedLog::Live);
                    }
                }

                if self.logs.files.is_empty() && !live_active {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("No saved sessions yet. They're saved automatically when the device disconnects, or click the save icon in the main view.")
                            .color(TEXT_LO)
                            .size(10.0),
                    );
                    return;
                }

                for path in self.logs.files.iter() {
                    let (title, subtitle) = format_file_title_subtitle(path);
                    let selected = matches!(&self.logs.selected, SelectedLog::File(p) if p == path);
                    let resp = session_row(
                        ui,
                        &title,
                        &subtitle,
                        selected,
                        PURPLE,
                        false,
                        0.0,
                        true,
                    );
                    if resp.clicked() {
                        new_selection = Some(SelectedLog::File(path.clone()));
                    }
                }
            });

        if let Some(sel) = new_selection {
            self.logs.selected = sel;
            self.logs.export_status = None;
            self.load_selected_log();
        }
    }

    fn render_log_details(&mut self, ui: &mut egui::Ui) -> (bool, bool, bool) {
        let mut delete_clicked = false;
        let mut export_clicked = false;
        let mut save_live_clicked = false;

        let is_live = matches!(self.logs.selected, SelectedLog::Live);
        let Some(log) = self.current_log_for_view() else {
            if let Some(err) = &self.logs.load_error {
                ui.label(RichText::new(format!("Failed to load: {err}")).color(RED).size(11.0));
            } else if matches!(self.logs.selected, SelectedLog::None) {
                ui.label(
                    RichText::new("Select a session on the left.")
                        .color(TEXT_LO)
                        .size(11.0),
                );
            } else {
                ui.label(
                    RichText::new("Waiting for first sample…")
                        .color(TEXT_LO)
                        .size(11.0),
                );
            }
            return (delete_clicked, export_clicked, save_live_clicked);
        };

        let stats = log.full_stats();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .id_salt("log_details_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if is_live {
                        let pulse = (self.now() * 2.5).sin() as f32 * 0.5 + 0.5;
                        let alpha = (180.0 + 75.0 * pulse).clamp(0.0, 255.0) as u8;
                        let dot = Color32::from_rgba_unmultiplied(HEART.r(), HEART.g(), HEART.b(), alpha);
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(10.0, 14.0), Sense::hover());
                        ui.painter().circle_filled(rect.center(), 4.0, dot);
                        ui.label(
                            RichText::new("Live session")
                                .color(TEXT_HI)
                                .size(13.0)
                                .strong(),
                        );
                    } else {
                        ui.label(
                            RichText::new(log.started_at_display())
                                .color(TEXT_HI)
                                .size(13.0)
                                .strong(),
                        );
                    }
                });
                if !log.device_name.is_empty() {
                    ui.label(
                        RichText::new(format!("Device: {}", log.device_name))
                            .color(TEXT_LO)
                            .size(10.0),
                    );
                }
                ui.add_space(4.0);

                card(ui, |ui| {
                    ui.label(RichText::new("Brief").color(TEXT_LO).size(10.0));
                    ui.columns(4, |c| {
                        metric(&mut c[0], "Duration", &logger::format_duration(log.duration_ms), TEAL);
                        metric(
                            &mut c[1],
                            "Samples",
                            &stats.brief.sample_count.to_string(),
                            TEAL,
                        );
                        metric(
                            &mut c[2],
                            "Avg BPM",
                            &format!("{:.0}", stats.brief.avg_bpm),
                            HEART,
                        );
                        metric(
                            &mut c[3],
                            "Min/Max",
                            &format!("{} / {}", stats.brief.min_bpm, stats.brief.max_bpm),
                            HEART,
                        );
                    });
                });

                ui.add_space(6.0);
                render_full_stats_card(ui, &stats);

                ui.add_space(8.0);
                ui.label(RichText::new("BPM over time").color(TEXT_LO).size(10.0));
                render_session_bpm_plot(ui, &log);

                ui.add_space(6.0);
                ui.label(RichText::new("HRV · RMSSD over time").color(TEXT_LO).size(10.0));
                render_session_rmssd_plot(ui, &log);

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if is_live {
                        let enabled = log.samples.len() > 0;
                        if pill_btn(ui, "Save now", TEXT_HI, ButtonStyle::Primary, enabled).clicked() && enabled {
                            save_live_clicked = true;
                        }
                        if pill_btn(ui, "Export CSV", TEXT_HI, ButtonStyle::Neutral, enabled).clicked() && enabled {
                            export_clicked = true;
                        }
                    } else {
                        if pill_btn(ui, "Export CSV", TEXT_HI, ButtonStyle::Neutral, true).clicked() {
                            export_clicked = true;
                        }
                        if pill_btn(ui, "Delete", RED, ButtonStyle::Danger, true).clicked() {
                            delete_clicked = true;
                        }
                    }

                    if let Some((shown_at, msg, is_error)) = &self.logs.export_status {
                        if shown_at.elapsed() < Duration::from_secs(5) {
                            let color = if *is_error { RED } else { GREEN };
                            ui.label(RichText::new(msg.clone()).color(color).size(10.0));
                        }
                    }
                });
            });

        (delete_clicked, export_clicked, save_live_clicked)
    }

    fn delete_selected_log(&mut self) {
        let SelectedLog::File(path) = self.logs.selected.clone() else {
            return;
        };
        match std::fs::remove_file(&path) {
            Ok(_) => {
                self.show_toast(
                    format!(
                        "Deleted {}",
                        path.file_name().and_then(|s| s.to_str()).unwrap_or("session")
                    ),
                    false,
                );
                self.logs.selected = SelectedLog::None;
                self.rescan_logs();
            }
            Err(e) => {
                self.logs.export_status = Some((Instant::now(), format!("Delete failed: {e}"), true));
            }
        }
    }

    fn export_selected_csv(&mut self) {
        let Some(log) = self.current_log_for_view() else { return };
        let csv_path = match &self.logs.selected {
            SelectedLog::File(json_path) => json_path.with_extension("csv"),
            SelectedLog::Live => {
                let dir = logger::default_log_dir();
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    self.logs.export_status = Some((Instant::now(), format!("Export failed: {e}"), true));
                    return;
                }
                dir.join("live_session.csv")
            }
            SelectedLog::None => return,
        };
        match log.export_csv(&csv_path) {
            Ok(_) => {
                let name = csv_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("session.csv")
                    .to_owned();
                self.logs.export_status =
                    Some((Instant::now(), format!("Exported {name}"), false));
            }
            Err(e) => {
                self.logs.export_status = Some((Instant::now(), format!("Export failed: {e}"), true));
            }
        }
    }

    fn render_toast(&mut self, ui: &mut egui::Ui) {
        let Some(toast) = &self.toast else { return };
        let age = toast.shown_at.elapsed();
        if age > Duration::from_secs(4) {
            self.toast = None;
            return;
        }
        let alpha = (1.0 - (age.as_secs_f32() / 4.0).clamp(0.0, 1.0)).powf(0.5);
        let fg = if toast.is_error { RED } else { GREEN };
        let fg = Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), (255.0 * alpha) as u8);
        let bg = Color32::from_rgba_unmultiplied(CARD.r(), CARD.g(), CARD.b(), (230.0 * alpha) as u8);
        let stroke =
            Color32::from_rgba_unmultiplied(CARD_STROKE.r(), CARD_STROKE.g(), CARD_STROKE.b(), (255.0 * alpha) as u8);

        let text = toast.text.clone();
        let max_w = ui.max_rect().width() - 16.0;

        egui::Area::new(egui::Id::new("toast_area"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -10.0))
            .show(ui.ctx(), |ui| {
                ui.set_max_width(max_w);
                egui::Frame::NONE
                    .fill(bg)
                    .stroke(Stroke::new(1.0, stroke))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.label(RichText::new(text).color(fg).size(11.0));
                    });
            });
    }
}

fn render_full_stats_card(ui: &mut egui::Ui, stats: &FullStats) {
    card(ui, |ui| {
        ui.label(RichText::new("BPM").color(TEXT_LO).size(10.0));
        ui.columns(3, |c| {
            metric(
                &mut c[0],
                "Avg",
                &format!("{:.1}", stats.brief.avg_bpm),
                HEART,
            );
            metric(
                &mut c[1],
                "Std Dev",
                &format!("{:.1}", stats.bpm_stddev),
                HEART,
            );
            metric(
                &mut c[2],
                "Range",
                &format!("{}–{}", stats.brief.min_bpm, stats.brief.max_bpm),
                HEART,
            );
        });

        ui.add_space(4.0);
        ui.label(RichText::new("HRV").color(TEXT_LO).size(10.0));
        ui.columns(3, |c| {
            metric(
                &mut c[0],
                "RMSSD avg",
                &fmt_opt(stats.rmssd_avg),
                TEAL,
            );
            metric(
                &mut c[1],
                "SDNN avg",
                &fmt_opt(stats.sdnn_avg),
                TEAL,
            );
            metric(
                &mut c[2],
                "pNN50 avg",
                &fmt_opt_pct(stats.pnn50_avg),
                PURPLE,
            );
        });
        ui.columns(3, |c| {
            metric(
                &mut c[0],
                "RMSSD min/max",
                &fmt_opt_pair(stats.rmssd_min, stats.rmssd_max),
                TEAL,
            );
            metric(
                &mut c[1],
                "SDNN min/max",
                &fmt_opt_pair(stats.sdnn_min, stats.sdnn_max),
                TEAL,
            );
            metric(
                &mut c[2],
                "pNN50 min/max",
                &fmt_opt_pair_pct(stats.pnn50_min, stats.pnn50_max),
                PURPLE,
            );
        });

        ui.add_space(4.0);
        ui.label(RichText::new("HR zones").color(TEXT_LO).size(10.0));
        zone_bar(ui, "Resting (<60)", stats.zones.resting_pct(), GREEN);
        zone_bar(ui, "Light (60–99)", stats.zones.light_pct(), TEAL);
        zone_bar(ui, "Moderate (100–139)", stats.zones.moderate_pct(), WATER);
        zone_bar(ui, "Vigorous (140–169)", stats.zones.vigorous_pct(), AMBER);
        zone_bar(ui, "Maximum (≥170)", stats.zones.maximum_pct(), RED);
    });
}

fn render_session_bpm_plot(ui: &mut egui::Ui, log: &SessionLog) {
    let pts: Vec<[f64; 2]> = log
        .samples
        .iter()
        .map(|s| [s.t_ms as f64 / 1000.0, s.bpm as f64])
        .collect();
    let (y_lo, y_hi) = bpm_bounds(&pts);
    let x_max = (log.duration_ms as f64 / 1000.0).max(1.0);
    Plot::new("log_bpm")
        .height(140.0)
        .show_axes([true, true])
        .show_grid(true)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .include_x(0.0)
        .include_x(x_max)
        .include_y(y_lo)
        .include_y(y_hi)
        .show(ui, |plot_ui| {
            plot_ui.line(Line::new("BPM", egui_plot::PlotPoints::from(pts)).color(HEART).width(1.6));
        });
}

fn render_session_rmssd_plot(ui: &mut egui::Ui, log: &SessionLog) {
    let pts: Vec<[f64; 2]> = log
        .samples
        .iter()
        .filter_map(|s| s.rmssd.map(|v| [s.t_ms as f64 / 1000.0, v as f64]))
        .collect();
    let x_max = (log.duration_ms as f64 / 1000.0).max(1.0);
    Plot::new("log_rmssd")
        .height(100.0)
        .show_axes([true, true])
        .show_grid(true)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .include_x(0.0)
        .include_x(x_max)
        .include_y(0.0)
        .include_y(120.0)
        .show(ui, |plot_ui| {
            plot_ui.line(Line::new("RMSSD", egui_plot::PlotPoints::from(pts)).color(TEAL).width(1.6));
        });
}

fn bpm_bounds(pts: &[[f64; 2]]) -> (f64, f64) {
    if pts.is_empty() {
        return (40.0, 180.0);
    }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for p in pts {
        if p[1] < lo {
            lo = p[1];
        }
        if p[1] > hi {
            hi = p[1];
        }
    }
    let pad = ((hi - lo) * 0.1).max(5.0);
    ((lo - pad).max(30.0), (hi + pad).min(240.0))
}

fn zone_bar(ui: &mut egui::Ui, label: &str, pct: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(TEXT_LO).size(10.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{:.1}%", pct)).color(TEXT_HI).size(10.0));
            let bar_w = (ui.available_width() - 4.0).max(40.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 8.0), Sense::hover());
            let painter = ui.painter();
            let radius = CornerRadius::same(3);
            painter.rect_filled(rect, radius, Color32::from_rgb(28, 28, 49));
            let fill_w = rect.width() * (pct / 100.0).clamp(0.0, 1.0);
            let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
            painter.rect_filled(fill_rect, radius, color);
        });
    });
}

fn fmt_opt(v: Option<f32>) -> String {
    v.map_or("—".to_owned(), |x| format!("{:.1}", x))
}

fn fmt_opt_pct(v: Option<f32>) -> String {
    v.map_or("—".to_owned(), |x| format!("{:.1}%", x))
}

fn fmt_opt_pair(min: Option<f32>, max: Option<f32>) -> String {
    match (min, max) {
        (Some(a), Some(b)) => format!("{:.1}–{:.1}", a, b),
        _ => "—".to_owned(),
    }
}

fn fmt_opt_pair_pct(min: Option<f32>, max: Option<f32>) -> String {
    match (min, max) {
        (Some(a), Some(b)) => format!("{:.1}–{:.1}%", a, b),
        _ => "—".to_owned(),
    }
}

fn open_path(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

enum ChevronDir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy)]
enum IconKind {
    Logs,
    Save,
    Close,
    Refresh,
    Folder,
}

fn icon_btn(ui: &mut egui::Ui, kind: IconKind, tooltip: &str) -> egui::Response {
    let size = Vec2::new(ICON_SIZE, ICON_SIZE);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let bg = if response.hovered() { ICON_BG_HOVER } else { ICON_BG };
    let radius = CornerRadius::same(6);
    painter.rect_filled(rect, radius, bg);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, CARD_STROKE), egui::StrokeKind::Outside);

    let c = rect.center();
    let stroke = Stroke::new(1.5, TEXT_HI);

    match kind {
        IconKind::Logs => {
            for i in [-1.0_f32, 0.0, 1.0] {
                let y = c.y + i * 4.0;
                painter.line_segment(
                    [egui::pos2(c.x - 6.0, y), egui::pos2(c.x + 6.0, y)],
                    stroke,
                );
            }
        }
        IconKind::Save => {
            painter.line_segment(
                [egui::pos2(c.x, c.y - 6.0), egui::pos2(c.x, c.y + 3.0)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x - 4.0, c.y - 1.0), egui::pos2(c.x, c.y + 3.0)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x + 4.0, c.y - 1.0), egui::pos2(c.x, c.y + 3.0)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x - 6.0, c.y + 6.5), egui::pos2(c.x + 6.0, c.y + 6.5)],
                stroke,
            );
        }
        IconKind::Close => {
            painter.line_segment(
                [egui::pos2(c.x - 5.0, c.y - 5.0), egui::pos2(c.x + 5.0, c.y + 5.0)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(c.x + 5.0, c.y - 5.0), egui::pos2(c.x - 5.0, c.y + 5.0)],
                stroke,
            );
        }
        IconKind::Refresh => {
            painter.text(
                c,
                egui::Align2::CENTER_CENTER,
                "↻",
                FontId::proportional(14.0),
                TEXT_HI,
            );
        }
        IconKind::Folder => {
            let body = egui::Rect::from_min_size(
                egui::pos2(c.x - 6.5, c.y - 3.0),
                Vec2::new(13.0, 8.5),
            );
            let tab = egui::Rect::from_min_size(
                egui::pos2(c.x - 6.5, c.y - 5.5),
                Vec2::new(5.5, 3.0),
            );
            painter.rect_stroke(tab, CornerRadius::same(1), stroke, egui::StrokeKind::Inside);
            painter.rect_stroke(body, CornerRadius::same(2), stroke, egui::StrokeKind::Inside);
        }
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.on_hover_text(tooltip)
}

#[derive(Clone, Copy)]
enum ButtonStyle {
    Neutral,
    Primary,
    Danger,
}

fn pill_btn(
    ui: &mut egui::Ui,
    label: &str,
    text_color: Color32,
    style: ButtonStyle,
    enabled: bool,
) -> egui::Response {
    let font_id = FontId::proportional(11.5);
    let text_color = if enabled {
        text_color
    } else {
        Color32::from_rgba_unmultiplied(text_color.r(), text_color.g(), text_color.b(), 110)
    };
    let text_w = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font_id.clone(), text_color)
        .size()
        .x;
    let size = Vec2::new((text_w + 24.0).max(78.0), 26.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let painter = ui.painter();
    let hovered = enabled && response.hovered();
    let (bg, stroke_color) = match style {
        ButtonStyle::Neutral => (
            if hovered { ICON_BG_HOVER } else { ICON_BG },
            CARD_STROKE,
        ),
        ButtonStyle::Primary => (
            if hovered {
                Color32::from_rgb(48, 38, 80)
            } else {
                Color32::from_rgb(38, 30, 65)
            },
            PURPLE,
        ),
        ButtonStyle::Danger => (
            if hovered {
                Color32::from_rgb(58, 30, 36)
            } else {
                Color32::from_rgb(42, 24, 30)
            },
            Color32::from_rgb(96, 40, 50),
        ),
    };
    let radius = CornerRadius::same(6);
    painter.rect_filled(rect, radius, bg);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, stroke_color), egui::StrokeKind::Outside);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font_id,
        text_color,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

#[allow(clippy::too_many_arguments)]
fn session_row(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    selected: bool,
    accent: Color32,
    live: bool,
    pulse: f32,
    has_data: bool,
) -> egui::Response {
    let avail_w = ui.available_width();
    let h = 44.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(avail_w, h), Sense::click());
    let painter = ui.painter();
    let bg = if selected {
        Color32::from_rgb(32, 32, 56)
    } else if response.hovered() {
        ICON_BG_HOVER
    } else {
        ICON_BG
    };
    let radius = CornerRadius::same(8);
    painter.rect_filled(rect, radius, bg);
    let stroke_color = if selected { accent } else { CARD_STROKE };
    let stroke_width = if selected { 1.5 } else { 1.0 };
    painter.rect_stroke(rect, radius, Stroke::new(stroke_width, stroke_color), egui::StrokeKind::Outside);

    let pad_x = 10.0;
    let mut cursor_x = rect.min.x + pad_x;
    if live {
        let alpha = if has_data {
            (180.0 + 75.0 * pulse).clamp(0.0, 255.0) as u8
        } else {
            120
        };
        let dot_color = Color32::from_rgba_unmultiplied(HEART.r(), HEART.g(), HEART.b(), alpha);
        painter.circle_filled(egui::pos2(cursor_x + 4.0, rect.center().y), 4.0, dot_color);
        cursor_x += 14.0;
    }
    painter.text(
        egui::pos2(cursor_x, rect.min.y + 8.0),
        egui::Align2::LEFT_TOP,
        title,
        FontId::proportional(12.0),
        TEXT_HI,
    );
    painter.text(
        egui::pos2(cursor_x, rect.min.y + 25.0),
        egui::Align2::LEFT_TOP,
        subtitle,
        FontId::proportional(9.5),
        TEXT_LO,
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn format_file_title_subtitle(path: &std::path::Path) -> (String, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("session");
    let inner = stem.trim_start_matches("session_");
    if let Some((date, time)) = inner.split_once('_') {
        let time = time.replace('-', ":");
        (date.to_owned(), format!("{} UTC", time))
    } else {
        (stem.to_owned(), String::new())
    }
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
