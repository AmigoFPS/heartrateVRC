pub mod app;
pub mod event;
pub mod logger;
pub mod page;
pub mod tui;
pub mod ui;
pub mod update;

use std::{error::Error, time::Duration};

use btleplug::api::Peripheral;
use heartrate_core::{
    heartrate_device::HeartrateDevice, hrv::HrvAnalyzer, osc::OscSender, settings_manager::AppSettings,
};
use ratatui::{Terminal, prelude::CrosstermBackend};
use tokio::sync::mpsc;

use crate::{
    app::App,
    event::{Event, EventHandler},
    tui::Tui,
};

enum AppState {
    Scanning,
    Connected,
}

/// Simple payload varint passed from the background task to the frontend App
enum DeviceUpdate {
    BpmUpdated {
        bpm: u16,
        rmssd: f64,
        sdnn: f64,
        pnn50: f64,
    },
    Disconnected,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let settings = AppSettings::try_load_from_file("settings.json").expect("Unable to load settings");

    let mut app = App::new();
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;

    let (tx, mut rx) = mpsc::channel::<DeviceUpdate>(32);
    tokio::spawn(async move {
        let mut host = HeartrateDevice::new().await.expect("Unable to create device");
        let sender = OscSender::new([127, 0, 0, 1], settings.send_port());
        let mut hrv_analyzer = HrvAnalyzer::new();
        let mut state = AppState::Scanning;

        loop {
            match state {
                AppState::Scanning => match host.auto_connect().await {
                    Ok(device) => {
                        let properties = device.properties().await.unwrap_or_default();
                        let display_name = properties.and_then(|p| p.local_name).unwrap_or_else(String::new);

                        log::info!("Found device {}!", display_name);
                        hrv_analyzer = HrvAnalyzer::new();
                        state = AppState::Connected;
                    }
                    Err(err) => {
                        let _ = sender.send_bpm(0, settings.float_addresses(), settings.int_addresses());
                        match err {
                            btleplug::Error::DeviceNotFound | btleplug::Error::NotConnected => {
                                log::warn!("Device not found, continuing search...");
                            }
                            btleplug::Error::NoSuchCharacteristic => {
                                log::error!("Found device but NoSuchCharacteristic, continuing search...");
                            }
                            btleplug::Error::TimedOut(duration) => {
                                log::error!("Time out {}, continuing search...", duration.as_millis());
                            }
                            _ => {
                                log::error!("Fatal BLE Error encountered: {}", err);
                            }
                        }
                        state = AppState::Scanning;
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                    }
                },
                AppState::Connected => match host.get_bpm().await {
                    Ok(data) => {
                        let bpm = data.bpm;
                        hrv_analyzer.add_rr_intervals(&data.intervals);

                        if let Err(err) = sender.send_bpm(bpm, settings.float_addresses(), settings.int_addresses()) {
                            log::error!("Osc sending error: {}", err);
                        }

                        let mut rmssd = 0.0;
                        let mut sdnn = 0.0;
                        let mut pnn50 = 0.0;

                        if let Some(metrics) = hrv_analyzer.compute() {
                            rmssd = metrics.rmssd;
                            sdnn = metrics.sdnn;
                            pnn50 = metrics.pnn50;

                            if let Err(err) = sender.send_hrv(&metrics, settings.hrv_addresses()) {
                                log::error!("Osc HRV sending error: {}", err);
                            }
                            log::info!(
                                "Sending {} BPM | HRV RMSSD:{:.1} SDNN:{:.1} pNN50:{:.1}",
                                bpm,
                                rmssd,
                                sdnn,
                                pnn50
                            );
                        } else {
                            log::info!("Sending {} BPM", bpm);
                        }

                        let _ = tx
                            .send(DeviceUpdate::BpmUpdated {
                                bpm: bpm as u16,
                                rmssd: rmssd as f64,
                                sdnn: sdnn as f64,
                                pnn50: pnn50 as f64,
                            })
                            .await;
                    }
                    Err(err) => {
                        log::error!("Error: {}, searching for device...", err);
                        let _ = sender.send_bpm(0, settings.float_addresses(), settings.int_addresses());
                        let _ = host.disconnect().await;
                        let _ = tx.send(DeviceUpdate::Disconnected).await;
                        state = AppState::Scanning;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                },
            }
        }
    });

    while !app.should_quit {
        tui.draw(&mut app)?;

        while let Ok(update) = rx.try_recv() {
            match update {
                DeviceUpdate::BpmUpdated {
                    bpm,
                    rmssd,
                    sdnn,
                    pnn50,
                } => {
                    app.update_metrics(bpm, rmssd, sdnn, pnn50);
                }
                DeviceUpdate::Disconnected => {
                    app.current_bpm = 0;
                }
            }
        }

        match tui.events.next()? {
            Event::Tick => {}
            Event::Key(key_event) => update::update_key_event(&mut app, key_event),
            Event::Mouse(mouse_event) => update::update_mouse_event(&mut app, mouse_event),
            Event::Resize(_, _) => {}
        };
    }

    tui.exit()?;
    Ok(())
}
