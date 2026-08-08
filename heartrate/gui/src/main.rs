#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui;
use heartrate_core::{
    heartrate_device::HeartrateDevice,
    hrv::{HrvAnalyzer, HrvMetrics},
    log_buffer,
    logger::{self, SessionLogger},
    osc::OscSender,
    settings_manager::AppSettings,
};

pub enum GuiCommand {
    ResetHrv,
    SaveLog,
}

pub enum BleEvent {
    Scanning,
    Connected(String),
    Disconnected,
    Data { bpm: i32, hrv: Option<HrvMetrics> },
    FatalError(String),
    LogSaved(PathBuf),
    LogError(String),
}

fn main() -> eframe::Result {
    let _ = log_buffer::init();

    let (tx, rx) = mpsc::channel();
    let (tx_cmd, rx_cmd) = mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(ble_worker(tx, rx_cmd));
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(app::DEFAULT_WINDOW_SIZE)
            .with_min_inner_size(app::DEFAULT_WINDOW_SIZE),
        ..Default::default()
    };

    eframe::run_native(
        "HeartRate OSC",
        options,
        Box::new(|cc| Ok(Box::new(app::HeartRateApp::new(cc, rx, tx_cmd)))),
    )
}

async fn ble_worker(tx: mpsc::Sender<BleEvent>, rx_cmd: mpsc::Receiver<GuiCommand>) {
    let settings = match AppSettings::try_load_from_file("settings.json") {
        Ok(s) => s,
        Err(e) => {
            log::error!("[APP] Failed to load settings.json: {e}");
            let _ = tx.send(BleEvent::FatalError(format!("Settings: {e}")));
            return;
        }
    };

    let log_dir = logger::default_log_dir();
    let sender = OscSender::new(settings.send_port()).await;

    loop {
        let mut host = match HeartrateDevice::new().await {
            Ok(h) => h,
            Err(e) => {
                log::error!("[BLE] Bluetooth unavailable: {e}; retrying in 2s...");
                let _ = tx.send(BleEvent::FatalError(format!("Bluetooth: {e}")));
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };
        let mut hrv_analyzer = HrvAnalyzer::with_filter(settings.hrv_filter());
        let mut hrv_reset_until: Option<Instant> = None;
        let mut scanning = true;
        let mut session_logger: Option<SessionLogger> = None;
        log::info!("[BLE] Scanning for a heart rate device...");
        let _ = tx.send(BleEvent::Scanning);

        loop {
            if scanning {
                match host.auto_connect().await {
                    Ok(name) => {
                        log::info!("[BLE] Connected to {name}");
                        hrv_analyzer.reset();
                        session_logger = Some(SessionLogger::new(name.clone()));
                        let _ = tx.send(BleEvent::Connected(name));
                        scanning = false;
                    }
                    Err(err) => {
                        sender.send_bpm(0, settings.float_addresses(), settings.int_addresses()).await;
                        if err.is_recoverable() {
                            log::debug!("[BLE] {err}; continuing search...");
                            continue;
                        }
                        log::error!("[BLE] {err}; re-initializing...");
                        let _ = tx.send(BleEvent::FatalError(format!("{err}")));
                        break;
                    }
                }
            } else {
                while let Ok(cmd) = rx_cmd.try_recv() {
                    match cmd {
                        GuiCommand::ResetHrv => {
                            hrv_analyzer.reset();
                            hrv_reset_until = Some(Instant::now() + Duration::from_secs(3));
                        }
                        GuiCommand::SaveLog => {
                            if let Some(ref logger) = session_logger {
                                if !logger.is_empty() {
                                    match logger.save(&log_dir) {
                                        Ok(path) => {
                                            log::info!("[LOG] Session saved to {}", path.display());
                                            let _ = tx.send(BleEvent::LogSaved(path));
                                        }
                                        Err(e) => {
                                            log::warn!("[LOG] Failed to save session: {e}");
                                            let _ = tx.send(BleEvent::LogError(format!("{e}")));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                match host.get_bpm().await {
                    Ok(data) => {
                        let now = Instant::now();
                        let suppress_hrv = hrv_reset_until.is_some_and(|until| now < until);
                        if !suppress_hrv {
                            hrv_reset_until = None;
                            hrv_analyzer.add_rr_intervals(&data.intervals);
                        }
                        let hrv = if suppress_hrv { None } else { hrv_analyzer.compute() };

                        sender.send_bpm(data.bpm, settings.float_addresses(), settings.int_addresses()).await;

                        if let Some(ref m) = hrv {
                            sender.send_hrv(m, settings.hrv_addresses()).await;
                        }

                        if let Some(ref mut logger) = session_logger {
                            logger.record(data.bpm.max(0) as u16, hrv.as_ref());
                        }

                        let _ = tx.send(BleEvent::Data { bpm: data.bpm, hrv });
                    }
                    Err(err) => {
                        sender.send_bpm(0, settings.float_addresses(), settings.int_addresses()).await;

                        if let Some(logger) = session_logger.take() {
                            auto_save(&logger, &log_dir, &tx);
                        }

                        if err.is_recoverable() {
                            log::warn!("[BLE] {err}; returning to discovery...");
                            let _ = tx.send(BleEvent::Disconnected);
                            scanning = true;
                            continue;
                        }

                        log::error!("[BLE] {err}; re-initializing...");
                        let _ = tx.send(BleEvent::FatalError(format!("{err}")));
                        break;
                    }
                }
            }
        }

        let _ = host.disconnect().await;
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

const AUTO_SAVE_MIN_SAMPLES: usize = 5000;

fn auto_save(logger: &SessionLogger, log_dir: &std::path::Path, tx: &mpsc::Sender<BleEvent>) {
    if logger.sample_count() < AUTO_SAVE_MIN_SAMPLES {
        return;
    }
    match logger.save(log_dir) {
        Ok(path) => {
            log::info!("[LOG] Session auto-saved to {}", path.display());
            let _ = tx.send(BleEvent::LogSaved(path));
        }
        Err(e) => {
            log::warn!("[LOG] Failed to auto-save session: {e}");
            let _ = tx.send(BleEvent::LogError(format!("{e}")));
        }
    }
}
