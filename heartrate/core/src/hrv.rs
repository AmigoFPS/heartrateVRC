use std::collections::VecDeque;
use std::time::Instant;

use serde::{Deserialize, Serialize};

const RR_UNIT_MS: f64 = 1000.0 / 1024.0;

const HISTORY_MS: f64 = 300_000.0;
const SHORT_WINDOW_MS: f64 = 60_000.0;

const MIN_BEATS: usize = 20;
const MIN_PAIRS: usize = 10;
const MIN_SPAN_MS: f64 = 10_000.0;

const FAIR_ARTIFACT_PCT: f32 = 5.0;
const POOR_ARTIFACT_PCT: f32 = 15.0;
const UNUSABLE_ARTIFACT_PCT: f32 = 30.0;

const GAP_MS: f64 = 500.0;
const RESYNC_MS: f64 = 2_000.0;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(default)]
pub struct RrFilter {
    pub min_rr_ms: f64,
    pub max_rr_ms: f64,
    pub max_rel_change: f64,
    pub reference_beats: usize,
}

impl Default for RrFilter {
    fn default() -> Self {
        Self {
            min_rr_ms: 300.0,
            max_rr_ms: 2000.0,
            max_rel_change: 0.20,
            reference_beats: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalQuality {
    Good,
    Fair,
    Poor,
}

impl SignalQuality {
    pub fn from_artifact_pct(pct: f32) -> Self {
        if pct < FAIR_ARTIFACT_PCT {
            Self::Good
        } else if pct < POOR_ARTIFACT_PCT {
            Self::Fair
        } else {
            Self::Poor
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::Fair => "fair",
            Self::Poor => "poor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HrvMetrics {
    pub rmssd: f32,
    pub sdnn: f32,
    pub pnn50: f32,
    pub mean_hr: f32,
    pub artifact_pct: f32,
    pub quality: SignalQuality,
}

#[derive(Debug, Clone, Copy)]
struct Beat {
    t_ms: f64,
    rr_ms: f64,
    accepted: bool,
    break_before: bool,
}

pub struct HrvAnalyzer {
    beats: VecDeque<Beat>,
    reference: VecDeque<f64>,
    filter: RrFilter,
    epoch: Instant,
    clock_ms: f64,
    anchored: bool,
    pending_break: bool,
    consecutive_rejects: usize,
    poor_reported: bool,
}

impl HrvAnalyzer {
    pub fn new() -> Self {
        Self::with_filter(RrFilter::default())
    }

    pub fn with_filter(filter: RrFilter) -> Self {
        Self {
            beats: VecDeque::new(),
            reference: VecDeque::with_capacity(filter.reference_beats),
            filter,
            epoch: Instant::now(),
            clock_ms: 0.0,
            anchored: false,
            pending_break: false,
            consecutive_rejects: 0,
            poor_reported: false,
        }
    }

    pub fn add_rr_intervals(&mut self, rr_raw: &[u16]) {
        let now_ms = self.epoch.elapsed().as_secs_f64() * 1000.0;
        self.add_rr_intervals_at(rr_raw, now_ms);
    }

    pub fn add_rr_intervals_at(&mut self, rr_raw: &[u16], now_ms: f64) {
        if rr_raw.is_empty() {
            return;
        }

        let packet_ms: f64 = rr_raw.iter().map(|&v| v as f64 * RR_UNIT_MS).sum();
        let packet_start = now_ms - packet_ms;

        let drift = packet_start - self.clock_ms;
        if !self.anchored {
            self.clock_ms = packet_start;
            self.anchored = true;
        } else if drift > GAP_MS || drift < -RESYNC_MS {
            log::debug!("[HRV] {drift:.0} ms unaccounted for; not pairing across the gap");
            self.clock_ms = packet_start;
            self.pending_break = true;
            if drift.abs() > RESYNC_MS {
                self.reference.clear();
                self.consecutive_rejects = 0;
            }
        }

        for &raw in rr_raw {
            let rr_ms = raw as f64 * RR_UNIT_MS;
            self.clock_ms += rr_ms;

            let accepted = self.accepts(rr_ms);
            self.beats.push_back(Beat {
                t_ms: self.clock_ms,
                rr_ms,
                accepted,
                break_before: std::mem::take(&mut self.pending_break),
            });

            if accepted {
                self.consecutive_rejects = 0;
                self.reference.push_back(rr_ms);
                while self.reference.len() > self.filter.reference_beats {
                    self.reference.pop_front();
                }
            } else {
                self.consecutive_rejects += 1;
                if self.consecutive_rejects >= self.filter.reference_beats.max(3) {
                    self.reference.clear();
                    self.consecutive_rejects = 0;
                }
            }
        }

        self.prune();
        self.report_quality();
    }

    pub fn reset(&mut self) {
        self.beats.clear();
        self.reference.clear();
        self.anchored = false;
        self.pending_break = false;
        self.consecutive_rejects = 0;
        self.poor_reported = false;
    }

    pub fn beat_count(&self) -> usize {
        self.beats.len()
    }

    pub fn compute(&self) -> Option<HrvMetrics> {
        let short = self.short_window();
        let total = short.len();
        if total == 0 {
            return None;
        }

        let accepted: Vec<f64> = short.iter().filter(|b| b.accepted).map(|b| b.rr_ms).collect();
        if accepted.len() < MIN_BEATS {
            return None;
        }

        let span = short[total - 1].t_ms - short[0].t_ms;
        if span < MIN_SPAN_MS {
            return None;
        }

        let artifact_pct = (total - accepted.len()) as f32 / total as f32 * 100.0;
        if artifact_pct > UNUSABLE_ARTIFACT_PCT {
            return None;
        }

        let mut sum_sq_diff = 0.0;
        let mut pairs = 0usize;
        let mut nn50 = 0usize;
        for pair in short.windows(2) {
            let (prev, next) = (pair[0], pair[1]);
            if !prev.accepted || !next.accepted || next.break_before {
                continue;
            }
            let diff = next.rr_ms - prev.rr_ms;
            sum_sq_diff += diff * diff;
            pairs += 1;
            if diff.abs() > 50.0 {
                nn50 += 1;
            }
        }
        if pairs < MIN_PAIRS {
            return None;
        }

        let mean_rr = accepted.iter().sum::<f64>() / accepted.len() as f64;

        Some(HrvMetrics {
            rmssd: (sum_sq_diff / pairs as f64).sqrt() as f32,
            sdnn: self.sdnn()?,
            pnn50: nn50 as f32 / pairs as f32 * 100.0,
            mean_hr: (60_000.0 / mean_rr) as f32,
            artifact_pct,
            quality: SignalQuality::from_artifact_pct(artifact_pct),
        })
    }

    fn accepts(&self, rr_ms: f64) -> bool {
        if rr_ms < self.filter.min_rr_ms || rr_ms > self.filter.max_rr_ms {
            return false;
        }
        let Some(reference) = self.reference_rr() else {
            return true;
        };
        ((rr_ms - reference) / reference).abs() <= self.filter.max_rel_change
    }

    fn reference_rr(&self) -> Option<f64> {
        if self.reference.len() < 3 {
            return None;
        }
        let mut sorted: Vec<f64> = self.reference.iter().copied().collect();
        sorted.sort_by(|a, b| a.total_cmp(b));
        Some(sorted[sorted.len() / 2])
    }

    fn short_window(&self) -> Vec<Beat> {
        let start = self.clock_ms - SHORT_WINDOW_MS;
        self.beats.iter().filter(|b| b.t_ms >= start).copied().collect()
    }

    fn sdnn(&self) -> Option<f32> {
        let rr: Vec<f64> = self.beats.iter().filter(|b| b.accepted).map(|b| b.rr_ms).collect();
        if rr.len() < 2 {
            return None;
        }
        let mean = rr.iter().sum::<f64>() / rr.len() as f64;
        let variance = rr.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (rr.len() - 1) as f64;
        Some(variance.sqrt() as f32)
    }

    fn prune(&mut self) {
        let cutoff = self.clock_ms - HISTORY_MS;
        while self.beats.front().is_some_and(|b| b.t_ms < cutoff) {
            self.beats.pop_front();
        }
    }

    fn artifact_pct(&self) -> Option<f32> {
        let start = self.clock_ms - SHORT_WINDOW_MS;
        let mut total = 0usize;
        let mut rejected = 0usize;
        for beat in self.beats.iter().rev() {
            if beat.t_ms < start {
                break;
            }
            total += 1;
            if !beat.accepted {
                rejected += 1;
            }
        }
        (total >= MIN_BEATS).then(|| rejected as f32 / total as f32 * 100.0)
    }

    fn report_quality(&mut self) {
        let Some(pct) = self.artifact_pct() else {
            return;
        };
        let poor = pct >= POOR_ARTIFACT_PCT;
        if poor == self.poor_reported {
            return;
        }
        if poor {
            log::warn!("[HRV] {pct:.0}% of RR intervals rejected as artifacts; check the strap contact");
        } else {
            log::info!("[HRV] RR signal back to usable ({pct:.0}% intervals rejected)");
        }
        self.poor_reported = poor;
    }
}

impl Default for HrvAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(rr_ms: f64) -> u16 {
        (rr_ms / RR_UNIT_MS).round() as u16
    }

    struct Feeder {
        analyzer: HrvAnalyzer,
        now_ms: f64,
    }

    impl Feeder {
        fn new() -> Self {
            Self {
                analyzer: HrvAnalyzer::new(),
                now_ms: 0.0,
            }
        }

        fn beat(&mut self, rr_ms: f64) {
            self.now_ms += rr_ms;
            self.analyzer.add_rr_intervals_at(&[raw(rr_ms)], self.now_ms);
        }

        fn alternating(&mut self, count: usize) {
            for i in 0..count {
                self.beat(if i % 2 == 0 { 980.0 } else { 1020.0 });
            }
        }
    }

    fn quantised_alternating_diff() -> f64 {
        (raw(1020.0) as f64 - raw(980.0) as f64) * RR_UNIT_MS
    }

    #[test]
    fn clean_alternating_signal_gives_exact_rmssd() {
        let mut f = Feeder::new();
        f.alternating(40);

        let m = f.analyzer.compute().unwrap();
        let expected = quantised_alternating_diff();
        assert!((m.rmssd as f64 - expected).abs() < 0.01, "rmssd was {}", m.rmssd);
        assert_eq!(m.pnn50, 0.0);
        assert_eq!(m.artifact_pct, 0.0);
        assert_eq!(m.quality, SignalQuality::Good);
        assert!((m.mean_hr - 60.0).abs() < 0.5, "mean_hr was {}", m.mean_hr);
    }

    #[test]
    fn intervals_outside_the_physiological_range_are_rejected() {
        let mut f = Feeder::new();
        f.alternating(40);
        let clean = f.analyzer.compute().unwrap();

        f.beat(120.0);
        f.beat(2600.0);
        let dirty = f.analyzer.compute().unwrap();

        assert!((dirty.rmssd - clean.rmssd).abs() < 0.5);
        assert!(dirty.artifact_pct > 0.0);
    }

    #[test]
    fn a_sudden_jump_is_rejected_as_an_ectopic_beat() {
        let mut f = Feeder::new();
        f.alternating(40);
        let clean = f.analyzer.compute().unwrap();

        f.beat(2000.0);
        f.beat(1000.0);

        let after = f.analyzer.compute().unwrap();
        assert!(
            (after.rmssd - clean.rmssd).abs() < 1.0,
            "artifact leaked into rmssd: {} vs {}",
            after.rmssd,
            clean.rmssd
        );
    }

    #[test]
    fn differences_are_not_taken_across_a_rejected_interval() {
        let mut clean = Feeder::new();
        clean.alternating(40);

        let mut gapped = Feeder::new();
        gapped.alternating(20);
        gapped.beat(400.0);
        gapped.alternating(20);

        let a = clean.analyzer.compute().unwrap();
        let b = gapped.analyzer.compute().unwrap();
        assert!((a.rmssd - b.rmssd).abs() < 1.0, "{} vs {}", a.rmssd, b.rmssd);
    }

    #[test]
    fn too_few_beats_yield_no_metrics() {
        let mut f = Feeder::new();
        f.alternating(10);
        assert!(f.analyzer.compute().is_none());
    }

    #[test]
    fn a_mostly_broken_window_yields_no_metrics() {
        let mut f = Feeder::new();
        f.alternating(30);
        assert!(f.analyzer.compute().is_some());

        for _ in 0..40 {
            f.beat(1000.0);
            f.beat(250.0);
        }
        assert!(f.analyzer.compute().is_none(), "unusable data should not be reported");
    }

    #[test]
    fn a_sustained_rate_change_is_followed_rather_than_rejected_forever() {
        let mut f = Feeder::new();
        f.alternating(40);

        for _ in 0..150 {
            f.beat(500.0);
        }

        let m = f.analyzer.compute().unwrap();
        assert!((m.mean_hr - 120.0).abs() < 2.0, "mean_hr was {}", m.mean_hr);
        assert!(m.artifact_pct < 5.0, "artifact_pct was {}", m.artifact_pct);
    }

    #[test]
    fn history_is_pruned_to_the_retention_window() {
        let mut f = Feeder::new();
        for _ in 0..500 {
            f.beat(1000.0);
        }
        let retained = f.analyzer.beat_count();
        assert!(
            (295..=305).contains(&retained),
            "expected ~300 beats of history, got {retained}"
        );
    }

    #[test]
    fn a_lost_notification_does_not_become_a_successive_difference() {
        let mut f = Feeder::new();
        for _ in 0..25 {
            f.beat(1000.0);
        }

        f.now_ms += 1_500.0;
        for _ in 0..25 {
            f.beat(850.0);
        }

        let m = f.analyzer.compute().unwrap();
        assert!(m.rmssd < 2.0, "gap leaked into rmssd: {}", m.rmssd);
        assert_eq!(m.artifact_pct, 0.0, "the beats themselves are valid");
    }

    #[test]
    fn a_reconnect_gap_does_not_stretch_the_timeline() {
        let mut f = Feeder::new();
        f.alternating(40);

        f.now_ms += 60_000.0;
        f.alternating(40);

        let retained = f.analyzer.beat_count();
        assert!(retained <= 80, "gap beats should not be invented, got {retained}");
        let m = f.analyzer.compute().unwrap();
        assert!((m.rmssd - 40.0).abs() < 2.0, "rmssd was {}", m.rmssd);
    }

    #[test]
    fn pnn50_counts_only_differences_above_50ms() {
        let mut f = Feeder::new();
        for i in 0..40 {
            f.beat(if i % 2 == 0 { 940.0 } else { 1060.0 });
        }
        let m = f.analyzer.compute().unwrap();
        assert!(m.pnn50 > 95.0, "pnn50 was {}", m.pnn50);
    }

    #[test]
    fn quality_bands_follow_the_artifact_share() {
        assert_eq!(SignalQuality::from_artifact_pct(0.0), SignalQuality::Good);
        assert_eq!(SignalQuality::from_artifact_pct(9.0), SignalQuality::Fair);
        assert_eq!(SignalQuality::from_artifact_pct(25.0), SignalQuality::Poor);
    }
}
