use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    symbols,
    text::Line,
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Padding, Paragraph, Tabs},
};

use crate::{app::App, page::Page};

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
        .style(Style::default().fg(Color::LightRed).bg(Color::Black))
        .highlight_style(Style::default().fg(Color::Black).bg(Color::LightRed))
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

fn render_heartrate_page(app: &mut App, frame: &mut Frame, area: Rect) {
    let dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(Color::Red))
        .data(&app.heartrate_data);

    let min_x = app.heartrate_data.first().map(|d| d.0).unwrap_or(0.0);
    let max_x = app.heartrate_data.last().map(|d| d.0).unwrap_or(100.0);

    let min_y = app
        .heartrate_data
        .iter()
        .map(|d| d.1)
        .fold(f64::INFINITY, f64::min)
        .min(60.0);

    let max_y = app
        .heartrate_data
        .iter()
        .map(|d| d.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .max(120.0);

    let x_axis = Axis::default()
        .style(Style::default().fg(Color::Gray))
        .bounds([min_x, max_x]);

    let y_axis = Axis::default()
        .style(Style::default().fg(Color::Gray))
        .bounds([min_y - 5.0, max_y + 5.0])
        .labels(vec![
            Line::from(format!("{:.0}", min_y)),
            Line::from(format!("{:.0}", (min_y + max_y) / 2.0)),
            Line::from(format!("{:.0}", max_y)),
        ]);

    let chart = Chart::new(vec![dataset]).x_axis(x_axis).y_axis(y_axis);
    frame.render_widget(chart, area);
}

fn render_rmssd_page(_app: &mut App, frame: &mut Frame, area: ratatui::prelude::Rect) {
    let text = "RMSSD (Short-term HRV)\n\nCalculates root mean square of successive differences.";
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center).fg(Color::Cyan), area);
}

fn render_sdnn_page(_app: &mut App, frame: &mut Frame, area: ratatui::prelude::Rect) {
    let text = "SDNN (Overall HRV)\n\nStandard deviation of NN intervals.";
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center).fg(Color::Green), area);
}

fn render_pnn50_page(_app: &mut App, frame: &mut Frame, area: ratatui::prelude::Rect) {
    let text = "pNN50\n\nPercentage of successive NN intervals that differ by more than 50ms.";
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center).fg(Color::Yellow), area);
}

fn render_logs_page(_app: &mut App, frame: &mut Frame, area: ratatui::prelude::Rect) {
    let text = "Data Logs\n\n[Timestamped historical heartrate stream]";
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center).fg(Color::Gray), area);
}
