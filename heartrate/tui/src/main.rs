pub mod app;
pub mod event;
pub mod page;
pub mod tui;
pub mod ui;
pub mod update;

use color_eyre::Result;
use ratatui::{Terminal, prelude::CrosstermBackend};

use crate::{
    app::App,
    event::{Event, EventHandler},
    tui::Tui,
};

fn main() -> Result<()> {
    let mut app = App::new();
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.enter()?;

    while !app.should_quit {
        tui.draw(&mut app)?;
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
