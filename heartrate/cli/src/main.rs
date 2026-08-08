use heartrate_core::{
    heartrate_device::HeartrateDevice, hrv::HrvAnalyzer, osc::OscSender, settings_manager::AppSettings,
};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let settings = AppSettings::try_load_from_file("settings.json").expect("Unable to load settings");
    let mut host = HeartrateDevice::new().await.expect("Unable to create device");
    let sender = OscSender::new(settings.send_port()).await;
    let mut hrv_analyzer = HrvAnalyzer::with_filter(settings.hrv_filter());
    let mut state = AppState::Scanning;

    loop {
        match state {
            AppState::Scanning => match host.auto_connect().await {
                Ok(name) => {
                    println!("Found device {}!", name);
                    hrv_analyzer = HrvAnalyzer::with_filter(settings.hrv_filter());
                    state = AppState::Sending;
                    continue;
                }
                Err(err) => {
                    sender.send_bpm(0, settings.float_addresses(), settings.int_addresses()).await;
                    eprintln!("{err}, continuing search...");
                    if !err.is_recoverable() {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                    state = AppState::Scanning;
                    continue;
                }
            },
            AppState::Sending => match host.get_bpm().await {
                Ok(data) => {
                    hrv_analyzer.add_rr_intervals(&data.intervals);
                    sender.send_bpm(data.bpm, settings.float_addresses(), settings.int_addresses()).await;
                    if let Some(metrics) = hrv_analyzer.compute() {
                        sender.send_hrv(&metrics, settings.hrv_addresses()).await;
                        println!(
                            "Sending {} BPM | HRV RMSSD:{:.1} SDNN:{:.1} pNN50:{:.1} | signal {} ({:.0}% artifacts)",
                            data.bpm,
                            metrics.rmssd,
                            metrics.sdnn,
                            metrics.pnn50,
                            metrics.quality.label(),
                            metrics.artifact_pct
                        );
                    } else {
                        println!("Sending {} BPM", data.bpm);
                    }
                }
                Err(err) => {
                    eprintln!("{err}, searching for device...");
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
