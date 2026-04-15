use std::collections::VecDeque;

const RR_UNIT_MS: f64 = 1000.0 / 1024.0;

const MIN_INTERVALS: usize = 10;

const WINDOW_SIZE: usize = 256;

#[derive(Debug, Clone)]
pub struct HrvMetrics {
    pub rmssd: f32,
    pub sdnn: f32,
    pub pnn50: f32,
}

pub struct HrvAnalyzer {
    rr_ms: VecDeque<f64>,
}

impl HrvAnalyzer {
    pub fn new() -> Self {
        Self {
            rr_ms: VecDeque::with_capacity(WINDOW_SIZE),
        }
    }

    pub fn add_rr_intervals(&mut self, rr_raw: &[u16]) {
        for &v in rr_raw {
            let ms = v as f64 * RR_UNIT_MS;
            self.rr_ms.push_back(ms);
            while self.rr_ms.len() > WINDOW_SIZE {
                self.rr_ms.pop_front();
            }
        }
    }

    pub fn compute(&self) -> Option<HrvMetrics> {
        let n = self.rr_ms.len();
        if n < MIN_INTERVALS {
            return None;
        }

        let rr: Vec<f64> = self.rr_ms.iter().copied().collect();
        let mean = rr.iter().sum::<f64>() / n as f64;

        let variance = rr.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
        let sdnn = variance.sqrt() as f32;

        let mut sum_sq_diff = 0.0;
        let mut count_diff = 0;
        let mut nn50 = 0usize;

        for i in 0..n - 1 {
            let diff = rr[i + 1] - rr[i];
            sum_sq_diff += diff * diff;
            count_diff += 1;
            if diff.abs() > 50.0 {
                nn50 += 1;
            }
        }

        let rmssd = if count_diff > 0 {
            (sum_sq_diff / count_diff as f64).sqrt() as f32
        } else {
            0.0
        };

        let pnn50 = if count_diff > 0 {
            (nn50 as f32 / count_diff as f32) * 100.0
        } else {
            0.0
        };

        Some(HrvMetrics {
            rmssd,
            sdnn,
            pnn50,
        })
    }
}

impl Default for HrvAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
