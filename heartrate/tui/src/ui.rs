use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Tabs},
};

use crate::{
    app::App,
    logger::LocalLogger,
    page::{Page, draw_metric_chart},
};

pub fn render(app: &mut App, frame: &mut Frame) {
    let main_block = Block::new()
        .title(" Heartrate App ")
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .fg(Color::White)
        .bg(Color::Black)
        .border_type(BorderType::Rounded)
        .padding(Padding::uniform(1));

    let main_area = frame.area();
    let inner_area = main_block.inner(main_area);

    frame.render_widget(main_block, main_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(inner_area);

    let tab_titles = vec!["1 Heartrate", "2 RMSSD", "3 SDNN", "4 pNN50", "5 Logs"];

    let tabs = Tabs::new(tab_titles)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .select(app.current_page)
        .style(Style::default().fg(app.current_page.into()).bg(Color::Black))
        .highlight_style(Style::default().fg(Color::Black).bg(app.current_page.into()))
        .divider("|");

    frame.render_widget(tabs, chunks[0]);

    match app.current_page {
        Page::Heartrate => render_heartrate_page(app, frame, chunks[1]),
        Page::Rmssd => render_rmssd_page(app, frame, chunks[1]),
        Page::Sdnn => render_sdnn_page(app, frame, chunks[1]),
        Page::Pnn50 => render_pnn50_page(app, frame, chunks[1]),
        Page::Logs => render_logs_page(app, frame, chunks[1]),
    }
}

pub fn render_heartrate_page(app: &mut App, frame: &mut ratatui::Frame, area: Rect) {
    draw_metric_chart(frame, area, &app.heartrate_data, app.current_page.into(), "BPM", 60.0, 120.0);
}

pub fn render_rmssd_page(app: &mut App, frame: &mut ratatui::Frame, area: Rect) {
    draw_metric_chart(frame, area, &app.rmssd_data, app.current_page.into(), "ms", 10.0, 100.0);
}

pub fn render_sdnn_page(app: &mut App, frame: &mut ratatui::Frame, area: Rect) {
    draw_metric_chart(frame, area, &app.sdnn_data, app.current_page.into(), "ms", 20.0, 150.0);
}

pub fn render_pnn50_page(app: &mut App, frame: &mut ratatui::Frame, area: Rect) {
    draw_metric_chart(frame, area, &app.pnn50_data, app.current_page.into(), "%", 0.0, 100.0);
}

fn render_logs_page(_app: &mut App, frame: &mut ratatui::Frame, area: ratatui::prelude::Rect) {
    let recent_logs = LocalLogger::get_last_lines(20);

    let log_text = if recent_logs.is_empty() {
        "No log entries recorded yet.".to_string()
    } else {
        recent_logs.join("\n")
    };

    let logs_paragraph = Paragraph::new(log_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Left);

    frame.render_widget(logs_paragraph, area);
}
