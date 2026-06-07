use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout},
    style::{Color, Style, Stylize},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::App;

pub fn render(app: &mut App, frame: &mut Frame) {
    let text = r"Press `Esc`, `Ctrl-C` or `q` to stop running.
    Press `j` and `k` to increment and decrement the counter respectively.";

    let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
        .flex(Flex::Center)
        .spacing(2)
        .split(frame.area());

    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .title(" Counter App ")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center),
        layout[0],
    );

    frame.render_widget(
        Paragraph::new(format!("Counter: {}", app.counter()))
            .block(
                Block::default()
                    .title(" Counter App ")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .fg(Color::White)
                    .border_type(BorderType::Rounded),
            )
            .style(Style::default().fg(Color::LightBlue))
            .alignment(Alignment::Center),
        layout[1],
    );
}
