use std::time::Duration;

use btleplug::{
    api::{Central, Manager, Peripheral, ScanFilter},
    platform::{self, Adapter},
};
use futures::StreamExt;
use uuid::Uuid;

const HEART_RATE_MEASUREMENT_UUID: Uuid = Uuid::from_u128(0x00002A37_0000_1000_8000_00805f9b34fb);
const HEART_RATE_SERVICE_UUID: Uuid = Uuid::from_u128(0x0000180D_0000_1000_8000_00805f9b34fb);

pub struct HeartrateDevice {
    adapter: Adapter,
    target_device: Option<btleplug::platform::Peripheral>,
}

pub struct BpmData {
    pub bpm: i32,
    pub intervals: Vec<u16>,
}

#[derive(Default)]
pub struct HeartrateData {
    pub bpm: u16,
    pub intervals: Vec<u16>,
}

impl Into<BpmData> for HeartrateData {
    fn into(self) -> BpmData {
        BpmData {
            bpm: self.bpm as i32,
            intervals: self.intervals,
        }
    }
}

impl HeartrateDevice {
    pub async fn new() -> Result<Self, btleplug::Error> {
        let manager = platform::Manager::new().await?;
        let adapters = manager.adapters().await?;
        let central = adapters
            .into_iter()
            .next()
            .ok_or_else(|| btleplug::Error::Other("No Bluetooth adapters found".into()))?;

        Ok(Self {
            adapter: central,
            target_device: None,
        })
    }

    pub async fn disconnect(&mut self) -> Result<(), btleplug::Error> {
        if let Some(device) = &self.target_device {
            if device.is_connected().await? {
                device.disconnect().await?;
            }
        }
        self.target_device = None;
        Ok(())
    }

    pub async fn auto_connect(&mut self) -> Result<&btleplug::platform::Peripheral, btleplug::Error> {
        let _ = self.disconnect().await;

        self.adapter
            .start_scan(ScanFilter {
                services: vec![HEART_RATE_SERVICE_UUID],
            })
            .await?;

        tokio::time::sleep(Duration::from_secs(5)).await;

        let _ = self.adapter.stop_scan().await;

        let devices = self.adapter.peripherals().await?;
        let device = devices
            .into_iter()
            .next()
            .ok_or_else(|| btleplug::Error::DeviceNotFound)?;

        device.connect().await?;
        device.discover_services().await?;

        let chars = device.characteristics();
        let hr_char = chars
            .iter()
            .find(|c| c.uuid == HEART_RATE_MEASUREMENT_UUID)
            .ok_or_else(|| btleplug::Error::NoSuchCharacteristic)?;

        device.subscribe(hr_char).await?;

        self.target_device = Some(device.clone());

        match &self.target_device {
            Some(device) => Ok(&device),
            None => Err(btleplug::Error::DeviceNotFound),
        }
    }

    pub async fn get_bpm(&self) -> Result<BpmData, btleplug::Error> {
        let target_device = self
            .target_device
            .as_ref()
            .ok_or_else(|| btleplug::Error::DeviceNotFound)?;

        if !target_device.is_connected().await.unwrap_or(false) {
            return Err(btleplug::Error::DeviceNotFound);
        }

        match target_device.notifications().await {
            Ok(mut notifications) => {
                let value_notification = tokio::time::timeout(Duration::from_secs(3), notifications.next())
                    .await
                    .map_err(|_| btleplug::Error::DeviceNotFound)? // Timeout hit
                    .ok_or(btleplug::Error::DeviceNotFound)?;

                let data = HeartrateDevice::parse_heart_rate_full(&value_notification.value);
                Ok(data.into())
            }
            Err(e) => {
                eprintln!("Failed to get notifications: {:?}", e);
                Err(e)
            }
        }
    }

    fn parse_heart_rate_full(data: &[u8]) -> HeartrateData {
        if data.is_empty() {
            return HeartrateData::default();
        }

        let flags = data[0];
        let is_u16 = (flags & 0x01) != 0;
        let has_energy = (flags & 0x08) != 0;
        let has_rr = (flags & 0x10) != 0;

        let bpm = if is_u16 && data.len() >= 3 {
            u16::from_le_bytes([data[1], data[2]])
        } else if data.len() >= 2 {
            data[1] as u16
        } else {
            0
        };

        let rr_start = 1 + (if is_u16 { 2 } else { 1 }) + (if has_energy { 2 } else { 0 });
        let rr_intervals: Vec<u16> = if has_rr && data.len() > rr_start {
            (rr_start..data.len().saturating_sub(1))
                .step_by(2)
                .filter_map(|i| {
                    if i + 1 < data.len() {
                        Some(u16::from_le_bytes([data[i], data[i + 1]]))
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            vec![]
        };

        HeartrateData {
            bpm,
            intervals: rr_intervals,
        }
    }
}
