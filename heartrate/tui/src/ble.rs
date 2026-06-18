use std::time::Duration;

use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use tokio::time::sleep;
use uuid::Uuid;

use crate::data::HeartRateData;
use crate::hrv::{HrvMetrics, calculate_hrv};

// UUID for the standard Bluetooth Heart Rate Service
pub const HEART_RATE_SERVICE_UUID: Uuid = Uuid::from_u128(0x0000180d_0000_1000_8000_00805f9b34fb);
// UUID for the Heart Rate Measurement Characteristic
pub const HEART_RATE_MEASUREMENT_UUID: Uuid = Uuid::from_u128(0x00002a37_0000_1000_8000_00805f9b34fb);

#[derive(Debug)]
pub enum BleError {
    NoAdapterFound,
    AdapterError(btleplug::Error),
    DeviceNotFound,
    ConnectionFailed(btleplug::Error),
    SubscriptionFailed(btleplug::Error),
}

impl std::fmt::Display for BleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BleError::NoAdapterFound => write!(f, "No Bluetooth adapters found on the system."),
            BleError::AdapterError(e) => write!(f, "Bluetooth adapter error: {}", e),
            BleError::DeviceNotFound => write!(f, "Heart Rate Device not found during scan."),
            BleError::ConnectionFailed(e) => write!(f, "Failed to connect to device: {}", e),
            BleError::SubscriptionFailed(e) => write!(f, "Failed to subscribe to HR characteristic: {}", e),
        }
    }
}

impl std::error::Error for BleError {}

pub async fn run_ble_loop(
    osc_tx: tokio::sync::mpsc::Sender<HeartRateData>,
    tui_tx: tokio::sync::watch::Sender<HeartRateData>,
) {
    let manager = match Manager::new().await {
        Ok(m) => m,
        Err(e) => {
            log::error!("Failed to initialize BLE Manager: {}", e);
            return;
        }
    };

    loop {
        log::info!("[BLE]: Searching for Bluetooth Adapter...");
        let adapter = match get_adapter(&manager).await {
            Ok(a) => a,
            Err(e) => {
                log::warn!("[BLE] {}; retrying in 5s...", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        log::info!("[BLE]: Adapter found. Scanning for HR Device...");
        let device = match find_hr_device(&adapter).await {
            Ok(d) => d,
            Err(e) => {
                log::warn!("[BLE] {}; restarting scan in 5s...", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        log::info!("[BLE]: HR Device isolated. Attempting connection...");
        if let Err(e) = device.connect().await {
            log::warn!("[BLE] {}; returning to discovery...", BleError::ConnectionFailed(e));
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        log::info!("[BLE]: Connected! Discovering services and subscribing...");
        if let Err(e) = handle_device_stream(&device, &osc_tx, &tui_tx).await {
            log::warn!("[BLE] Disconnected or Stream broken: {}. Re-initializing...", e);
        }

        // Clean up or ensure disconnection before restarting the loop
        let _ = device.disconnect().await;
        sleep(Duration::from_secs(2)).await;
    }
}

async fn get_adapter(manager: &Manager) -> Result<Adapter, BleError> {
    let adapters = manager.adapters().await.map_err(BleError::AdapterError)?;
    adapters.into_iter().next().ok_or(BleError::NoAdapterFound)
}

async fn find_hr_device(adapter: &Adapter) -> Result<Peripheral, BleError> {
    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(BleError::AdapterError)?;

    sleep(Duration::from_secs(5)).await;

    let peripherals = adapter.peripherals().await.map_err(BleError::AdapterError)?;

    for peripheral in peripherals {
        if let Ok(Some(properties)) = peripheral.properties().await {
            if properties.services.contains(&HEART_RATE_SERVICE_UUID) {
                let _ = adapter.stop_scan().await; // Clean up scanning state
                return Ok(peripheral);
            }
        }
    }

    let _ = adapter.stop_scan().await;
    Err(BleError::DeviceNotFound)
}

async fn handle_device_stream(
    device: &Peripheral,
    osc_tx: &tokio::sync::mpsc::Sender<HeartRateData>,
    tui_tx: &tokio::sync::watch::Sender<HeartRateData>,
) -> Result<(), BleError> {
    device.discover_services().await.map_err(BleError::SubscriptionFailed)?;

    let characteristics = device.characteristics();
    let hr_char = characteristics
        .iter()
        .find(|c| c.uuid == HEART_RATE_MEASUREMENT_UUID)
        .ok_or_else(|| BleError::SubscriptionFailed(btleplug::Error::Other("HR characteristic missing".into())))?;

    device.subscribe(hr_char).await.map_err(BleError::SubscriptionFailed)?;
    let mut notification_stream = device.notifications().await.map_err(BleError::SubscriptionFailed)?;

    loop {
        // If no notification arrives within 6 seconds, assume a quiet disconnect/timeout
        match tokio::time::timeout(Duration::from_secs(6), notification_stream.next()).await {
            Ok(Some(notification)) => {
                if notification.uuid == HEART_RATE_MEASUREMENT_UUID {
                    if let Some(hr_data) = parse_heart_rate(&notification.value) {
                        // Forward safely to OSC and TUI
                        let _ = osc_tx.send(hr_data.clone()).await;
                        let _ = tui_tx.send(hr_data);
                    }
                }
            }
            Ok(None) => {
                // Stream closed natively by the driver
                return Err(BleError::ConnectionFailed(btleplug::Error::Other("Stream closed".into())));
            }
            Err(_) => {
                // Timeout elapsed without a beat notification
                return Err(BleError::ConnectionFailed(btleplug::Error::Other(
                    "Heart rate transmission timed out".into(),
                )));
            }
        }
    }
}

fn parse_heart_rate(bytes: &[u8]) -> Option<HeartRateData> {
    if bytes.len() < 2 {
        return None;
    }

    let flags = bytes[0];
    let is_u16 = (flags & 0x01) == 0x01;
    let has_energy = (flags & 0x08) != 0;
    let has_rr = (flags & 0x10) != 0;

    let bpm = if is_u16 && bytes.len() >= 3 {
        u16::from_le_bytes([bytes[1], bytes[2]])
    } else if bytes.len() >= 2 {
        bytes[1] as u16
    } else {
        0
    };

    let mut rr_start = 1;
    rr_start += match is_u16 {
        true => 2,
        false => 1,
    };
    rr_start += match has_energy {
        true => 2,
        false => 0,
    };

    let mut rr_intervals: Vec<u16> = vec![];
    if has_rr && bytes.len() > rr_start {
        rr_intervals = (rr_start..bytes.len().saturating_sub(1))
            .step_by(2)
            .filter_map(|i| match i + 1 < bytes.len() {
                true => Some(u16::from_le_bytes([bytes[i], bytes[i + 1]])),
                false => None,
            })
            .collect();
    }

    let hrv = calculate_hrv(&rr_intervals).unwrap_or(HrvMetrics::default());

    // TODO: Retreive real data instead of mock data
    Some(HeartRateData { bpm, hrv, battery: 100 })
}
