use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Tabs},
};

use heartrate_core::{hrv::SignalQuality, log_buffer};

use crate::{
    app::App,
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
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(2)])
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

    let status_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray));
    let status_area = status_block.inner(chunks[2]);
    frame.render_widget(status_block, chunks[2]);
    render_status_bar(app, frame, status_area);
}

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let quality = app.current_hrv.map(|m| m.quality);
    let mut spans = signal_bars(quality);
    spans.push(Span::raw(" "));
    spans.push(match app.current_hrv {
        Some(m) if m.artifact_pct >= 0.5 => Span::styled(
            format!("{} · {:.0}%", m.quality.label(), m.artifact_pct),
            Style::default().fg(quality_color(quality)),
        ),
        Some(m) => Span::styled(m.quality.label(), Style::default().fg(quality_color(quality))),
        None => Span::styled("no clean beats", Style::default().fg(Color::DarkGray)),
    });

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn signal_bars(quality: Option<SignalQuality>) -> Vec<Span<'static>> {
    let level = match quality {
        Some(SignalQuality::Good) => 3,
        Some(SignalQuality::Fair) => 2,
        Some(SignalQuality::Poor) => 1,
        None => 0,
    };
    let lit = quality_color(quality);

    ["▁", "▃", "▅"]
        .into_iter()
        .enumerate()
        .map(|(i, glyph)| {
            let color = if i < level { lit } else { Color::DarkGray };
            Span::styled(glyph, Style::default().fg(color))
        })
        .collect()
}

fn quality_color(quality: Option<SignalQuality>) -> Color {
    match quality {
        Some(SignalQuality::Good) => Color::LightGreen,
        Some(SignalQuality::Fair) => Color::Yellow,
        Some(SignalQuality::Poor) => Color::LightRed,
        None => Color::DarkGray,
    }
}

#[cfg(test)]
mod tests {
    use heartrate_core::hrv::HrvMetrics;
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    fn draw_status_bar(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(60, 14)).unwrap();
        terminal.draw(|frame| render(app, frame)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let screen: Vec<String> = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol().to_owned())
                    .collect::<String>()
            })
            .collect();

        screen
            .iter()
            .find(|line| line.contains("▁▃▅"))
            .unwrap_or_else(|| panic!("no status bar in:\n{}", screen.join("\n")))
            .clone()
    }

    #[test]
    fn the_status_bar_reports_the_signal_quality() {
        let mut app = App::new();
        app.update_metrics(
            72,
            Some(&HrvMetrics {
                rmssd: 42.0,
                sdnn: 58.0,
                pnn50: 14.0,
                mean_hr: 71.0,
                artifact_pct: 8.0,
                quality: SignalQuality::Fair,
            }),
        );

        let status = draw_status_bar(&mut app);
        assert!(status.contains("fair · 8%"), "quality missing: {status:?}");
    }
    #[test]
    fn the_status_bar_says_so_when_there_is_nothing_to_report() {
        let mut app = App::new();
        app.update_metrics(0, None);

        let status = draw_status_bar(&mut app);
        assert!(status.contains("no clean beats"), "{status:?}");
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
    let recent_logs = log_buffer::last(20);

    let log_text = if recent_logs.is_empty() {
        "No log entries recorded yet.".to_string()
    } else {
        recent_logs
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    };

    let logs_paragraph = Paragraph::new(log_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Left);

    frame.render_widget(logs_paragraph, area);
}
