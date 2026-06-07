use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{app::App, page::Page};

pub fn update_key_event(app: &mut App, key_event: KeyEvent) {
    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
        match key_event.code {
            KeyCode::Char('c') | KeyCode::Char('C') => app.quit(),
            KeyCode::Char('1') => app.set_current_page(Page::Heartrate),
            KeyCode::Char('2') => app.set_current_page(Page::Rmssd),
            KeyCode::Char('3') => app.set_current_page(Page::Sdnn),
            KeyCode::Char('4') => app.set_current_page(Page::Pnn50),
            KeyCode::Char('5') => app.set_current_page(Page::Logs),
            _ => {}
        }
        return;
    }

    match key_event.code {
        KeyCode::Esc | KeyCode::Char('q') => app.quit(),
        _ => {}
    };
}

pub fn update_mouse_event(app: &mut App, mouse_event: MouseEvent) {
    match mouse_event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let col = mouse_event.column;
            let row = mouse_event.row;

            if row == 1 {
                match col {
                    1..=11 => app.set_current_page(Page::Heartrate),
                    13..=20 => app.set_current_page(Page::Rmssd),
                    22..=28 => app.set_current_page(Page::Sdnn),
                    30..=37 => app.set_current_page(Page::Pnn50),
                    39..=45 => app.set_current_page(Page::Logs),
                    _ => {}
                }
            }
        }
        MouseEventKind::Down(mouse_button) if mouse_button.is_middle() => app.quit(),
        _ => {}
    }
}
