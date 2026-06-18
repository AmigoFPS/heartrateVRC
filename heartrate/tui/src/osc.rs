use rosc::{OscMessage, OscPacket, OscType};
use vrchat_osc::{VRChatOSC, models::OscRootNode};

use crate::{data::HeartRateData, settings::Settings};

pub async fn run_osc_loop(mut osc_rx: tokio::sync::mpsc::Receiver<HeartRateData>, settings: &Settings) {
    let vrchat_osc = match VRChatOSC::new(None).await {
        Ok(v) => v,
        Err(e) => {
            log::warn!("[OSC] Failed to initialize Manager: {}", e);
            return;
        }
    };

    let root_node = OscRootNode::new().with_avatar();
    match vrchat_osc.register("HeartRate-Service", root_node, on_osc_packet).await {
        Ok(_) => log::info!("[OSC] service registered"),
        Err(e) => log::warn!("[OSC] Failed to register client: {}", e),
    }

    // Wait for the service to be registered
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    while let Some(data) = osc_rx.recv().await {
        for addr in settings.int_addresses.iter() {
            send_osc_message(&vrchat_osc, addr, OscType::Int(data.bpm as i32)).await;
        }

        for addr in settings.float_addresses.iter() {
            send_osc_message(&vrchat_osc, addr, OscType::Float((data.bpm as f32) / 200.0)).await;
        }

        for addr in settings.rmssd_addresses.iter() {
            send_osc_message(&vrchat_osc, addr, OscType::Int(data.hrv.rmssd.round() as i32)).await;
        }

        for addr in settings.sdnn_addresses.iter() {
            send_osc_message(&vrchat_osc, addr, OscType::Int(data.hrv.sdnn.round() as i32)).await;
        }

        for addr in settings.pnn50_addresses.iter() {
            send_osc_message(&vrchat_osc, addr, OscType::Int(data.hrv.pnn50.round() as i32)).await;
        }
    }

    match vrchat_osc.shutdown().await {
        Ok(_) => log::info!("[OSC] shutting down..."),
        Err(e) => log::warn!("[OSC] Failed to shutdown: {}", e),
    };
}

fn on_osc_packet(_packet: OscPacket) {}

async fn send_osc_message(vrchat_osc: &VRChatOSC, addr: &str, value: OscType) {
    match vrchat_osc
        .send(
            OscPacket::Message(OscMessage {
                addr: addr.into(),
                args: vec![value],
            }),
            "VRChat-Client-*",
        )
        .await
    {
        Ok(_) => {}
        Err(e) => log::warn!("[OSC] Failed to send message: {}", e),
    }
}
