use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use rosc::{OscMessage, OscPacket, OscType};
use serde::Deserialize;
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

use crate::hrv::HrvMetrics;

const MDNS_SERVICE: &str = "_oscjson._tcp.local.";
const VRCHAT_HOST_PREFIX: &str = "VRChat-Client-";

const RESCAN_INTERVAL: Duration = Duration::from_secs(5);
const HTTP_TIMEOUT: Duration = Duration::from_secs(2);
const BROWSE_POLL: Duration = Duration::from_secs(1);

const BPM_FLOAT_SCALE: f32 = 200.0;
const HRV_MS_FLOAT_SCALE: f32 = 200.0;
#[derive(Debug, Clone)]
pub struct VrchatAddress {
    pub osc_ip: String,
    pub osc_port: u16,
    pub http_addr: SocketAddr,
}

impl VrchatAddress {
    pub fn osc_target(&self) -> String {
        format!("{}:{}", self.osc_ip, self.osc_port)
    }
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

struct ClientInner {
    http: reqwest::Client,
    address: RwLock<Option<VrchatAddress>>,
    services: Mutex<HashMap<String, (Vec<IpAddr>, u16)>>,
    stop: AtomicBool,
}

pub struct VrchatClient {
    inner: Arc<ClientInner>,
    rescan_task: JoinHandle<()>,
}

impl VrchatClient {
    pub fn start() -> Self {
        let inner = Arc::new(ClientInner {
            http: reqwest::Client::builder()
                .timeout(HTTP_TIMEOUT)
                .build()
                .unwrap_or_default(),
            address: RwLock::new(None),
            services: Mutex::new(HashMap::new()),
            stop: AtomicBool::new(false),
        });

        let browse_inner = Arc::clone(&inner);
        std::thread::spawn(move || browse_worker(browse_inner));

        let rescan_inner = Arc::clone(&inner);
        let rescan_task = tokio::spawn(async move {
            loop {
                if rescan_inner.stop.load(Ordering::Relaxed) {
                    return;
                }
                rescan(&rescan_inner).await;
                tokio::time::sleep(RESCAN_INTERVAL).await;
            }
        });

        Self { inner, rescan_task }
    }

    pub fn address(&self) -> Option<VrchatAddress> {
        self.inner.address.read().ok().and_then(|a| a.clone())
    }

    pub fn shutdown(&self) {
        self.inner.stop.store(true, Ordering::Relaxed);
        self.rescan_task.abort();
    }
}

impl Drop for VrchatClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn browse_worker(inner: Arc<ClientInner>) {
    use mdns_sd::{ScopedIp, ServiceDaemon, ServiceEvent};

    let daemon = match ServiceDaemon::new() {
        Ok(d) => d,
        Err(e) => {
            log::warn!("[OSC] mDNS unavailable: {e}");
            return;
        }
    };
    let receiver = match daemon.browse(MDNS_SERVICE) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("[OSC] mDNS browse failed: {e}");
            return;
        }
    };

    while !inner.stop.load(Ordering::Relaxed) {
        match receiver.recv_timeout(BROWSE_POLL) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let addrs: Vec<IpAddr> = info
                    .get_addresses()
                    .iter()
                    .filter_map(|a| match a {
                        ScopedIp::V4(v4) => Some(IpAddr::V4(*v4.addr())),
                        ScopedIp::V6(v6) => Some(IpAddr::V6(*v6.addr())),
                        _ => None,
                    })
                    .filter(|ip| !ip.is_unspecified())
                    .collect();

                log::debug!("[OSC] mDNS resolved {} at {:?}", info.get_fullname(), addrs);
                if let Ok(mut services) = inner.services.lock() {
                    services.insert(info.get_fullname().to_string(), (addrs, info.get_port()));
                }
            }
            Ok(ServiceEvent::ServiceRemoved(_, fullname)) => {
                if let Ok(mut services) = inner.services.lock() {
                    services.remove(&fullname);
                }
            }
            Ok(_) | Err(_) => {}
        }
    }

    let _ = daemon.shutdown();
}

async fn rescan(inner: &Arc<ClientInner>) {
    if let Some(addr) = inner.address.read().ok().and_then(|a| a.clone()) {
        if check_endpoint(inner, addr.http_addr).await {
            return;
        }
        log::info!("[OSC] Lost VRChat at {}; rediscovering...", addr.http_addr);
        if let Ok(mut slot) = inner.address.write() {
            *slot = None;
        }
    }

    let candidates: Vec<(Vec<IpAddr>, u16)> = inner
        .services
        .lock()
        .map(|s| s.values().cloned().collect())
        .unwrap_or_default();
    let local = local_addresses();

    for (ips, port) in candidates {
        for ip in ips {
            if !local.contains(&ip) {
                log::debug!("[OSC] Skipping mDNS candidate {ip}:{port} (not a local address)");
                continue;
            }
            if !ip.is_loopback()
                && check_endpoint(inner, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)).await
            {
                return;
            }
            if check_endpoint(inner, SocketAddr::new(ip, port)).await {
                return;
            }
        }
    }

    if let Some(port) = oscquery_port_from_logs().await {
        log::debug!("[OSC] Found VRChat OSCQuery port {port} in log file");
        check_endpoint(inner, SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)).await;
    }
}

async fn check_endpoint(inner: &Arc<ClientInner>, http_addr: SocketAddr) -> bool {
    let url = format!("http://{http_addr}/?HOST_INFO");
    let info: HostInfo = match inner.http.get(&url).send().await {
        Ok(resp) => match resp.json().await {
            Ok(json) => json,
            Err(_) => return false,
        },
        Err(_) => return false,
    };

    if !info.name.starts_with(VRCHAT_HOST_PREFIX) {
        log::debug!("[OSC] Skipping {http_addr} ({} is not VRChat)", info.name);
        return false;
    }

    let is_new = inner
        .address
        .read()
        .ok()
        .and_then(|a| a.clone())
        .is_none_or(|a| a.http_addr != http_addr || a.osc_port != info.osc_port);

    if is_new {
        log::info!(
            "[OSC] VRChat found at {http_addr} (OSC {}:{})",
            info.osc_ip,
            info.osc_port
        );
    }

    if let Ok(mut slot) = inner.address.write() {
        *slot = Some(VrchatAddress {
            osc_ip: info.osc_ip,
            osc_port: info.osc_port,
            http_addr,
        });
    }
    true
}

fn local_addresses() -> Vec<IpAddr> {
    let mut out = vec![IpAddr::V4(Ipv4Addr::LOCALHOST), IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)];
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        out.extend(ifaces.into_iter().map(|i| i.ip()));
    }
    out
}

fn vrchat_log_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(
        PathBuf::from(home)
            .join("AppData")
            .join("LocalLow")
            .join("VRChat")
            .join("VRChat"),
    )
}

async fn oscquery_port_from_logs() -> Option<u16> {
    tokio::task::spawn_blocking(|| {
        let dir = vrchat_log_dir()?;
        let newest = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("output_log_"))
            .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())?;

        let content = std::fs::read_to_string(newest.path()).ok()?;
        parse_oscquery_port(&content)
    })
    .await
    .ok()
    .flatten()
}

fn parse_oscquery_port(content: &str) -> Option<u16> {
    const MARKER: &str = "of type OSCQuery on ";
    let mut result = None;
    let mut rest = content;
    while let Some(pos) = rest.find(MARKER) {
        let after = &rest[pos + MARKER.len()..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(port) = digits.parse::<u16>() {
            result = Some(port);
        }
        rest = &after[digits.len()..];
    }
    result
}

pub struct OscSender {
    socket: Option<UdpSocket>,
    client: VrchatClient,
    fallback: String,
    on_fallback: AtomicBool,
}

impl OscSender {
    pub async fn new(fallback_port: u16) -> Self {
        let socket = match UdpSocket::bind("0.0.0.0:0").await {
            Ok(s) => Some(s),
            Err(e) => {
                log::error!("[OSC] Failed to bind UDP socket: {e}; OSC output disabled");
                None
            }
        };

        Self {
            socket,
            client: VrchatClient::start(),
            fallback: format!("127.0.0.1:{fallback_port}"),
            on_fallback: AtomicBool::new(false),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.client.address().is_some()
    }

    pub fn address(&self) -> Option<VrchatAddress> {
        self.client.address()
    }

    pub async fn send_bpm(&self, bpm: i32, float_addresses: &[String], int_addresses: &[String]) {
        let Some(target) = self.target() else { return };

        for addr in float_addresses {
            self.send_message(&target, addr, OscType::Float(bpm as f32 / BPM_FLOAT_SCALE))
                .await;
        }

        for addr in int_addresses {
            self.send_message(&target, addr, OscType::Int(bpm)).await;
        }
    }

    pub async fn send_hrv(&self, metrics: &HrvMetrics, addresses: &[String]) {
        let Some(target) = self.target() else { return };

        let values = normalize_hrv(metrics);
        for (addr, value) in addresses.iter().zip(values) {
            self.send_message(&target, addr, OscType::Float(value)).await;
        }
    }

    pub fn shutdown(self) {
        self.client.shutdown();
        drop(self.socket);
        log::info!("[OSC] Shutdown complete");
    }

    fn target(&self) -> Option<String> {
        self.socket.as_ref()?;

        match self.client.address() {
            Some(addr) => {
                if self.on_fallback.swap(false, Ordering::Relaxed) {
                    log::info!("[OSC] Now sending to discovered VRChat at {}", addr.osc_target());
                }
                Some(addr.osc_target())
            }
            None => {
                if !self.on_fallback.swap(true, Ordering::Relaxed) {
                    log::warn!(
                        "[OSC] VRChat not discovered yet; sending to default {}",
                        self.fallback
                    );
                }
                Some(self.fallback.clone())
            }
        }
    }

    async fn send_message(&self, target: &str, addr: &str, value: OscType) {
        let Some(ref socket) = self.socket else { return };

        let packet = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args: vec![value],
        });

        match rosc::encoder::encode(&packet) {
            Ok(buf) => {
                if let Err(e) = socket.send_to(&buf, target).await {
                    log::warn!("[OSC] Failed to send {addr} to {target}: {e}");
                }
            }
            Err(e) => log::warn!("[OSC] Failed to encode message for {addr}: {e}"),
        }
    }
}

fn normalize_hrv(metrics: &HrvMetrics) -> [f32; 4] {
    [
        (metrics.rmssd / HRV_MS_FLOAT_SCALE).clamp(0.0, 1.0),
        (metrics.sdnn / HRV_MS_FLOAT_SCALE).clamp(0.0, 1.0),
        (metrics.pnn50 / 100.0).clamp(0.0, 1.0),
        (1.0 - metrics.artifact_pct / 100.0).clamp(0.0, 1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_prefix_matches_vrchat_only() {
        assert!("VRChat-Client-35464D".starts_with(VRCHAT_HOST_PREFIX));
        assert!("VRChat-Client-".starts_with(VRCHAT_HOST_PREFIX));
        assert!(!"OyasumiVR".starts_with(VRCHAT_HOST_PREFIX));
        assert!(!"HeartRate-Service".starts_with(VRCHAT_HOST_PREFIX));
    }

    #[test]
    fn log_port_parsing_takes_last_match() {
        let log = "junk\nBlah of type OSCQuery on 9012 something\nof type OSCQuery on 34567\n";
        assert_eq!(parse_oscquery_port(log), Some(34567));
        assert_eq!(parse_oscquery_port("no match"), None);
    }

    #[test]
    fn hrv_normalization_is_clamped() {
        let metrics = HrvMetrics {
            rmssd: 100.0,
            sdnn: 400.0,
            pnn50: 50.0,
            mean_hr: 60.0,
            artifact_pct: 10.0,
            quality: crate::hrv::SignalQuality::Fair,
        };
        assert_eq!(normalize_hrv(&metrics), [0.5, 1.0, 0.5, 0.9]);
    }

    #[test]
    fn hrv_normalization_handles_zero() {
        let metrics = HrvMetrics {
            rmssd: 0.0,
            sdnn: 0.0,
            pnn50: 0.0,
            mean_hr: 0.0,
            artifact_pct: 0.0,
            quality: crate::hrv::SignalQuality::Good,
        };
        assert_eq!(normalize_hrv(&metrics), [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn local_addresses_include_loopback() {
        assert!(local_addresses().contains(&IpAddr::V4(Ipv4Addr::LOCALHOST)));
    }
}
