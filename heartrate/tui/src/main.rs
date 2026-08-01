pub mod app;
pub mod data;
pub mod event;
pub mod page;
pub mod tui;
pub mod ui;
pub mod update;
pub mod worker;

use std::error::Error;

use heartrate_core::{log_buffer, settings_manager::AppSettings};
use ratatui::{Terminal, prelude::CrosstermBackend};
use tokio::sync::watch;

use crate::{
    app::App,
    data::HeartRateData,
    event::{Event, EventHandler},
    tui::Tui,
    worker::run_worker,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if let Err(e) = log_buffer::init() {
        eprintln!("TUI logger error: {}", e);
    }

    let settings = AppSettings::try_load_from_file("settings.json").expect("Unable to load settings");

    let (tui_tx, tui_rx) = watch::channel(HeartRateData::default());

    let mut app = App::new();
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;

    tokio::spawn(async move {
        run_worker(settings, tui_tx).await;
    });

    while !app.should_quit {
        {
            let current_data = tui_rx.borrow();
            app.update_metrics(current_data.bpm, current_data.hrv.as_ref());
        }
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
