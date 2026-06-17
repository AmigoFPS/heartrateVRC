#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use btleplug::{Error, api::Peripheral};
use eframe::egui;
use heartrate_core::{
    heartrate_device::HeartrateDevice,
    hrv::{HrvAnalyzer, HrvMetrics},
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
    let (tx, rx) = mpsc::channel();
    let (tx_cmd, rx_cmd) = mpsc::channel();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(ble_worker(tx, rx_cmd));
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 520.0])
            .with_min_inner_size([220.0, 220.0]),
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
            let _ = tx.send(BleEvent::FatalError(format!("Settings: {e}")));
            return;
        }
    };

    let log_dir = logger::default_log_dir();
    let sender = OscSender::new().await;

    loop {
        let mut host = match HeartrateDevice::new().await {
            Ok(h) => h,
            Err(e) => {
                let _ = tx.send(BleEvent::FatalError(format!("Bluetooth: {e}")));
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };
        let mut hrv_analyzer = HrvAnalyzer::new();
        let mut hrv_reset_until: Option<Instant> = None;
        let mut scanning = true;
        let mut session_logger: Option<SessionLogger> = None;
        let _ = tx.send(BleEvent::Scanning);

        loop {
            if scanning {
                match host.auto_connect().await {
                    Ok(device) => {
                        let name = device
                            .properties()
                            .await
                            .unwrap_or_default()
                            .and_then(|p| p.local_name)
                            .unwrap_or_default();
                        hrv_analyzer = HrvAnalyzer::new();
                        session_logger = Some(SessionLogger::new(name.clone()));
                        let _ = tx.send(BleEvent::Connected(name));
                        scanning = false;
                    }
                    Err(err) => {
                        sender.send_bpm(0, settings.float_addresses(), settings.int_addresses()).await;
                        match err {
                            Error::DeviceNotFound
                            | Error::NotConnected
                            | Error::NoSuchCharacteristic
                            | Error::TimedOut(_) => continue,
                            other => {
                                eprintln!("Connection error: {:?}", other);
                                let _ = tx.send(BleEvent::FatalError(format!("{other}")));
                                break;
                            }
                        }
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
                                            let _ = tx.send(BleEvent::LogSaved(path));
                                        }
                                        Err(e) => {
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
                        eprintln!("Get BPM error: {:?}", err);

                        sender.send_bpm(0, settings.float_addresses(), settings.int_addresses()).await;

                        match err {
                            Error::DeviceNotFound | Error::NotConnected | Error::TimedOut(_) => {
                                if let Some(logger) = session_logger.take() {
                                    auto_save(&logger, &log_dir, &tx);
                                }
                                let _ = tx.send(BleEvent::Disconnected);
                                scanning = true;
                                continue;
                            }
                            other => {
                                eprintln!("Unrecoverable error: {:?}", other);
                                if let Some(logger) = session_logger.take() {
                                    auto_save(&logger, &log_dir, &tx);
                                }
                                let _ = tx.send(BleEvent::FatalError(format!("{other}")));
                                break;
                            }
                        }
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
            let _ = tx.send(BleEvent::LogSaved(path));
        }
        Err(e) => {
            let _ = tx.send(BleEvent::LogError(format!("{e}")));
        }
    }
}
