use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::sync::Mutex;

static LOG_BUFFER: Mutex<Vec<String>> = Mutex::new(Vec::new());
const MAX_LOGS: usize = 50;

pub struct LocalLogger;

impl Log for LocalLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let mut buffer = LOG_BUFFER.lock().unwrap();
            if buffer.len() >= MAX_LOGS {
                buffer.remove(0);
            }
            buffer.push(format!("[{}] {}", record.level(), record.args()));
        }
    }

    fn flush(&self) {}
}

impl LocalLogger {
    pub fn init() -> Result<(), SetLoggerError> {
        log::set_max_level(LevelFilter::Info);
        log::set_logger(&LocalLogger)
    }

    pub fn get_last_lines(count: usize) -> Vec<String> {
        let buffer = LOG_BUFFER.lock().unwrap();
        buffer
            .iter()
            .cloned()
            .rev()
            .take(count)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}
