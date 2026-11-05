use std::{
    collections::VecDeque,
    path::Path,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{
    heartrate_device::HeartrateDevice,
    hrv::{HrvAnalyzer, HrvMetrics},
    logger::{self, SessionLog, SessionLogger},
    osc::OscSender,
    settings_manager::AppSettings,
};

pub const MAX_HISTORY: usize = 600;

const RESET_SETTLE: Duration = Duration::from_secs(3);

const RESET_COOLDOWN: Duration = Duration::from_secs(11);

const AUTO_SAVE_MIN_SAMPLES: usize = 5000;

const ADAPTER_RETRY: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    Scanning,
    Connected(String),
    Error(String),
}

impl DeviceStatus {
    pub fn is_connected(&self) -> bool {
        matches!(self, DeviceStatus::Connected(_))
    }
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub seq:      u64,
    pub text:     String,
    pub is_error: bool,
    pub at:       Instant,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub device:         DeviceStatus,
    pub bpm:            i32,
    pub hrv:            Option<HrvMetrics>,
    pub bpm_history:    VecDeque<[f64; 2]>,
    pub rmssd_history:  VecDeque<[f64; 2]>,
    pub last_data_t:    f64,
    pub osc_connected:  bool,
    pub osc_target:     Option<String>,
    pub notice:         Option<Notice>,
    pub reset_ready_at: Option<Instant>,
    pub revision:       u64,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            device:         DeviceStatus::Scanning,
            bpm:            0,
            hrv:            None,
            bpm_history:    VecDeque::with_capacity(MAX_HISTORY),
            rmssd_history:  VecDeque::with_capacity(MAX_HISTORY),
            last_data_t:    0.0,
            osc_connected:  false,
            osc_target:     None,
            notice:         None,
            reset_ready_at: None,
            revision:       0,
        }
    }
}

impl Snapshot {
    pub fn reset_cooldown_secs(&self, now: Instant) -> f32 {
        self.reset_ready_at
            .map(|until| until.saturating_duration_since(now).as_secs_f32())
            .unwrap_or(0.0)
    }

    pub fn reset_recharge(&self, now: Instant) -> f32 {
        let left = self.reset_cooldown_secs(now);
        if left <= 0.0 {
            return 1.0;
        }
        1.0 - (left / RESET_COOLDOWN.as_secs_f32()).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    ResetHrv,
    SaveLog,
}

struct Shared {
    snapshot: RwLock<Snapshot>,
    session: Mutex<Option<SessionLogger>>,
    hrv_suppressed_until_ms: AtomicU64,
    revision: AtomicU64,
    notice_seq: AtomicU64,
    stop: AtomicBool,
    t0: Instant,
}

impl Shared {
    fn elapsed_ms(&self) -> u64 {
        self.t0.elapsed().as_millis() as u64
    }

    fn hrv_suppressed(&self) -> bool {
        self.elapsed_ms() < self.hrv_suppressed_until_ms.load(Ordering::Relaxed)
    }

    fn update(&self, f: impl FnOnce(&mut Snapshot)) {
        let mut snap = self.snapshot.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut snap);
        snap.revision = self.revision.fetch_add(1, Ordering::Relaxed) + 1;
    }

    fn read<R>(&self, f: impl FnOnce(&Snapshot) -> R) -> R {
        let snap = self.snapshot.read().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&snap)
    }

    fn say(&self, text: impl Into<String>, is_error: bool) {
        let seq = self.notice_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let notice = Notice {
            seq,
            text: text.into(),
            is_error,
            at: Instant::now(),
        };
        self.update(|s| s.notice = Some(notice));
    }

    fn with_session<R>(&self, f: impl FnOnce(&mut Option<SessionLogger>) -> R) -> R {
        let mut guard = self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard)
    }
}

pub struct Runtime {
    shared:   Arc<Shared>,
    commands: std::sync::mpsc::Sender<Command>,
    worker:   Option<std::thread::JoinHandle<()>>,
}

impl Runtime {
    pub fn start(settings: AppSettings) -> Self {
        let shared = Arc::new(Shared {
            snapshot: RwLock::new(Snapshot::default()),
            session: Mutex::new(None),
            hrv_suppressed_until_ms: AtomicU64::new(0),
            revision: AtomicU64::new(0),
            notice_seq: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            t0: Instant::now(),
        });

        let (tx, rx) = std::sync::mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        let worker = std::thread::Builder::new()
            .name("heartrate".to_owned())
            .spawn(move || run_worker_thread(worker_shared, settings, rx))
            .ok();

        if worker.is_none() {
            log::error!("[APP] Could not start the heart rate thread");
            shared.update(|s| s.device = DeviceStatus::Error("could not start the worker".into()));
        }

        Self {
            shared,
            commands: tx,
            worker,
        }
    }

    pub fn read<R>(&self, f: impl FnOnce(&Snapshot) -> R) -> R {
        self.shared.read(f)
    }

    pub fn snapshot(&self) -> Snapshot {
        self.shared.read(|s| s.clone())
    }

    pub fn revision(&self) -> u64 {
        self.shared.revision.load(Ordering::Relaxed)
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.shared.t0.elapsed().as_secs_f64()
    }

    pub fn reset_hrv(&self) {
        let now = Instant::now();
        if self.shared.read(|s| s.reset_cooldown_secs(now) > 0.0) {
            return;
        }

        self.shared
            .hrv_suppressed_until_ms
            .store(self.shared.elapsed_ms() + RESET_SETTLE.as_millis() as u64, Ordering::Relaxed);
        self.shared.update(|s| {
            s.hrv = None;
            s.rmssd_history.clear();
            s.reset_ready_at = Some(now + RESET_COOLDOWN);
        });
        let _ = self.commands.send(Command::ResetHrv);
        log::info!("[HRV] Window reset; measurements resume in {}s", RESET_SETTLE.as_secs());
    }

    pub fn save_log(&self) {
        let _ = self.commands.send(Command::SaveLog);
    }

    pub fn live_log(&self) -> Option<SessionLog> {
        self.shared.with_session(|s| s.as_ref().map(|l| l.snapshot()))
    }

    pub fn has_live_session(&self) -> bool {
        self.shared.with_session(|s| s.is_some())
    }

    pub fn request_shutdown(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
    }

    pub fn is_shutdown(&self) -> bool {
        self.shared.stop.load(Ordering::Relaxed)
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.request_shutdown();
        if let Some(worker) = self.worker.take() {
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
    }
}

fn run_worker_thread(shared: Arc<Shared>, settings: AppSettings, commands: std::sync::mpsc::Receiver<Command>) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("[APP] Could not start the Tokio runtime: {e}");
            shared.update(|s| s.device = DeviceStatus::Error(format!("runtime: {e}")));
            return;
        }
    };

    runtime.block_on(pipeline(shared, settings, commands));
}

async fn pipeline(shared: Arc<Shared>, settings: AppSettings, commands: std::sync::mpsc::Receiver<Command>) {
    let log_dir = logger::default_log_dir();
    let sender = OscSender::new(settings.send_port()).await;

    while !shared.stop.load(Ordering::Relaxed) {
        publish_osc(&shared, &sender);

        let mut device = match HeartrateDevice::new().await {
            Ok(d) => d,
            Err(e) => {
                log::error!("[BLE] Bluetooth unavailable: {e}; retrying in {}s...", ADAPTER_RETRY.as_secs());
                shared.update(|s| s.device = DeviceStatus::Error(format!("Bluetooth: {e}")));
                sleep_interruptible(&shared, ADAPTER_RETRY).await;
                continue;
            }
        };

        let mut analyzer = HrvAnalyzer::with_filter(settings.hrv_filter());
        let mut scanning = true;
        log::info!("[BLE] Scanning for a heart rate device...");
        shared.update(|s| {
            s.device = DeviceStatus::Scanning;
            s.bpm = 0;
        });

        while !shared.stop.load(Ordering::Relaxed) {
            drain_commands(&shared, &commands, &mut analyzer, &log_dir);
            publish_osc(&shared, &sender);

            if scanning {
                match device.auto_connect().await {
                    Ok(name) => {
                        log::info!("[BLE] Connected to {name}");
                        analyzer.reset();
                        shared.with_session(|s| *s = Some(SessionLogger::new(name.clone())));
                        shared.update(|s| {
                            s.device = DeviceStatus::Connected(name);
                            s.hrv = None;
                        });
                        scanning = false;
                    }
                    Err(err) => {
                        sender
                            .send_bpm(0, settings.float_addresses(), settings.int_addresses())
                            .await;
                        if err.is_recoverable() {
                            log::debug!("[BLE] {err}; continuing search...");
                            continue;
                        }
                        log::error!("[BLE] {err}; re-initializing...");
                        shared.update(|s| s.device = DeviceStatus::Error(format!("{err}")));
                        break;
                    }
                }
                continue;
            }

            match device.get_bpm().await {
                Ok(data) => {
                    let suppress = shared.hrv_suppressed();
                    if !suppress {
                        analyzer.add_rr_intervals(&data.intervals);
                    }
                    let hrv = if suppress { None } else { analyzer.compute() };

                    sender
                        .send_bpm(data.bpm, settings.float_addresses(), settings.int_addresses())
                        .await;
                    if let Some(ref metrics) = hrv {
                        sender.send_hrv(metrics, settings.hrv_addresses()).await;
                    }

                    shared.with_session(|s| {
                        if let Some(logger) = s.as_mut() {
                            logger.record(data.bpm.max(0) as u16, hrv.as_ref());
                        }
                    });

                    let t = shared.t0.elapsed().as_secs_f64();
                    shared.update(|s| {
                        s.bpm = data.bpm;
                        s.hrv = hrv;
                        s.last_data_t = t;
                        push_point(&mut s.bpm_history, [t, data.bpm as f64]);
                        if let Some(rmssd) = s.hrv.map(|m| m.rmssd as f64) {
                            push_point(&mut s.rmssd_history, [t, rmssd]);
                        }
                    });
                }
                Err(err) => {
                    sender
                        .send_bpm(0, settings.float_addresses(), settings.int_addresses())
                        .await;

                    if let Some(logger) = shared.with_session(|s| s.take()) {
                        auto_save(&shared, &logger, &log_dir);
                    }

                    if err.is_recoverable() {
                        log::warn!("[BLE] {err}; returning to discovery...");
                        shared.update(|s| {
                            s.device = DeviceStatus::Scanning;
                            s.bpm = 0;
                            s.hrv = None;
                        });
                        scanning = true;
                        continue;
                    }

                    log::error!("[BLE] {err}; re-initializing...");
                    shared.update(|s| s.device = DeviceStatus::Error(format!("{err}")));
                    break;
                }
            }
        }

        let _ = device.disconnect().await;
        if let Some(logger) = shared.with_session(|s| s.take()) {
            auto_save(&shared, &logger, &log_dir);
        }
        sleep_interruptible(&shared, Duration::from_secs(1)).await;
    }

    sender
        .send_bpm(0, settings.float_addresses(), settings.int_addresses())
        .await;
    sender.shutdown();
    log::info!("[APP] Heart rate pipeline stopped");
}

fn drain_commands(
    shared: &Arc<Shared>,
    commands: &std::sync::mpsc::Receiver<Command>,
    analyzer: &mut HrvAnalyzer,
    log_dir: &Path,
) {
    while let Ok(cmd) = commands.try_recv() {
        match cmd {
            Command::ResetHrv => analyzer.reset(),
            Command::SaveLog => save_now(shared, log_dir),
        }
    }
}

fn save_now(shared: &Arc<Shared>, log_dir: &Path) {
    let saved = shared.with_session(|s| match s.as_ref() {
        Some(logger) if !logger.is_empty() => Some(logger.save(log_dir)),
        _ => None,
    });

    match saved {
        Some(Ok(path)) => {
            log::info!("[LOG] Session saved to {}", path.display());
            shared.say(saved_message(&path), false);
        }
        Some(Err(e)) => {
            log::warn!("[LOG] Failed to save session: {e}");
            shared.say(format!("Log error: {e}"), true);
        }
        None => shared.say("Nothing recorded yet", true),
    }
}

fn auto_save(shared: &Arc<Shared>, logger: &SessionLogger, log_dir: &Path) {
    if logger.sample_count() < AUTO_SAVE_MIN_SAMPLES {
        return;
    }
    match logger.save(log_dir) {
        Ok(path) => {
            log::info!("[LOG] Session auto-saved to {}", path.display());
            shared.say(saved_message(&path), false);
        }
        Err(e) => {
            log::warn!("[LOG] Failed to auto-save session: {e}");
            shared.say(format!("Log error: {e}"), true);
        }
    }
}

fn saved_message(path: &Path) -> String {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("session");
    format!("Log saved: {name}")
}

fn publish_osc(shared: &Arc<Shared>, sender: &OscSender) {
    let address = sender.address();
    let connected = address.is_some();
    let target: Option<String> = address.map(|a| a.osc_target());

    let changed = shared.read(|s| s.osc_connected != connected || s.osc_target != target);
    if changed {
        shared.update(|s| {
            s.osc_connected = connected;
            s.osc_target = target;
        });
    }
}

fn push_point(history: &mut VecDeque<[f64; 2]>, point: [f64; 2]) {
    history.push_back(point);
    while history.len() > MAX_HISTORY {
        history.pop_front();
    }
}

async fn sleep_interruptible(shared: &Arc<Shared>, total: Duration) {
    let step = Duration::from_millis(100);
    let mut left = total;
    while left > Duration::ZERO && !shared.stop.load(Ordering::Relaxed) {
        let chunk = step.min(left);
        tokio::time::sleep(chunk).await;
        left -= chunk;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared() -> Arc<Shared> {
        Arc::new(Shared {
            snapshot: RwLock::new(Snapshot::default()),
            session: Mutex::new(None),
            hrv_suppressed_until_ms: AtomicU64::new(0),
            revision: AtomicU64::new(0),
            notice_seq: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            t0: Instant::now(),
        })
    }

    #[test]
    fn history_is_bounded() {
        let mut history = VecDeque::new();
        for i in 0..MAX_HISTORY + 50 {
            push_point(&mut history, [i as f64, 60.0]);
        }
        assert_eq!(history.len(), MAX_HISTORY);
        assert_eq!(history.front().unwrap()[0], 50.0);
    }

    #[test]
    fn every_update_bumps_the_revision() {
        let shared = shared();
        let before = shared.read(|s| s.revision);
        shared.update(|s| s.bpm = 70);
        let after = shared.read(|s| s.revision);
        assert!(after > before);
        assert_eq!(shared.read(|s| s.bpm), 70);
    }

    #[test]
    fn notices_carry_an_increasing_sequence() {
        let shared = shared();
        shared.say("first", false);
        let first = shared.read(|s| s.notice.as_ref().unwrap().seq);
        shared.say("second", true);
        let second = shared.read(|s| s.notice.as_ref().map(|n| (n.seq, n.is_error)).unwrap());
        assert_eq!(second.0, first + 1);
        assert!(second.1);
    }

    #[test]
    fn suppression_expires_on_the_runtime_clock() {
        let shared = shared();
        assert!(!shared.hrv_suppressed());
        shared
            .hrv_suppressed_until_ms
            .store(shared.elapsed_ms() + 5_000, Ordering::Relaxed);
        assert!(shared.hrv_suppressed());
        shared.hrv_suppressed_until_ms.store(0, Ordering::Relaxed);
        assert!(!shared.hrv_suppressed());
    }

    #[test]
    fn a_fresh_snapshot_reports_the_reset_control_as_ready() {
        let snap = Snapshot::default();
        let now = Instant::now();
        assert_eq!(snap.reset_cooldown_secs(now), 0.0);
        assert_eq!(snap.reset_recharge(now), 1.0);
    }

    #[test]
    fn the_reset_control_recharges_over_the_cooldown() {
        let now = Instant::now();
        let snap = Snapshot {
            reset_ready_at: Some(now + RESET_COOLDOWN),
            ..Snapshot::default()
        };
        assert!(snap.reset_cooldown_secs(now) > RESET_COOLDOWN.as_secs_f32() - 0.5);
        assert!(snap.reset_recharge(now) < 0.05);

        let halfway = now + RESET_COOLDOWN / 2;
        assert!((snap.reset_recharge(halfway) - 0.5).abs() < 0.05);

        let past = now + RESET_COOLDOWN * 2;
        assert_eq!(snap.reset_cooldown_secs(past), 0.0);
        assert_eq!(snap.reset_recharge(past), 1.0);
    }

    #[test]
    fn a_poisoned_lock_still_serves_readers() {
        let shared = shared();
        shared.update(|s| s.bpm = 55);

        let poisoner = Arc::clone(&shared);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.snapshot.write().unwrap();
            panic!("writer died holding the lock");
        })
        .join();

        assert_eq!(shared.read(|s| s.bpm), 55);
        shared.update(|s| s.bpm = 56);
        assert_eq!(shared.read(|s| s.bpm), 56);
    }
}
