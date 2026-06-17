use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::{collections::VecDeque, sync::Mutex};

pub static LOG_BUFFER: Mutex<Option<VecDeque<String>>> = Mutex::new(None);
const MAX_LOGS: usize = 50;

pub struct LocalLogger;

impl Log for LocalLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let mut buffer_guard = LOG_BUFFER.lock().unwrap();
            if buffer_guard.is_none() {
                *buffer_guard = Some(VecDeque::with_capacity(MAX_LOGS));
            }

            if let Some(ref mut queue) = *buffer_guard {
                // Format the log line cleanly
                let log_entry = format!("[{}] {}", record.level(), record.args());

                queue.push_back(log_entry);
                // Keep only the last 50 items
                if queue.len() > 50 {
                    queue.pop_front();
                }
            }
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

        if let Some(ref queue) = *buffer {
            let skip_amount = queue.len().saturating_sub(count);
            queue.iter().skip(skip_amount).cloned().collect()
        } else {
            Vec::new()
        }
    }
}
