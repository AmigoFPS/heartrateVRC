pub mod app;
pub mod ble;
pub mod data;
pub mod event;
pub mod hrv;
pub mod logger;
pub mod osc;
pub mod page;
pub mod settings;
pub mod tui;
pub mod ui;
pub mod update;

use std::error::Error;

use ratatui::{Terminal, prelude::CrosstermBackend};
use tokio::sync::{mpsc, watch};

use crate::{
    app::App,
    ble::run_ble_loop,
    data::HeartRateData,
    event::{Event, EventHandler},
    logger::LocalLogger,
    osc::run_osc_loop,
    settings::Settings,
    tui::Tui,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // env_logger::builder().filter_level(log::LevelFilter::Debug).init();
    match LocalLogger::init() {
        Ok(_) => {}
        Err(e) => log::error!("TUI Logger error: {}", e),
    };

    let settings = Settings::default()
        .load_or_create("settings_v1.json")
        .expect("Unable to load settings");

    let (osc_tx, osc_rx) = mpsc::channel::<HeartRateData>(32);
    let (tui_tx, tui_rx) = watch::channel(HeartRateData::default());

    let mut app = App::new();
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;

    tokio::spawn(async move {
        run_ble_loop(osc_tx, tui_tx).await;
    });

    tokio::spawn(async move {
        run_osc_loop(osc_rx, &settings).await;
    });

    while !app.should_quit {
        let current_data = tui_rx.borrow();
        // TODO: Implement 3 metrics
        app.update_metrics(current_data.bpm, 0.0, 0.0, 0.0);
        drop(current_data);
        tui.draw(&mut app)?;

        match tui.events.next()? {
            Event::Tick => {}
            Event::Key(key_event) => update::update_key_event(&mut app, key_event),
            Event::Mouse(mouse_event) => update::update_mouse_event(&mut app, mouse_event),
            Event::Resize(_, _) => {}
        }
    }

    tui.exit()?;
    Ok(())
}
