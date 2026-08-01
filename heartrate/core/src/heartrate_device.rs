use std::pin::Pin;
use std::time::Duration;

use btleplug::api::{Central, Characteristic, Manager as _, Peripheral as _, ScanFilter, ValueNotification};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::{Stream, StreamExt};
use uuid::Uuid;

pub const HEART_RATE_SERVICE_UUID: Uuid = Uuid::from_u128(0x0000180d_0000_1000_8000_00805f9b34fb);
pub const HEART_RATE_MEASUREMENT_UUID: Uuid = Uuid::from_u128(0x00002a37_0000_1000_8000_00805f9b34fb);

const SCAN_DURATION: Duration = Duration::from_secs(5);
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(6);

type NotificationStream = Pin<Box<dyn Stream<Item = ValueNotification> + Send>>;

#[derive(Debug)]
pub enum BleError {
    NoAdapter,
    Adapter(btleplug::Error),
    DeviceNotFound,
    NotConnected,
    ConnectionFailed(btleplug::Error),
    SubscriptionFailed(btleplug::Error),
    CharacteristicMissing,
    Timeout,
    StreamClosed,
}

impl BleError {
    pub fn is_recoverable(&self) -> bool {
        !matches!(self, BleError::NoAdapter | BleError::Adapter(_))
    }
}

impl std::fmt::Display for BleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BleError::NoAdapter => write!(f, "No Bluetooth adapter found on the system"),
            BleError::Adapter(e) => write!(f, "Bluetooth adapter error: {e}"),
            BleError::DeviceNotFound => write!(f, "No heart rate device found during scan"),
            BleError::NotConnected => write!(f, "Not connected to a heart rate device"),
            BleError::ConnectionFailed(e) => write!(f, "Failed to connect to device: {e}"),
            BleError::SubscriptionFailed(e) => write!(f, "Failed to subscribe to the HR characteristic: {e}"),
            BleError::CharacteristicMissing => {
                write!(f, "Device does not expose the Heart Rate Measurement characteristic")
            }
            BleError::Timeout => write!(f, "Heart rate transmission timed out"),
            BleError::StreamClosed => write!(f, "Notification stream closed by the driver"),
        }
    }
}

impl std::error::Error for BleError {}

#[derive(Debug, Clone, Default)]
pub struct BpmData {
    pub bpm: i32,
    pub intervals: Vec<u16>,
}

pub struct HeartrateDevice {
    adapter: Adapter,
    device: Option<Peripheral>,
    hr_char: Option<Characteristic>,
    notifications: Option<NotificationStream>,
}

impl HeartrateDevice {
    pub async fn new() -> Result<Self, BleError> {
        let manager = Manager::new().await.map_err(BleError::Adapter)?;
        let adapter = manager
            .adapters()
            .await
            .map_err(BleError::Adapter)?
            .into_iter()
            .next()
            .ok_or(BleError::NoAdapter)?;

        Ok(Self {
            adapter,
            device: None,
            hr_char: None,
            notifications: None,
        })
    }

    pub async fn auto_connect(&mut self) -> Result<String, BleError> {
        let _ = self.disconnect().await;

        let (device, name) = find_device(&self.adapter).await?;

        device.connect().await.map_err(BleError::ConnectionFailed)?;
        device.discover_services().await.map_err(BleError::SubscriptionFailed)?;

        let hr_char = device
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == HEART_RATE_MEASUREMENT_UUID)
            .ok_or(BleError::CharacteristicMissing)?;

        device.subscribe(&hr_char).await.map_err(BleError::SubscriptionFailed)?;

        let notifications = device.notifications().await.map_err(BleError::SubscriptionFailed)?;

        self.device = Some(device);
        self.hr_char = Some(hr_char);
        self.notifications = Some(notifications);

        Ok(name)
    }

    pub async fn get_bpm(&mut self) -> Result<BpmData, BleError> {
        let stream = self.notifications.as_mut().ok_or(BleError::NotConnected)?;
        let deadline = tokio::time::Instant::now() + NOTIFY_TIMEOUT;

        loop {
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(notification)) => {
                    if notification.uuid != HEART_RATE_MEASUREMENT_UUID {
                        continue;
                    }
                    match parse_heart_rate(&notification.value) {
                        Some(data) => return Ok(data),
                        None => {
                            log::debug!("[BLE] Ignoring malformed HR packet ({} bytes)", notification.value.len());
                        }
                    }
                }
                Ok(None) => return Err(BleError::StreamClosed),
                Err(_) => return Err(BleError::Timeout),
            }
        }
    }

    pub async fn disconnect(&mut self) -> Result<(), BleError> {
        self.notifications = None;
        let hr_char = self.hr_char.take();

        let Some(device) = self.device.take() else {
            return Ok(());
        };

        if let Some(hr_char) = hr_char {
            let _ = device.unsubscribe(&hr_char).await;
        }

        if device.is_connected().await.unwrap_or(false) {
            device.disconnect().await.map_err(BleError::ConnectionFailed)?;
        }

        Ok(())
    }
}

async fn find_device(adapter: &Adapter) -> Result<(Peripheral, String), BleError> {
    adapter
        .start_scan(ScanFilter {
            services: vec![HEART_RATE_SERVICE_UUID],
        })
        .await
        .map_err(BleError::Adapter)?;

    tokio::time::sleep(SCAN_DURATION).await;

    let peripherals = adapter.peripherals().await.map_err(BleError::Adapter);
    let _ = adapter.stop_scan().await;

    for peripheral in peripherals? {
        let Ok(Some(properties)) = peripheral.properties().await else {
            continue;
        };
        if !properties.services.contains(&HEART_RATE_SERVICE_UUID) {
            continue;
        }

        let name = properties
            .local_name
            .or(properties.advertisement_name)
            .unwrap_or_else(|| properties.address.to_string());

        return Ok((peripheral, name));
    }

    Err(BleError::DeviceNotFound)
}

fn parse_heart_rate(data: &[u8]) -> Option<BpmData> {
    if data.len() < 2 {
        return None;
    }

    let flags = data[0];
    let is_u16 = (flags & 0x01) != 0;
    let has_energy = (flags & 0x08) != 0;
    let has_rr = (flags & 0x10) != 0;

    let bpm = if is_u16 {
        if data.len() < 3 {
            return None;
        }
        u16::from_le_bytes([data[1], data[2]])
    } else {
        data[1] as u16
    };

    let rr_start = 1 + if is_u16 { 2 } else { 1 } + if has_energy { 2 } else { 0 };

    let mut intervals = Vec::new();
    if has_rr {
        let mut i = rr_start;
        while i + 1 < data.len() {
            intervals.push(u16::from_le_bytes([data[i], data[i + 1]]));
            i += 2;
        }
    }

    Some(BpmData {
        bpm: bpm as i32,
        intervals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_u8_bpm_without_rr() {
        let data = parse_heart_rate(&[0x00, 72]).unwrap();
        assert_eq!(data.bpm, 72);
        assert!(data.intervals.is_empty());
    }

    #[test]
    fn parses_u16_bpm() {
        let data = parse_heart_rate(&[0x01, 0x2C, 0x01]).unwrap();
        assert_eq!(data.bpm, 300);
    }

    #[test]
    fn parses_rr_intervals() {
        let data = parse_heart_rate(&[0x10, 60, 0x00, 0x04, 0x10, 0x04]).unwrap();
        assert_eq!(data.bpm, 60);
        assert_eq!(data.intervals, vec![1024, 1040]);
    }

    #[test]
    fn skips_energy_expended_field_before_rr() {
        let data = parse_heart_rate(&[0x18, 60, 0xFF, 0xFF, 0x00, 0x04]).unwrap();
        assert_eq!(data.bpm, 60);
        assert_eq!(data.intervals, vec![1024]);
    }

    #[test]
    fn rejects_truncated_packets() {
        assert!(parse_heart_rate(&[]).is_none());
        assert!(parse_heart_rate(&[0x00]).is_none());
        assert!(parse_heart_rate(&[0x01, 0x2C]).is_none());
    }

    #[test]
    fn ignores_trailing_odd_byte_in_rr_block() {
        let data = parse_heart_rate(&[0x10, 60, 0x00, 0x04, 0x99]).unwrap();
        assert_eq!(data.intervals, vec![1024]);
    }

    #[test]
    fn adapter_errors_are_not_recoverable_by_rescanning() {
        assert!(!BleError::NoAdapter.is_recoverable());
        assert!(BleError::DeviceNotFound.is_recoverable());
        assert!(BleError::Timeout.is_recoverable());
        assert!(BleError::StreamClosed.is_recoverable());
    }
}
