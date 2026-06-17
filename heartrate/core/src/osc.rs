use std::sync::Arc;

use rosc::{OscMessage, OscPacket, OscType};
use vrchat_osc::{VRChatOSC, models::OscRootNode};

use crate::hrv::HrvMetrics;

pub struct OscSender {
    vrchat_osc: Option<Arc<VRChatOSC>>,
}

impl OscSender {
    pub async fn new() -> Self {
        let vrchat_osc = match VRChatOSC::new(None).await {
            Ok(v) => v,
            Err(e) => {
                log::warn!("[OSC] Failed to initialize: {}", e);
                return Self { vrchat_osc: None };
            }
        };

        let root_node = OscRootNode::new().with_avatar();
        match vrchat_osc.register("HeartRate-Service", root_node, |_| {}).await {
            Ok(_) => log::info!("[OSC] Service registered"),
            Err(e) => log::warn!("[OSC] Failed to register service: {}", e),
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        Self { vrchat_osc: Some(vrchat_osc) }
    }

    pub async fn send_bpm(&self, bpm: i32, float_addresses: &[String], int_addresses: &[String]) {
        let Some(ref vrc) = self.vrchat_osc else { return };

        for addr in float_addresses {
            let packet = OscPacket::Message(OscMessage {
                addr: addr.clone(),
                args: vec![OscType::Float(bpm as f32 / 200.0)],
            });
            if let Err(e) = vrc.send(packet, "VRChat-Client-*").await {
                log::warn!("[OSC] Failed to send float: {}", e);
            }
        }

        for addr in int_addresses {
            let packet = OscPacket::Message(OscMessage {
                addr: addr.clone(),
                args: vec![OscType::Int(bpm)],
            });
            if let Err(e) = vrc.send(packet, "VRChat-Client-*").await {
                log::warn!("[OSC] Failed to send int: {}", e);
            }
        }
    }

    pub async fn send_hrv(&self, metrics: &HrvMetrics, addresses: &[String]) {
        let Some(ref vrc) = self.vrchat_osc else { return };

        let values = [
            (metrics.rmssd / 200.0).min(1.0),
            (metrics.sdnn / 200.0).min(1.0),
            (metrics.pnn50 / 100.0).min(1.0),
        ];

        for (i, addr) in addresses.iter().enumerate() {
            if let Some(&v) = values.get(i) {
                let packet = OscPacket::Message(OscMessage {
                    addr: addr.clone(),
                    args: vec![OscType::Float(v)],
                });
                if let Err(e) = vrc.send(packet, "VRChat-Client-*").await {
                    log::warn!("[OSC] Failed to send HRV: {}", e);
                }
            }
        }
    }

    pub async fn shutdown(self) {
        if let Some(vrc) = self.vrchat_osc {
            match vrc.shutdown().await {
                Ok(_) => log::info!("[OSC] Shutdown complete"),
                Err(e) => log::warn!("[OSC] Shutdown error: {}", e),
            }
        }
    }
}
