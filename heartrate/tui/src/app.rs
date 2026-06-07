use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, MouseEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
};

#[derive(Debug, Default)]
pub struct App {
    counter: u8,
    exit: bool,
}

enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

impl App {
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) => match key_event.kind {
                KeyEventKind::Press => self.handle_key_event(key_event),
                _ => {}
            },
            Event::Mouse(mouse_event) => match mouse_event.kind {
                MouseEventKind::ScrollUp => self.handle_scroll_event(ScrollDirection::Up),
                MouseEventKind::ScrollDown => self.handle_scroll_event(ScrollDirection::Down),
                MouseEventKind::ScrollLeft => self.handle_scroll_event(ScrollDirection::Left),
                MouseEventKind::ScrollRight => self.handle_scroll_event(ScrollDirection::Right),
                _ => {}
            },
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Left | KeyCode::Char('a') => self.decrement_counter(),
            KeyCode::Right | KeyCode::Char('d') => self.increment_counter(),
            _ => {}
        }
    }

    fn handle_scroll_event(&mut self, direction: ScrollDirection) {
        match direction {
            ScrollDirection::Up => self.increment_counter(),
            ScrollDirection::Down => self.decrement_counter(),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn decrement_counter(&mut self) {
        if self.counter > 0 {
            self.counter -= 1;
        }
    }

    fn increment_counter(&mut self) {
        if self.counter < u8::MAX {
            self.counter += 1;
        }
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(" Counter App Tutorial ".bold());
        let instructions = Line::from(vec![
            " Decrement ".into(),
            "<Left, A>".blue().bold(),
            " Increment ".into(),
            "<Right, D>".blue().bold(),
            " Quit ".into(),
            "<Q> ".blue().bold(),
        ]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let counter_text = Text::from(vec![Line::from(vec!["Value: ".into(), self.counter.to_string().yellow()])]);

        Paragraph::new(counter_text).centered().block(block).render(area, buf);
    }
}
