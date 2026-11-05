use std::time::Duration;

use heartrate_core::{
    runtime::{DeviceStatus, Runtime},
    settings_manager::AppSettings,
};

fn main() {
    let settings = match AppSettings::load_or_create() {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("Could not load the settings: {e}; using the defaults");
            AppSettings::default()
        }
    };

    let runtime = Runtime::start(settings);
    let mut seen_revision = 0;

    loop {
        let revision = runtime.revision();
        if revision == seen_revision {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        seen_revision = revision;

        runtime.read(|snap| match &snap.device {
            DeviceStatus::Connected(_) if snap.bpm > 0 => match snap.hrv {
                Some(m) => println!(
                    "Sending {} BPM | HRV RMSSD:{:.1} SDNN:{:.1} pNN50:{:.1} | signal {} ({:.0}% artifacts)",
                    snap.bpm,
                    m.rmssd,
                    m.sdnn,
                    m.pnn50,
                    m.quality.label(),
                    m.artifact_pct
                ),
                None => println!("Sending {} BPM", snap.bpm),
            },
            DeviceStatus::Connected(name) => println!("Connected to {name}"),
            DeviceStatus::Scanning => println!("Searching for a heart rate device..."),
            DeviceStatus::Error(message) => eprintln!("{message}"),
        });
    }
}
