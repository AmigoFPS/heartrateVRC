use heartrate_core::hrv::HrvMetrics;

use crate::page::Page;

const MAX_POINTS: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub heartrate_data: Vec<(f64, f64)>,
    pub rmssd_data: Vec<(f64, f64)>,
    pub sdnn_data: Vec<(f64, f64)>,
    pub pnn50_data: Vec<(f64, f64)>,

    pub current_bpm: u16,
    pub current_page: Page,
    pub should_quit: bool,
    pub start_time: std::time::Instant,
}

impl Default for App {
    fn default() -> Self {
        Self {
            heartrate_data: Default::default(),
            current_bpm: Default::default(),
            rmssd_data: Default::default(),
            sdnn_data: Default::default(),
            pnn50_data: Default::default(),
            current_page: Page::Heartrate,
            should_quit: Default::default(),
            start_time: std::time::Instant::now(),
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn quit(&mut self) {
        self.should_quit = true
    }

    pub fn update_metrics(&mut self, bpm: u16, hrv: Option<&HrvMetrics>) {
        self.current_bpm = bpm;
        let elapsed = self.start_time.elapsed().as_secs_f64();

        push_point(&mut self.heartrate_data, (elapsed, bpm as f64));

        if let Some(hrv) = hrv {
            push_point(&mut self.rmssd_data, (elapsed, hrv.rmssd as f64));
            push_point(&mut self.sdnn_data, (elapsed, hrv.sdnn as f64));
            push_point(&mut self.pnn50_data, (elapsed, hrv.pnn50 as f64));
        }
    }
}

fn push_point(series: &mut Vec<(f64, f64)>, point: (f64, f64)) {
    series.push(point);
    if series.len() > MAX_POINTS {
        series.remove(0);
    }
}
