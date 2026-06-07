use crate::page::Page;

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

    pub fn update_metrics(&mut self, bpm: u16, rmssd: f64, sdnn: f64, pnn50: f64) {
        self.current_bpm = bpm;
        let elapsed = self.start_time.elapsed().as_secs_f64();

        self.heartrate_data.push((elapsed, bpm as f64));
        self.rmssd_data.push((elapsed, rmssd));
        self.sdnn_data.push((elapsed, sdnn));
        self.pnn50_data.push((elapsed, pnn50));

        let max_points = 100;
        if self.heartrate_data.len() > max_points {
            self.heartrate_data.remove(0);
        }
        if self.rmssd_data.len() > max_points {
            self.rmssd_data.remove(0);
        }
        if self.sdnn_data.len() > max_points {
            self.sdnn_data.remove(0);
        }
        if self.pnn50_data.len() > max_points {
            self.pnn50_data.remove(0);
        }
    }
}
