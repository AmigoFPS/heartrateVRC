use ratatui::{
    layout::Rect,
    macros::ratatui_core,
    style::{Color, Style, Stylize},
    symbols,
    text::Line,
    widgets::{Axis, Chart, Dataset, GraphType},
};

#[derive(Default, Debug, PartialEq, Clone, Copy)]
pub enum Page {
    #[default]
    Heartrate,
    Rmssd,
    Sdnn,
    Pnn50,
    Logs,
}

impl From<Page> for usize {
    fn from(value: Page) -> Self {
        match value {
            Page::Heartrate => 0,
            Page::Rmssd => 1,
            Page::Sdnn => 2,
            Page::Pnn50 => 3,
            Page::Logs => 4,
        }
    }
}

impl From<Page> for ratatui_core::style::Color {
    fn from(value: Page) -> Self {
        match value {
            Page::Heartrate => Color::LightRed,
            Page::Rmssd => Color::LightCyan,
            Page::Sdnn => Color::LightGreen,
            Page::Pnn50 => Color::LightYellow,
            Page::Logs => Color::White,
        }
    }
}

impl From<Page> for Option<usize> {
    fn from(value: Page) -> Self {
        match value {
            Page::Heartrate => Some(0),
            Page::Rmssd => Some(1),
            Page::Sdnn => Some(2),
            Page::Pnn50 => Some(3),
            Page::Logs => Some(4),
        }
    }
}

pub fn draw_metric_chart(
    frame: &mut ratatui::Frame,
    area: Rect,
    data: &[(f64, f64)],
    line_color: Color,
    y_axis_title: &str,
    fallback_min_y: f64,
    fallback_max_y: f64,
) {
    let dataset = Dataset::default()
        .name(format!("{:.2} {}", data.last().unwrap_or(&(0.0, 0.0)).1, y_axis_title))
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(line_color))
        .data(data);

    let min_x = data.first().map(|d| d.0).unwrap_or(0.0);
    let max_x = data.last().map(|d| d.0).unwrap_or(100.0);

    let min_y = data
        .iter()
        .map(|d| d.1)
        .fold(f64::INFINITY, f64::min)
        .min(fallback_min_y);
    let max_y = data
        .iter()
        .map(|d| d.1)
        .fold(f64::NEG_INFINITY, f64::max)
        .max(fallback_max_y);

    let x_axis = Axis::default()
        .title("Time".italic())
        .style(Style::default().fg(Color::Gray))
        .bounds([min_x, max_x]);

    let y_axis = Axis::default()
        .title(y_axis_title.italic())
        .style(Style::default().fg(Color::Gray))
        .bounds([min_y * 0.9, max_y * 1.1])
        .labels(vec![
            Line::from(format!("{:.1}", min_y)),
            Line::from(format!("{:.1}", (min_y + max_y) / 2.0)),
            Line::from(format!("{:.1}", max_y)),
        ]);

    let chart = Chart::new(vec![dataset]).x_axis(x_axis).y_axis(y_axis);

    frame.render_widget(chart, area);
}
