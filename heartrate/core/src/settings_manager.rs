use serde::{Deserialize, Serialize};
use std::fs;

use crate::hrv::RrFilter;

#[derive(Serialize, Deserialize, Debug)]
pub struct AppSettings {
    send_port: u16,
    correction: i32,
    float_addresses: Vec<String>,
    int_addresses: Vec<String>,
    hrv_addresses: Vec<String>,
    #[serde(default)]
    hrv_filter: RrFilter,
}

impl AppSettings {
    pub fn send_port(&self) -> u16 {
        self.send_port
    }

    pub fn correction(&self) -> i32 {
        self.correction
    }

    pub fn float_addresses(&self) -> &[String] {
        self.float_addresses.as_slice()
    }

    pub fn int_addresses(&self) -> &[String] {
        self.int_addresses.as_slice()
    }

    pub fn hrv_addresses(&self) -> &[String] {
        self.hrv_addresses.as_slice()
    }

    pub fn hrv_filter(&self) -> RrFilter {
        self.hrv_filter
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            send_port: 9000,
            correction: 0,
            float_addresses: vec![
                "/avatar/parameters/Heartrate_OSC".to_owned(),
                "/avatar/parameters/Heartrate2".to_owned(),
                "/avatar/parameters/HRPercent".to_owned(),
            ],
            int_addresses: vec![
                "/avatar/parameters/HeartrateInt".to_owned(),
                "/avatar/parameters/HR".to_owned(),
            ],
            hrv_addresses: vec![
                "/avatar/parameters/HRV_RMSSD".to_owned(),
                "/avatar/parameters/HRV_SDNN".to_owned(),
                "/avatar/parameters/HRV_pNN50".to_owned(),
                "/avatar/parameters/HRV_Quality".to_owned(),
            ],
            hrv_filter: RrFilter::default(),
        }
    }
}

impl AppSettings {
    pub fn try_load_from_file(path: &str) -> Result<Self, AppSetttingsError> {
        Self::load_from_file(path).or_else(|err| match err {
            AppSetttingsError::Io(ref io_err) => match io_err.kind() {
                std::io::ErrorKind::NotFound => {
                    let settings = AppSettings::default();
                    settings.save_to_file(path)?;
                    Ok(settings)
                }
                _ => Err(err),
            },
            _ => Err(err),
        })
    }

    pub fn load_from_file(path: &str) -> Result<Self, AppSetttingsError> {
        let data = fs::read_to_string(path)?;
        let config = serde_json::from_str(&data)?;
        Ok(config)
    }

    pub fn save_to_file(&self, path: &str) -> Result<(), AppSetttingsError> {
        let data = serde_json::to_string_pretty(self)?;
        fs::write(path, data)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum AppSetttingsError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for AppSetttingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AppSetttingsError::Io(err) => write!(f, "File System Error: {}", err),
            AppSetttingsError::Parse(err) => write!(f, "JSON Syntax Error: {}", err),
        }
    }
}

impl From<std::io::Error> for AppSetttingsError {
    fn from(err: std::io::Error) -> Self {
        AppSetttingsError::Io(err)
    }
}

impl From<serde_json::Error> for AppSetttingsError {
    fn from(err: serde_json::Error) -> Self {
        AppSetttingsError::Parse(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_without_a_filter_block_fall_back_to_the_defaults() {
        let json = r#"{
            "send_port": 9000,
            "correction": 20,
            "float_addresses": ["/avatar/parameters/Heartrate_OSC"],
            "int_addresses": ["/avatar/parameters/HR"],
            "hrv_addresses": [
                "/avatar/parameters/HRV_RMSSD",
                "/avatar/parameters/HRV_SDNN",
                "/avatar/parameters/HRV_pNN50"
            ]
        }"#;

        let settings: AppSettings = serde_json::from_str(json).expect("legacy config should still parse");
        let filter = settings.hrv_filter();
        assert_eq!(filter.max_rel_change, RrFilter::default().max_rel_change);
        assert_eq!(settings.hrv_addresses().len(), 3);
    }

    #[test]
    fn a_partial_filter_block_keeps_the_remaining_defaults() {
        let json = r#"{
            "send_port": 9000,
            "correction": 0,
            "float_addresses": [],
            "int_addresses": [],
            "hrv_addresses": [],
            "hrv_filter": { "max_rel_change": 0.25 }
        }"#;

        let settings: AppSettings = serde_json::from_str(json).expect("partial filter block should parse");
        assert_eq!(settings.hrv_filter().max_rel_change, 0.25);
        assert_eq!(settings.hrv_filter().min_rr_ms, RrFilter::default().min_rr_ms);
    }
}
