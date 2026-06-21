use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use rosc::{OscMessage, OscPacket, OscType};
use serde::Deserialize;
use tokio::net::UdpSocket;

use crate::hrv::HrvMetrics;

type OscResult<T> = std::result::Result<T, OscError>;

#[derive(Debug)]
pub struct OscError(pub String);

impl std::fmt::Display for OscError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OscError {}

impl From<std::io::Error> for OscError {
    fn from(e: std::io::Error) -> Self {
        Self(e.to_string())
    }
}

impl From<rosc::OscError> for OscError {
    fn from(e: rosc::OscError) -> Self {
        Self(e.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OscValue {
    Bool(bool),
    Float(f32),
}

#[derive(Debug, Clone)]
pub struct VrchatAddress {
    pub osc_ip: String,
    pub osc_port: u16,
    pub http_addr: SocketAddr,
}

#[derive(Debug, Deserialize)]
struct HostInfo {
    #[serde(rename = "NAME")]
    name: String,
    #[serde(rename = "OSC_IP")]
    osc_ip: String,
    #[serde(rename = "OSC_PORT")]
    osc_port: u16,
}

#[derive(Debug, Deserialize)]
struct OscQueryNode {
    #[serde(rename = "FULL_PATH")]
    full_path: Option<String>,
    #[serde(rename = "VALUE")]
    value: Option<Vec<serde_json::Value>>,
    #[serde(rename = "CONTENTS")]
    contents: Option<HashMap<String, OscQueryNode>>,
}

struct MdnsCandidate {
    http_addr: SocketAddr,
}

pub struct VrchatOscQuery {
    client: reqwest::Client,
    address: Option<VrchatAddress>,
}

impl VrchatOscQuery {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            address: None,
        }
    }

    pub fn get_address(&self) -> Option<&VrchatAddress> {
        self.address.as_ref()
    }

    pub async fn discover(&mut self) -> OscResult<bool> {
        let candidates = tokio::task::spawn_blocking(scan_mdns)
            .await
            .map_err(|e| OscError(format!("spawn_blocking failed: {e}")))?;

        for candidate in candidates {
            match self.check_candidate(&candidate).await {
                Ok(Some(addr)) => {
                    log::info!(
                        "VRChat OSCQuery found at {} (OSC {}:{})",
                        candidate.http_addr,
                        addr.osc_ip,
                        addr.osc_port
                    );
                    self.address = Some(addr);
                    return Ok(true);
                }
                Ok(None) => {}
                Err(e) => {
                    log::debug!("OSCQuery candidate {} rejected: {}", candidate.http_addr, e);
                }
            }
        }

        Ok(false)
    }

    async fn check_candidate(&self, candidate: &MdnsCandidate) -> OscResult<Option<VrchatAddress>> {
        let url = format!("http://{}/?HOST_INFO", candidate.http_addr);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| OscError(e.to_string()))?;

        let info: HostInfo = resp.json().await.map_err(|e| OscError(e.to_string()))?;

        if !info.name.starts_with("VRChat-Client-") {
            return Ok(None);
        }

        Ok(Some(VrchatAddress {
            osc_ip: info.osc_ip,
            osc_port: info.osc_port,
            http_addr: candidate.http_addr,
        }))
    }

    pub async fn get_bulk(&self) -> OscResult<Vec<(String, OscValue)>> {
        let addr = self
            .address
            .as_ref()
            .ok_or_else(|| OscError("VRChat not yet discovered".to_string()))?;

        let url = format!("http://{}/avatar/parameters", addr.http_addr);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| OscError(e.to_string()))?;

        let node: OscQueryNode = resp.json().await.map_err(|e| OscError(e.to_string()))?;

        let mut params = Vec::new();
        collect_params(&node, &mut params);
        Ok(params)
    }
}

impl Default for VrchatOscQuery {
    fn default() -> Self {
        Self::new()
    }
}

fn scan_mdns() -> Vec<MdnsCandidate> {
    let mut candidates = Vec::new();

    match try_scan_mdns(&mut candidates) {
        Ok(()) => {}
        Err(e) => log::warn!("mDNS scan failed: {}", e),
    }

    if candidates.is_empty() {
        log::debug!("mDNS found nothing; falling back to localhost:9001");
        if let Ok(addr) = "127.0.0.1:9001".parse() {
            candidates.push(MdnsCandidate { http_addr: addr });
        }
    }

    candidates
}

fn try_scan_mdns(out: &mut Vec<MdnsCandidate>) -> std::result::Result<(), String> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};

    let mdns = ServiceDaemon::new().map_err(|e| e.to_string())?;
    let receiver = mdns.browse("_oscjson._tcp.local.").map_err(|e| e.to_string())?;

    let deadline = std::time::Instant::now() + Duration::from_secs(5);

    loop {
        let now = std::time::Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline - now;
        let poll = remaining.min(Duration::from_millis(500));

        match receiver.recv_timeout(poll) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let port = info.get_port();
                for addr in info.get_addresses() {
                    use mdns_sd::ScopedIp;
                    let ip: std::net::IpAddr = match addr {
                        ScopedIp::V4(v4) => std::net::IpAddr::V4(*v4.addr()),
                        ScopedIp::V6(v6) => std::net::IpAddr::V6(*v6.addr()),
                        _ => continue,
                    };
                    let socket_addr = SocketAddr::new(ip, port);
                    log::debug!("mDNS found oscjson service at {}", socket_addr);
                    out.push(MdnsCandidate { http_addr: socket_addr });
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    mdns.stop_browse("_oscjson._tcp.local.").ok();
    Ok(())
}

fn collect_params(node: &OscQueryNode, out: &mut Vec<(String, OscValue)>) {
    if let Some(path) = &node.full_path {
        if path.starts_with("/avatar/parameters/") {
            if let Some(values) = &node.value {
                if let Some(first) = values.first() {
                    if let Some(v) = json_to_osc_value(first) {
                        out.push((path.clone(), v));
                    }
                }
            }
        }
    }
    if let Some(contents) = &node.contents {
        for child in contents.values() {
            collect_params(child, out);
        }
    }
}

pub fn json_to_osc_value(v: &serde_json::Value) -> Option<OscValue> {
    match v {
        serde_json::Value::Bool(b) => Some(OscValue::Bool(*b)),
        serde_json::Value::Number(n) => n.as_f64().map(|f| OscValue::Float(f as f32)),
        _ => None,
    }
}

pub struct OscSender {
    socket: Option<UdpSocket>,
    dest: Option<String>,
}

impl OscSender {
    pub async fn new() -> Self {
        let mut query = VrchatOscQuery::new();
        match query.discover().await {
            Ok(true) => {}
            Ok(false) => {
                log::warn!("[OSC] VRChat not found via mDNS/OSCQuery");
                return Self { socket: None, dest: None };
            }
            Err(e) => {
                log::warn!("[OSC] Discovery failed: {}", e);
                return Self { socket: None, dest: None };
            }
        }

        let Some(addr) = query.get_address() else {
            return Self { socket: None, dest: None };
        };

        let dest = format!("{}:{}", addr.osc_ip, addr.osc_port);
        match UdpSocket::bind("0.0.0.0:0").await {
            Ok(socket) => {
                log::info!("[OSC] Ready to send to {}", dest);
                Self {
                    socket: Some(socket),
                    dest: Some(dest),
                }
            }
            Err(e) => {
                log::warn!("[OSC] Failed to bind UDP socket: {}", e);
                Self { socket: None, dest: None }
            }
        }
    }

    pub async fn send_bpm(&self, bpm: i32, float_addresses: &[String], int_addresses: &[String]) {
        let Some(ref socket) = self.socket else { return };
        let Some(ref dest) = self.dest else { return };

        for addr in float_addresses {
            self.send_message(
                socket,
                dest,
                addr,
                OscType::Float(bpm as f32 / 200.0),
            )
            .await;
        }

        for addr in int_addresses {
            self.send_message(socket, dest, addr, OscType::Int(bpm)).await;
        }
    }

    pub async fn send_hrv(&self, metrics: &HrvMetrics, addresses: &[String]) {
        let Some(ref socket) = self.socket else { return };
        let Some(ref dest) = self.dest else { return };

        let values = [
            (metrics.rmssd / 200.0).min(1.0),
            (metrics.sdnn / 200.0).min(1.0),
            (metrics.pnn50 / 100.0).min(1.0),
        ];

        for (i, addr) in addresses.iter().enumerate() {
            if let Some(&v) = values.get(i) {
                self.send_message(socket, dest, addr, OscType::Float(v)).await;
            }
        }
    }

    pub async fn shutdown(self) {
        drop(self.socket);
        log::info!("[OSC] Shutdown complete");
    }

    async fn send_message(&self, socket: &UdpSocket, dest: &str, addr: &str, value: OscType) {
        let packet = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![value],
        });

        match rosc::encoder::encode(&packet) {
            Ok(buf) => {
                if let Err(e) = socket.send_to(&buf, dest).await {
                    log::warn!("[OSC] Failed to send to {}: {}", addr, e);
                }
            }
            Err(e) => log::warn!("[OSC] Failed to encode message for {}: {}", addr, e),
        }
    }
}
