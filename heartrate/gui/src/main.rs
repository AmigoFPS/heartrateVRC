#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use btleplug::{Error, api::Peripheral};
use eframe::egui;
use heartrate_core::{
    heartrate_device::HeartrateDevice,
    hrv::{HrvAnalyzer, HrvMetrics},
    osc::OscSender,
    settings_manager::AppSettings,
};

pub enum GuiCommand {
    ResetHrv,
}

pub enum BleEvent {
    Scanning,
    Connected(String),
    Disconnected,
    Data { bpm: i32, hrv: Option<HrvMetrics> },
    FatalError(String),
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
            .with_inner_size([320.0, 560.0])
            .with_min_inner_size([320.0, 560.0]),
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

    loop {
        let mut host = match HeartrateDevice::new().await {
            Ok(h) => h,
            Err(e) => {
                let _ = tx.send(BleEvent::FatalError(format!("Bluetooth: {e}")));
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        let sender = OscSender::new([127, 0, 0, 1], settings.send_port());
        let mut hrv_analyzer = HrvAnalyzer::new();
        let mut hrv_reset_until: Option<Instant> = None;
        let mut scanning = true;
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
                        let _ = tx.send(BleEvent::Connected(name));
                        scanning = false;
                    }
                    Err(err) => {
                        let _ = sender.send_bpm(0, settings.float_addresses(), settings.int_addresses());
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

                        let _ = sender.send_bpm(data.bpm, settings.float_addresses(), settings.int_addresses());

                        if let Some(ref m) = hrv {
                            let _ = sender.send_hrv(m, settings.hrv_addresses());
                        }

                        let _ = tx.send(BleEvent::Data { bpm: data.bpm, hrv });
                    }
                    Err(err) => {
                        eprintln!("Get BPM error: {:?}", err);

                        let _ = sender.send_bpm(0, settings.float_addresses(), settings.int_addresses());

                        match err {
                            Error::DeviceNotFound | Error::NotConnected | Error::TimedOut(_) => {
                                let _ = tx.send(BleEvent::Disconnected);
                                scanning = true;
                                continue;
                            }
                            other => {
                                eprintln!("Unrecoverable error: {:?}", other);
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
