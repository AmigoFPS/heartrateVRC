use std::time::Duration;

use heartrate_core::{
    heartrate_device::HeartrateDevice, hrv::HrvAnalyzer, osc::OscSender, settings_manager::AppSettings,
};
use tokio::sync::watch;

use crate::data::HeartRateData;

pub async fn run_worker(settings: AppSettings, tui_tx: watch::Sender<HeartRateData>) {
    let sender = OscSender::new(settings.send_port()).await;

    loop {
        let mut host = match HeartrateDevice::new().await {
            Ok(h) => h,
            Err(e) => {
                log::error!("[BLE] Bluetooth unavailable: {}; retrying in 5s...", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let mut hrv_analyzer = HrvAnalyzer::new();
        let mut scanning = true;
        log::info!("[BLE] Scanning for a heart rate device...");

        loop {
            if scanning {
                match host.auto_connect().await {
                    Ok(name) => {
                        log::info!("[BLE] Connected to {}", name);
                        hrv_analyzer.reset();
                        scanning = false;
                    }
                    Err(err) => {
                        sender.send_bpm(0, settings.float_addresses(), settings.int_addresses()).await;
                        let _ = tui_tx.send(HeartRateData::default());

                        if err.is_recoverable() {
                            log::debug!("[BLE] {}; continuing search...", err);
                            continue;
                        }
                        log::error!("[BLE] {}; re-initializing...", err);
                        break;
                    }
                }
            } else {
                match host.get_bpm().await {
                    Ok(data) => {
                        hrv_analyzer.add_rr_intervals(&data.intervals);
                        let hrv = hrv_analyzer.compute();

                        sender.send_bpm(data.bpm, settings.float_addresses(), settings.int_addresses()).await;
                        if let Some(ref metrics) = hrv {
                            sender.send_hrv(metrics, settings.hrv_addresses()).await;
                        }

                        let _ = tui_tx.send(HeartRateData {
                            bpm: data.bpm.max(0) as u16,
                            hrv,
                        });
                    }
                    Err(err) => {
                        sender.send_bpm(0, settings.float_addresses(), settings.int_addresses()).await;
                        let _ = tui_tx.send(HeartRateData::default());

                        if err.is_recoverable() {
                            log::warn!("[BLE] {}; returning to discovery...", err);
                            scanning = true;
                            continue;
                        }
                        log::error!("[BLE] {}; re-initializing...", err);
                        break;
                    }
                }
            }
        }

        let _ = host.disconnect().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
