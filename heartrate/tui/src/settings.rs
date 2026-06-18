use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    pub float_addresses: Vec<String>,
    pub int_addresses: Vec<String>,
    pub rmssd_addresses: Vec<String>,
    pub sdnn_addresses: Vec<String>,
    pub pnn50_addresses: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            float_addresses: vec![
                "/avatar/parameters/Heartrate_OSC".to_owned(),
                "/avatar/parameters/Heartrate2".to_owned(),
                "/avatar/parameters/HRPercent".to_owned(),
            ],
            int_addresses: vec![
                "/avatar/parameters/HeartrateInt".to_owned(),
                "/avatar/parameters/HR".to_owned(),
            ],
            rmssd_addresses: vec!["/avatar/parameters/HRV_RMSSD".to_owned()],
            sdnn_addresses: vec!["/avatar/parameters/HRV_SDNN".to_owned()],
            pnn50_addresses: vec!["/avatar/parameters/HRV_pNN50".to_owned()],
        }
    }
}

impl Settings {
    pub fn save(self, path: &str) -> Result<Self, SettingsError> {
        let data = serde_json::to_string_pretty(&self)?;
        fs::write(path, data)?;
        Ok(self)
    }

    pub fn load(mut self, path: &str) -> Result<Self, SettingsError> {
        let data = fs::read_to_string(path)?;
        let settings: Settings = serde_json::from_str(&data)?;
        self = settings.clone();
        Ok(self)
    }

    pub fn load_or_create(self, path: &str) -> Result<Self, SettingsError> {
        let clone = self.clone();
        _ = clone.load(path).or_else(|err| match err {
            SettingsError::Io(ref error) => match error.kind() {
                std::io::ErrorKind::NotFound => {
                    let settings = Settings::default().save(path)?;
                    Ok(settings)
                }
                _ => Err(err),
            },
            _ => Err(err),
        });

        Ok(self)
    }
}

#[derive(Debug)]
pub enum SettingsError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SettingsError::Io(err) => write!(f, "File System Error: {}", err),
            SettingsError::Parse(err) => write!(f, "JSON Syntax Error: {}", err),
        }
    }
}

impl From<std::io::Error> for SettingsError {
    fn from(err: std::io::Error) -> Self {
        SettingsError::Io(err)
    }
}

impl From<serde_json::Error> for SettingsError {
    fn from(err: serde_json::Error) -> Self {
        SettingsError::Parse(err)
    }
}
