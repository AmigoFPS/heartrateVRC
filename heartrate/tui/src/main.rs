pub mod app;
pub mod event;
pub mod page;
pub mod tui;
pub mod ui;
pub mod update;

use std::error::Error;

use heartrate_core::runtime::Runtime;
use heartrate_core::{log_buffer, settings_manager::AppSettings};
use ratatui::{Terminal, prelude::CrosstermBackend};

use crate::{
    app::App,
    event::{Event, EventHandler},
    tui::Tui,
};

fn main() -> Result<(), Box<dyn Error>> {
    if let Err(e) = log_buffer::init() {
        eprintln!("TUI logger error: {}", e);
    }

    let settings = match AppSettings::load_or_create() {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("Could not load the settings: {e}; using the defaults");
            AppSettings::default()
        }
    };

    let runtime = Runtime::start(settings);

    let mut app = App::new();
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;

    let mut seen_revision = 0;
    while !app.should_quit {
        let revision = runtime.revision();
        if revision != seen_revision {
            seen_revision = revision;
            let (bpm, hrv) = runtime.read(|snap| (snap.bpm.max(0) as u16, snap.hrv));
            app.update_metrics(bpm, hrv.as_ref());
        }

        tui.draw(&mut app)?;

        match tui.events.next()? {
            Event::Tick => {}
            Event::Key(key_event) => update::update_key_event(&mut app, key_event),
            Event::Mouse(mouse_event) => update::update_mouse_event(&mut app, mouse_event),
            Event::Resize(_, _) => {}
        }
    }

    runtime.request_shutdown();
    tui.exit()?;
    Ok(())
}
