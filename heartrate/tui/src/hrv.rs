#[derive(Debug, Clone, Default)]
pub struct HrvMetrics {
    /// Root Mean Square of Successive Differences (ms)
    pub rmssd: f64,
    /// Standard Deviation of NN Intervals (ms)
    pub sdnn: f64,
    /// Percentage of successive NN intervals that differ by more than 50ms
    pub pnn50: f64,
}

pub fn calculate_hrv(intervals: &[u16]) -> Option<HrvMetrics> {
    if intervals.len() < 2 {
        return None;
    }

    let intervals_f64: Vec<f64> = intervals.iter().map(|&x| x as f64).collect();

    let rmssd = calculate_rmssd(&intervals_f64);
    let sdnn = calculate_sdnn(&intervals_f64);
    let pnn50 = calculate_pnn50(intervals);

    Some(HrvMetrics { rmssd, sdnn, pnn50 })
}

fn calculate_rmssd(intervals: &[f64]) -> f64 {
    if intervals.len() < 2 {
        return 0.0;
    }

    let sum_sq_diff: f64 = intervals.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();

    (sum_sq_diff / (intervals.len() - 1) as f64).sqrt()
}

fn calculate_sdnn(intervals: &[f64]) -> f64 {
    if intervals.len() < 2 {
        return 0.0;
    }

    let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
    let variance = intervals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (intervals.len() - 1) as f64;

    variance.sqrt()
}

fn calculate_pnn50(intervals: &[u16]) -> f64 {
    if intervals.len() < 2 {
        return 0.0;
    }

    let count_diff_gt_50 = intervals
        .windows(2)
        .filter(|w| (w[1] as i32 - w[0] as i32).abs() > 50)
        .count();

    (count_diff_gt_50 as f64 / (intervals.len() - 1) as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rmssd_calculation() {
        let intervals = vec![800, 820, 810, 830];
        let intervals_f64: Vec<f64> = intervals.iter().map(|&x| x as f64).collect();
        let rmssd = calculate_rmssd(&intervals_f64);
        assert!((rmssd - 17.32).abs() < 0.1);
    }

    #[test]
    fn test_sdnn_calculation() {
        let intervals = vec![800.0, 820.0, 810.0, 830.0];
        let sdnn = calculate_sdnn(&intervals);
        assert!((sdnn - 12.91).abs() < 0.1);
    }

    #[test]
    fn test_pnn50_calculation() {
        let intervals = vec![800, 820, 850, 810, 870];
        let pnn50 = calculate_pnn50(&intervals);
        assert_eq!(pnn50, 25.0);
    }

    #[test]
    fn test_hrv_metrics_calculate() {
        let intervals = vec![800, 820, 810, 830, 850];
        let metrics = calculate_hrv(&intervals).unwrap();
        assert!(metrics.rmssd > 0.0);
        assert!(metrics.sdnn > 0.0);
        assert!(metrics.pnn50 >= 0.0);
    }

    #[test]
    fn test_insufficient_data() {
        let intervals = vec![800];
        assert!(calculate_hrv(&intervals).is_none());
    }

    #[test]
    fn test_empty_data() {
        let intervals: Vec<u16> = vec![];
        assert!(calculate_hrv(&intervals).is_none());
    }
}
