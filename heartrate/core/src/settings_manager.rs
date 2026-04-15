use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Debug)]
pub struct AppSettings {
    send_port: u16,
    correction: i32,
    float_addresses: Vec<String>,
    int_addresses: Vec<String>,
    #[serde(default = "default_hrv_addresses")]
    hrv_addresses: Vec<String>,
}

fn default_hrv_addresses() -> Vec<String> {
    vec![
        "/avatar/parameters/HRV_RMSSD".to_owned(),
        "/avatar/parameters/HRV_SDNN".to_owned(),
        "/avatar/parameters/HRV_pNN50".to_owned(),
    ]
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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            send_port: 9000,
            correction: 0,
            hrv_addresses: default_hrv_addresses(),
            float_addresses: vec![
                "/avatar/parameters/Heartrate_OSC".to_owned(),
                "/avatar/parameters/Heartrate2".to_owned(),
                "/avatar/parameters/HRPercent".to_owned(),
            ],
            int_addresses: vec![
                "/avatar/parameters/HeartrateInt".to_owned(),
                "/avatar/parameters/HR".to_owned(),
            ],
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
