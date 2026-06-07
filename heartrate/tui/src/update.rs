use crossterm::event::MouseEvent;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;

pub fn update_key_event(app: &mut App, key_event: KeyEvent) {
    match key_event.code {
        KeyCode::Esc | KeyCode::Char('q') => app.quit(),
        KeyCode::Char('c') | KeyCode::Char('C') if key_event.modifiers == KeyModifiers::CONTROL => app.quit(),
        KeyCode::Right | KeyCode::Char('k') => app.increment_counter(),
        KeyCode::Left | KeyCode::Char('j') => app.decrement_counter(),
        _ => {}
    };
}

pub fn update_mouse_event(app: &mut App, mouse_event: MouseEvent) {
    match mouse_event.kind {
        crossterm::event::MouseEventKind::ScrollDown => app.decrement_counter(),
        crossterm::event::MouseEventKind::ScrollUp => app.increment_counter(),
        _ => {}
    }
}
