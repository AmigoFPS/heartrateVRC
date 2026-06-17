use btleplug::{Error, api::Peripheral};
use heartrate_core::{
    heartrate_device::HeartrateDevice, hrv::HrvAnalyzer, osc::OscSender, settings_manager::AppSettings,
};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let settings = AppSettings::try_load_from_file("settings.json").expect("Unable to load settings");
    let mut host = HeartrateDevice::new().await.expect("Unable to create device");
    let sender = OscSender::new().await;
    let mut hrv_analyzer = HrvAnalyzer::new();
    let mut state = AppState::Scanning;

    loop {
        match state {
            AppState::Scanning => match host.auto_connect().await {
                Ok(device) => {
                    let properties = device.properties().await.unwrap_or_default();
                    let display_name = properties.and_then(|p| p.local_name).unwrap_or_else(|| String::new());

                    println!("Found device {}!", display_name);
                    hrv_analyzer = HrvAnalyzer::new();
                    state = AppState::Sending;
                    continue;
                }
                Err(err) => {
                    sender.send_bpm(0, settings.float_addresses(), settings.int_addresses()).await;
                    match err {
                        Error::DeviceNotFound | Error::NotConnected => {
                            eprintln!("Device not found, continuing search...");
                            state = AppState::Scanning;
                            continue;
                        }
                        Error::NoSuchCharacteristic => {
                            eprintln!("Found device but NoSuchCharacteristic, continuing search...");
                            state = AppState::Scanning;
                            continue;
                        }
                        Error::TimedOut(duration) => {
                            eprintln!("Time out {}, continuing search...", duration.as_millis());
                            state = AppState::Scanning;
                            continue;
                        }
                        _ => panic!("Error: {}", err),
                    }
                }
            },
            AppState::Sending => match host.get_bpm().await {
                Ok(data) => {
                    hrv_analyzer.add_rr_intervals(&data.intervals);
                    sender.send_bpm(data.bpm, settings.float_addresses(), settings.int_addresses()).await;
                    if let Some(metrics) = hrv_analyzer.compute() {
                        sender.send_hrv(&metrics, settings.hrv_addresses()).await;
                        println!(
                            "Sending {} BPM | HRV RMSSD:{:.1} SDNN:{:.1} pNN50:{:.1}",
                            data.bpm, metrics.rmssd, metrics.sdnn, metrics.pnn50
                        );
                    } else {
                        println!("Sending {} BPM", data.bpm);
                    }
                }
                Err(err) => {
                    eprintln!("Error: {}, searching for device...", err);
                    sender.send_bpm(0, settings.float_addresses(), settings.int_addresses()).await;
                    let _ = host.disconnect().await;
                    state = AppState::Scanning;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            },
        }
    }
}

enum AppState {
    Scanning,
    Sending,
}
