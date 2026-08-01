use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

const CAPACITY: usize = 1000;

#[derive(Debug, Clone)]
pub struct LogLine {
    pub t_ms: u64,
    pub level: Level,
    pub message: String,
}

impl LogLine {
    pub fn timestamp(&self) -> String {
        let ms = self.t_ms % 1000;
        let secs = self.t_ms / 1000;
        format!("{:02}:{:02}.{:03}", secs / 60, secs % 60, ms)
    }
}

impl std::fmt::Display for LogLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}] {}", self.timestamp(), self.level, self.message)
    }
}

static BUFFER: OnceLock<Mutex<VecDeque<LogLine>>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();
static VERBOSE: AtomicBool = AtomicBool::new(false);
static LOGGER: BufferLogger = BufferLogger;

struct BufferLogger;

impl Log for BufferLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        push(LogLine {
            t_ms: start().elapsed().as_millis() as u64,
            level: record.level(),
            message: record.args().to_string(),
        });
    }

    fn flush(&self) {}
}

fn buffer() -> &'static Mutex<VecDeque<LogLine>> {
    BUFFER.get_or_init(|| Mutex::new(VecDeque::with_capacity(CAPACITY)))
}

fn start() -> &'static Instant {
    START.get_or_init(Instant::now)
}

fn push(line: LogLine) {
    let Ok(mut buf) = buffer().lock() else {
        return;
    };
    buf.push_back(line);
    while buf.len() > CAPACITY {
        buf.pop_front();
    }
}

pub fn init() -> Result<(), SetLoggerError> {
    let _ = start();
    log::set_max_level(level_filter(VERBOSE.load(Ordering::Relaxed)));
    log::set_logger(&LOGGER)
}

fn level_filter(verbose: bool) -> LevelFilter {
    if verbose { LevelFilter::Debug } else { LevelFilter::Info }
}

pub fn set_verbose(verbose: bool) {
    VERBOSE.store(verbose, Ordering::Relaxed);
    log::set_max_level(level_filter(verbose));
}

pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

pub fn snapshot() -> Vec<LogLine> {
    buffer().lock().map(|b| b.iter().cloned().collect()).unwrap_or_default()
}

pub fn range(start: usize, end: usize) -> Vec<LogLine> {
    let Ok(buf) = buffer().lock() else {
        return Vec::new();
    };
    let end = end.min(buf.len());
    if start >= end {
        return Vec::new();
    }
    buf.iter().skip(start).take(end - start).cloned().collect()
}

pub fn last(count: usize) -> Vec<LogLine> {
    let Ok(buf) = buffer().lock() else {
        return Vec::new();
    };
    buf.iter().skip(buf.len().saturating_sub(count)).cloned().collect()
}

pub fn len() -> usize {
    buffer().lock().map(|b| b.len()).unwrap_or(0)
}

pub fn clear() {
    if let Ok(mut buf) = buffer().lock() {
        buf.clear();
    }
}

pub fn as_text() -> String {
    snapshot().iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_formatting() {
        let line = |t_ms| LogLine {
            t_ms,
            level: Level::Info,
            message: String::new(),
        };
        assert_eq!(line(0).timestamp(), "00:00.000");
        assert_eq!(line(65_004).timestamp(), "01:05.004");
    }

    #[test]
    fn buffer_keeps_the_most_recent_records() {
        clear();
        for i in 0..CAPACITY + 10 {
            push(LogLine {
                t_ms: i as u64,
                level: Level::Info,
                message: i.to_string(),
            });
        }
        let lines = snapshot();
        assert_eq!(lines.len(), CAPACITY);
        assert_eq!(lines.first().unwrap().message, "10");
        assert_eq!(lines.last().unwrap().message, (CAPACITY + 9).to_string());
        assert_eq!(last(2).len(), 2);

        let window = range(0, 3);
        assert_eq!(window.len(), 3);
        assert_eq!(window[0].message, "10");
        assert!(range(5, 5).is_empty());
        assert_eq!(range(CAPACITY - 1, CAPACITY + 50).len(), 1);

        clear();
        assert_eq!(len(), 0);
    }
}
