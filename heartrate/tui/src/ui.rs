use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    widgets::{Block, BorderType, Borders, Paragraph, Tabs},
};

use crate::{app::App, page::Page};

pub fn render(app: &mut App, frame: &mut Frame) {
    let main_block = Block::new()
        .title(" Heartrate App ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .fg(Color::White)
        .border_type(BorderType::Rounded);

    let main_area = frame.area();
    let inner_area = main_block.inner(main_area);

    frame.render_widget(main_block, main_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner_area);

    let tab_titles = vec!["Heartrate", "RMSSD", "SDNN", "pNN50", "Logs"];

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .select(app.current_page())
        .style(Style::default().fg(Color::Gray))
        .highlight_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
        .divider(symbols::DOT);

    frame.render_widget(tabs, chunks[0]);

    match app.current_page() {
        Page::Heartrate => render_heartrate_page(app, frame, chunks[1]),
        Page::Rmssd => render_rmssd_page(app, frame, chunks[1]),
        Page::Sdnn => render_sdnn_page(app, frame, chunks[1]),
        Page::Pnn50 => render_pnn50_page(app, frame, chunks[1]),
        Page::Logs => render_logs_page(app, frame, chunks[1]),
    }
}

fn render_heartrate_page(_app: &mut App, frame: &mut Frame, area: ratatui::prelude::Rect) {
    let text = "❤️ Heartrate Monitor\n\n[Insert live graph / current BPM here]";
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center).fg(Color::Red), area);
}

fn render_rmssd_page(_app: &mut App, frame: &mut Frame, area: ratatui::prelude::Rect) {
    let text = "📊 RMSSD (Short-term HRV)\n\nCalculates root mean square of successive differences.";
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center).fg(Color::Cyan), area);
}

fn render_sdnn_page(_app: &mut App, frame: &mut Frame, area: ratatui::prelude::Rect) {
    let text = "📈 SDNN (Overall HRV)\n\nStandard deviation of NN intervals.";
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center).fg(Color::Green), area);
}

fn render_pnn50_page(_app: &mut App, frame: &mut Frame, area: ratatui::prelude::Rect) {
    let text = "⏱️ pNN50\n\nPercentage of successive NN intervals that differ by more than 50ms.";
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center).fg(Color::Yellow), area);
}

fn render_logs_page(_app: &mut App, frame: &mut Frame, area: ratatui::prelude::Rect) {
    let text = "📋 Data Logs\n\n[Timestamped historical heartrate stream]";
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center).fg(Color::Gray), area);
}
