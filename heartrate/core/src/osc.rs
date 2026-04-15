use std::net::{SocketAddrV4, UdpSocket};

use rosc::{OscMessage, OscPacket, OscType};

use crate::hrv::HrvMetrics;

pub struct OscSender {
    socket: UdpSocket,
    target_addr: SocketAddrV4,
}

impl OscSender {
    pub fn new(host: [u8; 4], port: u16) -> Self {
        let socket = UdpSocket::bind("0.0.0.0:0").expect("Couldn't bind to UDP socket");
        let target_addr = SocketAddrV4::new(host.into(), port);
        Self { socket, target_addr }
    }

    pub fn send_bpm(
        &self,
        bpm: i32,
        float_addresses: &[String],
        int_addresses: &[String],
    ) -> Result<(), rosc::OscError> {
        float_addresses
            .iter()
            .try_for_each(|address| -> Result<(), rosc::OscError> {
                let msg = OscMessage {
                    addr: address.to_string(),
                    args: vec![OscType::Float(bpm as f32 / 200.0)],
                };

                let packet = OscPacket::Message(msg);
                let bytes = rosc::encoder::encode(&packet)?;
                _ = self
                    .socket
                    .send_to(&bytes, self.target_addr)
                    .map_err(|_| rosc::OscError::BadAddress);

                Ok(())
            })?;

        int_addresses
            .iter()
            .try_for_each(|address| -> Result<(), rosc::OscError> {
                let msg = OscMessage {
                    addr: address.to_string(),
                    args: vec![OscType::Int(bpm)],
                };

                let packet = OscPacket::Message(msg);
                let bytes = rosc::encoder::encode(&packet)?;
                _ = self
                    .socket
                    .send_to(&bytes, self.target_addr)
                    .map_err(|_| rosc::OscError::BadAddress);

                Ok(())
            })?;

        Ok(())
    }

    pub fn send_hrv(
        &self,
        metrics: &HrvMetrics,
        addresses: &[String],
    ) -> Result<(), rosc::OscError> {
        let values = [
            (metrics.rmssd / 200.0).min(1.0),
            (metrics.sdnn / 200.0).min(1.0),
            (metrics.pnn50 / 100.0).min(1.0),
        ];
        for (i, address) in addresses.iter().enumerate() {
            if let Some(&v) = values.get(i) {
                let msg = OscMessage {
                    addr: address.to_string(),
                    args: vec![OscType::Float(v)],
                };
                let packet = OscPacket::Message(msg);
                let bytes = rosc::encoder::encode(&packet)?;
                _ = self
                    .socket
                    .send_to(&bytes, self.target_addr)
                    .map_err(|_| rosc::OscError::BadAddress);
            }
        }
        Ok(())
    }
}
