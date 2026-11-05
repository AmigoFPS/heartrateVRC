use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use heartrate_core::{
    hrv::{HrvMetrics, SignalQuality},
    runtime::{DeviceStatus, Snapshot},
};
use heartrate_vr::{
    panel::{self, Input, Live, Panel},
    render::{PanelRenderer, png},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/panel-preview".to_owned());
    std::fs::create_dir_all(&out_dir)?;

    let mut renderer = PanelRenderer::new(panel::DESIGN_WIDTH, panel::DESIGN_HEIGHT)?;
    println!("font: {}", renderer.font_path());

    let now = Instant::now();
    let now_t = 600.0;

    let connected = Snapshot {
        device: DeviceStatus::Connected("Polar H10 ABC123".into()),
        bpm: 86,
        hrv: Some(HrvMetrics {
            rmssd:        42.7,
            sdnn:         58.3,
            pnn50:        18.4,
            mean_hr:      86.0,
            artifact_pct: 6.0,
            quality:      SignalQuality::Fair,
        }),
        bpm_history: wave(now_t, 60.0, 110.0, 0.05),
        rmssd_history: wave(now_t, 20.0, 70.0, 0.013),
        last_data_t: now_t - 0.4,
        osc_connected: true,
        osc_target: Some("127.0.0.1:9000".into()),
        ..Snapshot::default()
    };

    let searching = Snapshot {
        device: DeviceStatus::Scanning,
        ..Snapshot::default()
    };

    let stale = Snapshot {
        device: DeviceStatus::Connected("Polar H10 ABC123".into()),
        bpm: 86,
        last_data_t: now_t - 45.0,
        bpm_history: wave(now_t - 45.0, 60.0, 110.0, 0.05),
        osc_connected: false,
        osc_target: Some("127.0.0.1:9000".into()),
        ..Snapshot::default()
    };

    let error = Snapshot {
        device: DeviceStatus::Error("Bluetooth: No Bluetooth adapter found".into()),
        ..Snapshot::default()
    };

    for (name, snap) in [
        ("connected", &connected),
        ("searching", &searching),
        ("stale", &stale),
        ("error", &error),
    ] {
        let mut p = Panel::new(panel::DESIGN_WIDTH, panel::DESIGN_HEIGHT);
        p.set_live(Live::from_snapshot(snap, now, now_t));
        write(&mut renderer, &p, &out_dir, name)?;
    }

    let mut holding = Panel::new(panel::DESIGN_WIDTH, panel::DESIGN_HEIGHT);
    holding.set_live(Live::from_snapshot(&connected, now, now_t));
    let reset = holding.metrics().reset;
    holding.handle(
        Input::Press {
            x: reset.center_x(),
            y: reset.center_y(),
        },
        now,
    );
    holding.tick(now + Duration::from_millis(600));
    write(&mut renderer, &holding, &out_dir, "holding")?;

    let mut cooling = Panel::new(panel::DESIGN_WIDTH, panel::DESIGN_HEIGHT);
    cooling.set_live(Live {
        reset_cooldown: 7,
        reset_recharge: 0.36,
        ..Live::from_snapshot(&connected, now, now_t)
    });
    cooling.set_notice(1, "Log saved: 2026-10-31_14-02-11.json", false);
    write(&mut renderer, &cooling, &out_dir, "cooling")?;

    let size = 128;
    let icon = heartrate_vr::render::tab_icon(size);
    let path = format!("{out_dir}/tab-icon.png");
    std::fs::write(&path, png::encode_rgba(size, size, &icon))?;
    println!("wrote {path} ({size}x{size})");

    Ok(())
}

fn write(renderer: &mut PanelRenderer, panel: &Panel, dir: &str, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (_, w, h) = renderer.render(panel);
    let pixels = renderer.straight_alpha_pixels().to_vec();
    let path = format!("{dir}/panel-{name}.png");
    std::fs::write(&path, png::encode_rgba(w, h, &pixels))?;
    println!("wrote {path} ({w}x{h})");
    Ok(())
}

fn wave(now_t: f64, low: f64, high: f64, rate: f64) -> VecDeque<[f64; 2]> {
    (0..120)
        .map(|i| {
            let t = now_t - 120.0 + i as f64;
            let phase = (i as f64 * rate * std::f64::consts::TAU).sin();
            [t, low + (high - low) * (0.5 + 0.5 * phase)]
        })
        .collect()
}
