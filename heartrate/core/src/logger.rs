use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::hrv::HrvMetrics;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Sample {
    pub t_ms: u64,
    pub bpm: u16,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rmssd: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sdnn: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pnn50: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub artifact_pct: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionLog {
    pub started_at_unix_ms: u64,
    pub duration_ms: u64,
    #[serde(default)]
    pub device_name: String,
    pub samples: Vec<Sample>,
}

pub struct SessionLogger {
    started_at: Instant,
    started_at_unix_ms: u64,
    device_name: String,
    samples: Vec<Sample>,
}

impl SessionLogger {
    pub fn new(device_name: impl Into<String>) -> Self {
        let started_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            started_at: Instant::now(),
            started_at_unix_ms,
            device_name: device_name.into(),
            samples: Vec::new(),
        }
    }

    pub fn record(&mut self, bpm: u16, hrv: Option<&HrvMetrics>) {
        if bpm == 0 {
            return;
        }
        let t_ms = self.started_at.elapsed().as_millis() as u64;
        self.samples.push(Sample {
            t_ms,
            bpm,
            rmssd: hrv.map(|h| h.rmssd),
            sdnn: hrv.map(|h| h.sdnn),
            pnn50: hrv.map(|h| h.pnn50),
            artifact_pct: hrv.map(|h| h.artifact_pct),
        });
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn snapshot(&self) -> SessionLog {
        let duration_ms = self.samples.last().map(|s| s.t_ms).unwrap_or(0);
        SessionLog {
            started_at_unix_ms: self.started_at_unix_ms,
            duration_ms,
            device_name: self.device_name.clone(),
            samples: self.samples.clone(),
        }
    }

    pub fn save(&self, log_dir: &Path) -> io::Result<PathBuf> {
        fs::create_dir_all(log_dir)?;
        let timestamp = format_filename_timestamp(self.started_at_unix_ms);
        let filename = format!("session_{}.json", timestamp);
        let path = log_dir.join(filename);
        let json = serde_json::to_string_pretty(&self.snapshot())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(&path, json)?;
        Ok(path)
    }
}

impl SessionLog {
    pub fn load_from_file(path: &Path) -> io::Result<Self> {
        let data = fs::read_to_string(path)?;
        serde_json::from_str(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn list_in_dir(log_dir: &Path) -> io::Result<Vec<PathBuf>> {
        if !log_dir.exists() {
            return Ok(vec![]);
        }
        let mut entries: Vec<PathBuf> = fs::read_dir(log_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
        Ok(entries)
    }

    pub fn brief_stats(&self) -> BriefStats {
        let n = self.samples.len();
        if n == 0 {
            return BriefStats::default();
        }

        let mut sum_bpm: u64 = 0;
        let mut min_bpm = u16::MAX;
        let mut max_bpm: u16 = 0;
        for s in &self.samples {
            sum_bpm += s.bpm as u64;
            if s.bpm < min_bpm {
                min_bpm = s.bpm;
            }
            if s.bpm > max_bpm {
                max_bpm = s.bpm;
            }
        }
        let avg_bpm = sum_bpm as f32 / n as f32;

        BriefStats {
            duration_secs: self.duration_ms as f64 / 1000.0,
            sample_count: n,
            avg_bpm,
            min_bpm,
            max_bpm,
        }
    }

    pub fn full_stats(&self) -> FullStats {
        let brief = self.brief_stats();
        let n = self.samples.len();
        if n == 0 {
            return FullStats {
                brief,
                ..FullStats::default()
            };
        }

        let mean = brief.avg_bpm as f64;
        let variance = self
            .samples
            .iter()
            .map(|s| (s.bpm as f64 - mean).powi(2))
            .sum::<f64>()
            / n as f64;
        let bpm_stddev = variance.sqrt() as f32;

        let (rmssd_avg, rmssd_min, rmssd_max) = stat_minmax_avg(self.samples.iter().filter_map(|s| s.rmssd));
        let (sdnn_avg, sdnn_min, sdnn_max) = stat_minmax_avg(self.samples.iter().filter_map(|s| s.sdnn));
        let (pnn50_avg, pnn50_min, pnn50_max) = stat_minmax_avg(self.samples.iter().filter_map(|s| s.pnn50));
        let (artifact_avg, _, artifact_max) = stat_minmax_avg(self.samples.iter().filter_map(|s| s.artifact_pct));

        let mut zones = HrZones::default();
        for s in &self.samples {
            match s.bpm {
                0..=59 => zones.resting_count += 1,
                60..=99 => zones.light_count += 1,
                100..=139 => zones.moderate_count += 1,
                140..=169 => zones.vigorous_count += 1,
                _ => zones.maximum_count += 1,
            }
        }
        zones.total = n;

        FullStats {
            brief,
            bpm_stddev,
            rmssd_avg,
            rmssd_min,
            rmssd_max,
            sdnn_avg,
            sdnn_min,
            sdnn_max,
            pnn50_avg,
            pnn50_min,
            pnn50_max,
            artifact_avg,
            artifact_max,
            zones,
        }
    }

    pub fn export_csv(&self, path: &Path) -> io::Result<()> {
        let mut out = String::new();
        out.push_str("t_ms,bpm,rmssd,sdnn,pnn50,artifact_pct\n");
        for s in &self.samples {
            out.push_str(&format!(
                "{},{},{},{},{},{}\n",
                s.t_ms,
                s.bpm,
                s.rmssd.map_or(String::new(), |v| format!("{:.3}", v)),
                s.sdnn.map_or(String::new(), |v| format!("{:.3}", v)),
                s.pnn50.map_or(String::new(), |v| format!("{:.3}", v)),
                s.artifact_pct.map_or(String::new(), |v| format!("{:.1}", v)),
            ));
        }
        fs::write(path, out)
    }

    pub fn started_at_display(&self) -> String {
        format_display_timestamp(self.started_at_unix_ms)
    }
}

fn stat_minmax_avg<I: Iterator<Item = f32>>(iter: I) -> (Option<f32>, Option<f32>, Option<f32>) {
    let mut sum = 0.0f64;
    let mut count = 0usize;
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for v in iter {
        sum += v as f64;
        count += 1;
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    if count == 0 {
        (None, None, None)
    } else {
        (Some((sum / count as f64) as f32), Some(min), Some(max))
    }
}

#[derive(Debug, Clone, Default)]
pub struct BriefStats {
    pub duration_secs: f64,
    pub sample_count: usize,
    pub avg_bpm: f32,
    pub min_bpm: u16,
    pub max_bpm: u16,
}

#[derive(Debug, Clone, Default)]
pub struct FullStats {
    pub brief: BriefStats,
    pub bpm_stddev: f32,
    pub rmssd_avg: Option<f32>,
    pub rmssd_min: Option<f32>,
    pub rmssd_max: Option<f32>,
    pub sdnn_avg: Option<f32>,
    pub sdnn_min: Option<f32>,
    pub sdnn_max: Option<f32>,
    pub pnn50_avg: Option<f32>,
    pub pnn50_min: Option<f32>,
    pub pnn50_max: Option<f32>,
    pub artifact_avg: Option<f32>,
    pub artifact_max: Option<f32>,
    pub zones: HrZones,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HrZones {
    pub resting_count: usize,
    pub light_count: usize,
    pub moderate_count: usize,
    pub vigorous_count: usize,
    pub maximum_count: usize,
    pub total: usize,
}

impl HrZones {
    fn pct(&self, count: usize) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            count as f32 / self.total as f32 * 100.0
        }
    }
    pub fn resting_pct(&self) -> f32 {
        self.pct(self.resting_count)
    }
    pub fn light_pct(&self) -> f32 {
        self.pct(self.light_count)
    }
    pub fn moderate_pct(&self) -> f32 {
        self.pct(self.moderate_count)
    }
    pub fn vigorous_pct(&self) -> f32 {
        self.pct(self.vigorous_count)
    }
    pub fn maximum_pct(&self) -> f32 {
        self.pct(self.maximum_count)
    }
}

pub fn default_log_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.join("logs");
        }
    }
    PathBuf::from("logs")
}

fn format_filename_timestamp(unix_ms: u64) -> String {
    let (y, mo, d, h, mi, s) = unix_ms_to_ymdhms(unix_ms);
    format!("{:04}-{:02}-{:02}_{:02}-{:02}-{:02}", y, mo, d, h, mi, s)
}

fn format_display_timestamp(unix_ms: u64) -> String {
    let (y, mo, d, h, mi, s) = unix_ms_to_ymdhms(unix_ms);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", y, mo, d, h, mi, s)
}

pub fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

fn unix_ms_to_ymdhms(unix_ms: u64) -> (i32, u32, u32, u32, u32, u32) {
    let total_secs = unix_ms / 1000;
    let days = (total_secs / 86_400) as i64;
    let secs_of_day = (total_secs % 86_400) as u32;
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m, d, hour, minute, second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ymd_epoch() {
        assert_eq!(unix_ms_to_ymdhms(0), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn ymd_known_date() {
        let ms = 1_709_210_096u64 * 1000;
        assert_eq!(unix_ms_to_ymdhms(ms), (2024, 2, 29, 12, 34, 56));
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(0), "00:00");
        assert_eq!(format_duration(65_000), "01:05");
        assert_eq!(format_duration(3_725_000), "1:02:05");
    }

    #[test]
    fn brief_and_full_stats() {
        let log = SessionLog {
            started_at_unix_ms: 0,
            duration_ms: 10_000,
            device_name: "test".into(),
            samples: vec![
                Sample {
                    t_ms: 0,
                    bpm: 60,
                    rmssd: Some(40.0),
                    sdnn: Some(50.0),
                    pnn50: Some(10.0),
                    artifact_pct: Some(0.0),
                },
                Sample {
                    t_ms: 5_000,
                    bpm: 80,
                    rmssd: Some(50.0),
                    sdnn: Some(60.0),
                    pnn50: Some(20.0),
                    artifact_pct: Some(4.0),
                },
                Sample {
                    t_ms: 10_000,
                    bpm: 100,
                    rmssd: None,
                    sdnn: None,
                    pnn50: None,
                    artifact_pct: None,
                },
            ],
        };

        let brief = log.brief_stats();
        assert_eq!(brief.min_bpm, 60);
        assert_eq!(brief.max_bpm, 100);
        assert_eq!(brief.sample_count, 3);
        assert!((brief.avg_bpm - 80.0).abs() < 0.001);

        let full = log.full_stats();
        assert!((full.rmssd_avg.unwrap() - 45.0).abs() < 0.001);
        assert!((full.artifact_avg.unwrap() - 2.0).abs() < 0.001);
        assert!((full.artifact_max.unwrap() - 4.0).abs() < 0.001);
        assert_eq!(full.zones.resting_count, 0);
        assert_eq!(full.zones.light_count, 2);
        assert_eq!(full.zones.moderate_count, 1);
    }
}
