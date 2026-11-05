use std::{
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use heartrate_core::runtime::Runtime;

use crate::{
    context::{AppKind, VrContext},
    error::VrError,
    manifest::{self, DASHBOARD_OVERLAY_KEY, DISPLAY_NAME},
    overlay::OverlayHandle,
    panel::{self, Input, Outcome, Panel},
    render::{self, PanelRenderer, gpu::GpuTexture},
    sys,
};

const ICON_SIZE: u32 = 128;

const POLL_HZ_VISIBLE: f32 = 60.0;
const POLL_HZ_HIDDEN: f32 = 6.0;

const MIN_REDRAW_INTERVAL: Duration = Duration::from_millis(50);
const REFRESH_INTERVAL: Duration = Duration::from_millis(1000);

const LIVE_REFRESH_INTERVAL: Duration = Duration::from_millis(250);

const RECONNECT_INTERVAL: Duration = Duration::from_secs(5);
const TAB_RETRY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DashboardStatus {
    pub connected:       bool,
    pub runtime_version: String,
    pub tab_ready:       bool,
    pub registered:      bool,
    pub auto_launch:     bool,
    pub error:           Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DashCommand {
    SetAutoLaunch(bool),
}

pub struct Dashboard {
    status:       Arc<RwLock<DashboardStatus>>,
    commands:     Arc<Mutex<Vec<DashCommand>>>,
    stop:         Arc<AtomicBool>,
    steamvr_quit: Arc<AtomicBool>,
    thread:       Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Dashboard {
    pub fn start(runtime: Arc<Runtime>, exit_with_steamvr: bool) -> Self {
        let status = Arc::new(RwLock::new(DashboardStatus::default()));
        let commands = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let steamvr_quit = Arc::new(AtomicBool::new(false));

        let thread = {
            let state = ThreadState {
                runtime,
                status: Arc::clone(&status),
                commands: Arc::clone(&commands),
                stop: Arc::clone(&stop),
                steamvr_quit: Arc::clone(&steamvr_quit),
                exit_with_steamvr,
            };
            std::thread::Builder::new()
                .name("steamvr".to_owned())
                .spawn(move || run(state))
                .ok()
        };

        if thread.is_none() {
            log::error!("[VR] Could not start the SteamVR thread; the dashboard tab is unavailable");
            set_error(&status, "could not start the SteamVR thread".to_owned());
        }

        Self {
            status,
            commands,
            stop,
            steamvr_quit,
            thread: Mutex::new(thread),
        }
    }

    pub fn set_auto_launch(&self, on: bool) {
        self.push(DashCommand::SetAutoLaunch(on));
    }

    fn push(&self, command: DashCommand) {
        let mut queue = self.commands.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if !queue.contains(&command) {
            queue.push(command);
        }
    }

    pub fn status(&self) -> DashboardStatus {
        self.status
            .read()
            .map(|s| s.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    pub fn should_exit(&self) -> bool {
        self.steamvr_quit.load(Ordering::Relaxed)
    }

    pub fn request_shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub fn shutdown(&self, timeout: Duration) {
        self.request_shutdown();

        let taken = self
            .thread
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        let Some(thread) = taken else {
            return;
        };

        let deadline = Instant::now() + timeout;
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        if thread.is_finished() {
            let _ = thread.join();
        } else {
            log::warn!("[VR] The SteamVR thread did not stop in time; leaving it behind");
        }
    }
}

impl Drop for Dashboard {
    fn drop(&mut self) {
        self.shutdown(Duration::from_secs(2));
    }
}

struct ThreadState {
    runtime:           Arc<Runtime>,
    status:            Arc<RwLock<DashboardStatus>>,
    commands:          Arc<Mutex<Vec<DashCommand>>>,
    stop:              Arc<AtomicBool>,
    steamvr_quit:      Arc<AtomicBool>,
    exit_with_steamvr: bool,
}

impl ThreadState {
    fn is_shutdown(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn take_commands(&self) -> Vec<DashCommand> {
        let mut queue = self.commands.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        std::mem::take(&mut *queue)
    }

    fn update_status(&self, f: impl FnOnce(&mut DashboardStatus)) {
        let mut guard = self.status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut guard);
    }
}

fn set_error(status: &Arc<RwLock<DashboardStatus>>, message: String) {
    let mut guard = status.write().unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.error = Some(message);
}

enum SessionEnd {
    Quit,
    Stopped,
    Restart,
}

fn run(state: ThreadState) {
    log::info!("[VR] SteamVR thread started");

    let mut renderer = match PanelRenderer::new(panel::DESIGN_WIDTH, panel::DESIGN_HEIGHT) {
        Ok(r) => {
            log::debug!("[VR] Panel font: {}", r.font_path());
            r
        }
        Err(e) => {
            log::error!("[VR] {e}; the dashboard tab is unavailable");
            set_error(&state.status, format!("{e}"));
            return;
        }
    };

    let mut announced_failure = false;

    while !state.is_shutdown() {
        match Session::start(&state) {
            Ok(mut session) => {
                announced_failure = false;
                state.update_status(|s| s.error = None);

                match session.run(&state, &mut renderer) {
                    Ok(SessionEnd::Quit) => {
                        if state.exit_with_steamvr {
                            log::info!("[VR] SteamVR is shutting down; exiting with it");
                            state.steamvr_quit.store(true, Ordering::Relaxed);
                        } else {
                            log::info!("[VR] SteamVR is shutting down; heart rate output continues");
                        }
                    }
                    Ok(SessionEnd::Stopped) => {}
                    Ok(SessionEnd::Restart) => {
                        log::info!("[VR] Session ended; waiting for SteamVR to come back");
                    }
                    Err(e) => {
                        log::warn!("[VR] Session ended with an error: {e}");
                        set_error(&state.status, format!("{e}"));
                    }
                }
            }
            Err(e) => {
                if !announced_failure {
                    log::info!("[VR] Not connected to SteamVR: {e}");
                    set_error(&state.status, format!("{e}"));
                    announced_failure = true;
                } else {
                    log::debug!("[VR] Still cannot connect to SteamVR: {e}");
                }
            }
        }

        state.update_status(|s| {
            s.connected = false;
            s.tab_ready = false;
        });

        if state.is_shutdown() || state.steamvr_quit.load(Ordering::Relaxed) {
            break;
        }
        sleep_interruptible(&state, RECONNECT_INTERVAL);
    }

    log::info!("[VR] SteamVR thread stopped");
}

struct Session {
    ctx:          VrContext,
    tab:          Option<Tab>,
    tab_retry_at: Instant,
}

struct Tab {
    handle:              OverlayHandle,
    thumbnail:           OverlayHandle,
    panel:               Panel,
    gpu:                 Option<GpuTexture>,
    uploaded:            Option<u64>,
    visible:             bool,
    pending:             Vec<Input>,
    next_redraw_allowed: Instant,
    refresh_at:          Instant,
    live_revision:       u64,
    live_refresh_at:     Instant,
}

const INPUT_BACKLOG: usize = 256;

impl Session {
    fn start(state: &ThreadState) -> Result<Self, VrError> {
        let ctx = VrContext::init(AppKind::Overlay)?;

        let registration = match ctx.applications.as_ref() {
            Some(apps) => announce(apps),
            None => manifest::Registration::default(),
        };

        let runtime_version = ctx.runtime_version().to_owned();
        state.update_status(|s| {
            s.connected = true;
            s.runtime_version = runtime_version;
            s.registered = registration.registered;
            s.auto_launch = registration.auto_launch;
        });

        let now = Instant::now();
        let mut session = Self {
            ctx,
            tab: None,
            tab_retry_at: now,
        };
        session.open_tab(state, now);
        Ok(session)
    }

    fn open_tab(&mut self, state: &ThreadState, now: Instant) {
        if self.tab.is_some() || now < self.tab_retry_at {
            return;
        }

        let (handle, thumbnail) = match self.ctx.overlay.create_dashboard(DASHBOARD_OVERLAY_KEY, DISPLAY_NAME) {
            Ok(pair) => pair,
            Err(e) => {
                log::warn!("[VR] SteamVR would not give us a dashboard tab: {e}");
                set_error(&state.status, format!("{e}"));
                self.tab_retry_at = now + TAB_RETRY;
                return;
            }
        };

        let o = &self.ctx.overlay;
        let configured = o
            .set_width_meters(handle, panel::WIDTH_METERS)
            .and_then(|()| o.set_alpha(handle, 1.0))
            .and_then(|()| o.set_texture_color_space(handle, sys::EColorSpace_Gamma))
            .and_then(|()| o.set_flag(handle, sys::VROverlayFlags_IsPremultiplied, true))
            .and_then(|()| o.set_input_method(handle, sys::VROverlayInputMethod_Mouse))
            .and_then(|()| o.set_mouse_scale(handle, panel::DESIGN_WIDTH as f32, panel::DESIGN_HEIGHT as f32));

        if let Err(e) = configured {
            log::warn!("[VR] Could not set the dashboard tab up: {e}");
            set_error(&state.status, format!("{e}"));
            let _ = o.destroy(handle);
            let _ = o.destroy(thumbnail);
            self.tab_retry_at = now + TAB_RETRY;
            return;
        }

        let icon = render::tab_icon(ICON_SIZE);
        if let Err(e) = o.set_raw_rgba(thumbnail, &icon, ICON_SIZE, ICON_SIZE) {
            log::debug!("[VR] Could not paint the dashboard tab icon: {e}");
        }

        let gpu = match GpuTexture::new(self.ctx.system.dxgi_adapter_index(), panel::DESIGN_WIDTH, panel::DESIGN_HEIGHT)
        {
            Ok(texture) => Some(texture),
            Err(e) => {
                log::warn!(
                    "[VR] Direct3D 11 unavailable ({e}); using the CPU upload path, \
                     which makes the panel blink as it updates"
                );
                None
            }
        };

        log::info!("[VR] Dashboard tab ready (overlay {})", handle.0);
        state.update_status(|s| s.tab_ready = true);

        self.tab = Some(Tab {
            handle,
            thumbnail,
            panel: Panel::new(panel::DESIGN_WIDTH, panel::DESIGN_HEIGHT),
            gpu,
            uploaded: None,
            visible: false,
            pending: Vec::new(),
            next_redraw_allowed: now,
            refresh_at: now,
            live_revision: 0,
            live_refresh_at: now,
        });
    }

    fn run(&mut self, state: &ThreadState, renderer: &mut PanelRenderer) -> Result<SessionEnd, VrError> {
        loop {
            if state.is_shutdown() {
                return Ok(SessionEnd::Stopped);
            }
            let tick_start = Instant::now();

            if let Some(end) = self.pump_events() {
                return Ok(end);
            }

            self.handle_commands(state);
            self.open_tab(state, tick_start);

            let outcome = self.drain_inputs(tick_start);
            self.apply(state, outcome);

            if !self.tick_panel(state, renderer, tick_start) {
                return Ok(SessionEnd::Restart);
            }

            self.pace(tick_start);
        }
    }

    fn pump_events(&mut self) -> Option<SessionEnd> {
        while let Some(event) = self.ctx.system.poll_event() {
            match event.eventType {
                sys::VREvent_Quit => {
                    self.ctx.system.acknowledge_quit();
                    return Some(SessionEnd::Quit);
                }
                sys::VREvent_DriverRequestedQuit | sys::VREvent_ProcessQuit => {
                    log::debug!("[VR] {} - dropping the session", self.ctx.system.event_name(event.eventType));
                    return Some(SessionEnd::Restart);
                }
                _ => {}
            }
        }

        let tab = self.tab.as_mut()?;

        let height = panel::DESIGN_HEIGHT as f32;
        while let Some(event) = self.ctx.overlay.poll_event(tab.handle) {
            let mouse = event.mouse();
            let (x, y) = (mouse.x, height - mouse.y);

            let input = match event.eventType {
                sys::VREvent_MouseMove => Some(Input::Move { x, y }),
                sys::VREvent_MouseButtonDown if mouse.button == sys::VRMouseButton_Left => Some(Input::Press { x, y }),
                sys::VREvent_MouseButtonUp if mouse.button == sys::VRMouseButton_Left => Some(Input::Release),
                sys::VREvent_FocusLeave | sys::VREvent_OverlayHidden => Some(Input::Leave),
                _ => None,
            };

            if let Some(input) = input
                && tab.pending.len() < INPUT_BACKLOG
            {
                tab.pending.push(input);
            }
        }

        None
    }

    fn drain_inputs(&mut self, now: Instant) -> Outcome {
        let Some(tab) = self.tab.as_mut() else {
            return Outcome::default();
        };

        let mut total = Outcome::default();
        for input in std::mem::take(&mut tab.pending) {
            total.merge(tab.panel.handle(input, now));
        }
        total.merge(tab.panel.tick(now));

        if total.haptic {
            self.ctx.overlay.laser_haptic(tab.handle, 0.014, 160.0, 0.35);
        }
        total
    }

    fn handle_commands(&self, state: &ThreadState) {
        let commands = state.take_commands();
        if commands.is_empty() {
            return;
        }

        let Some(apps) = self.ctx.applications.as_ref() else {
            set_error(&state.status, "this SteamVR runtime does not expose IVRApplications".to_owned());
            return;
        };

        for command in commands {
            let result = match command {
                DashCommand::SetAutoLaunch(on) => {
                    if on && !apps.is_installed(manifest::APP_KEY) {
                        match manifest::register(apps, false) {
                            Ok(path) => {
                                log::info!("[VR] Registered with SteamVR: {}", path.display())
                            }
                            Err(e) => log::warn!("[VR] Could not register: {e}"),
                        }
                    }
                    apps.set_auto_launch(manifest::APP_KEY, on).map(|()| {
                        log::info!("[VR] SteamVR auto-start {}", if on { "enabled" } else { "disabled" });
                    })
                }
            };

            if let Err(e) = result {
                log::warn!("[VR] {e}");
                set_error(&state.status, format!("{e}"));
            }
        }

        let registration = manifest::registration(apps);
        state.update_status(|s| {
            s.registered = registration.registered;
            s.auto_launch = registration.auto_launch;
        });
    }

    fn apply(&self, state: &ThreadState, outcome: Outcome) {
        if outcome.reset_hrv {
            log::info!("[VR] HRV reset from the dashboard");
            state.runtime.reset_hrv();
        }
        if outcome.save_log {
            log::info!("[VR] Session save requested from the dashboard");
            state.runtime.save_log();
        }
    }

    fn tick_panel(&mut self, state: &ThreadState, renderer: &mut PanelRenderer, now: Instant) -> bool {
        let Some(tab) = self.tab.as_mut() else {
            return true;
        };

        let visible = self.ctx.overlay.is_visible(tab.handle)
            || (self.ctx.overlay.is_dashboard_visible() && self.ctx.overlay.is_active_dashboard_overlay(tab.handle));

        if visible != tab.visible {
            tab.visible = visible;
            if !visible {
                tab.panel.handle(Input::Leave, now);
                tab.pending.clear();
            }
            log::debug!("[VR] Dashboard tab visibility: {visible}");
        }

        if !visible {
            return true;
        }

        let revision = state.runtime.revision();
        if revision != tab.live_revision || now >= tab.live_refresh_at {
            tab.live_revision = revision;
            tab.live_refresh_at = now + LIVE_REFRESH_INTERVAL;

            let (live, notice) = {
                let now_t = state.runtime.elapsed_secs();
                state.runtime.read(|snap| {
                    (
                        panel::Live::from_snapshot(snap, now, now_t),
                        snap.notice.as_ref().map(|n| (n.seq, n.text.clone(), n.is_error)),
                    )
                })
            };
            tab.panel.set_live(live);
            if let Some((seq, text, is_error)) = notice {
                tab.panel.set_notice(seq, &text, is_error);
            }
        }

        let due = tab.panel.is_dirty()
            || now >= tab.refresh_at
            || (tab.panel.is_animating() && now >= tab.next_redraw_allowed);
        if !due || now < tab.next_redraw_allowed {
            return true;
        }

        tab.next_redraw_allowed = now + MIN_REDRAW_INTERVAL;
        tab.refresh_at = now + REFRESH_INTERVAL;
        tab.panel.clear_dirty();

        let (pixels, w, h) = renderer.render(&tab.panel);
        let fingerprint = fingerprint(pixels);
        if tab.uploaded == Some(fingerprint) {
            return true;
        }

        let result = match &tab.gpu {
            Some(gpu) => {
                if let Err(e) = gpu.update(pixels) {
                    log::warn!("[VR] Direct3D texture update failed: {e}; restarting the session");
                    return false;
                }
                self.ctx.overlay.set_texture(tab.handle, &gpu.as_openvr_texture())
            }
            None => self.ctx.overlay.set_raw_rgba(tab.handle, pixels, w, h),
        };

        match result {
            Ok(()) => {
                tab.uploaded = Some(fingerprint);
                true
            }
            Err(e) => {
                log::warn!("[VR] Panel upload failed: {e}; restarting the session");
                false
            }
        }
    }

    fn pace(&self, tick_start: Instant) {
        let hz = match self.tab.as_ref() {
            Some(tab) if tab.visible => POLL_HZ_VISIBLE,
            _ => POLL_HZ_HIDDEN,
        };
        let period = Duration::from_secs_f32(1.0 / hz);
        if let Some(rest) = period.checked_sub(tick_start.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let Some(tab) = self.tab.take() else {
            return;
        };
        let _ = self.ctx.overlay.clear_texture(tab.handle);
        for handle in [tab.handle, tab.thumbnail] {
            if let Err(e) = self.ctx.overlay.destroy(handle) {
                log::debug!("[VR] Could not destroy overlay {}: {e}", handle.0);
            }
        }
    }
}

fn announce(apps: &crate::applications::Applications) -> manifest::Registration {
    if apps.is_installed(manifest::APP_KEY) {
        return manifest::registration(apps);
    }

    if manifest::manifest_path().exists() {
        match manifest::register(apps, true) {
            Ok(path) => log::debug!("[VR] Announced ourselves with {}", path.display()),
            Err(e) => log::debug!("[VR] Could not announce ourselves: {e}"),
        }
        return manifest::registration(apps);
    }

    match manifest::register(apps, false) {
        Ok(path) => {
            log::info!("[VR] Registered with SteamVR: {}", path.display());
            match apps.set_auto_launch(manifest::APP_KEY, true) {
                Ok(()) => log::info!(
                    "[VR] This application will now start with SteamVR. Turn that off \
                     with --autostart false, or from the Logs view."
                ),
                Err(e) => log::warn!("[VR] Could not enable SteamVR auto-start: {e}"),
            }
        }
        Err(e) => log::warn!("[VR] Could not register with SteamVR: {e}"),
    }

    manifest::registration(apps)
}

fn fingerprint(pixels: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in pixels {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn sleep_interruptible(state: &ThreadState, total: Duration) {
    let step = Duration::from_millis(100);
    let mut left = total;
    while left > Duration::ZERO && !state.is_shutdown() {
        let chunk = step.min(left);
        std::thread::sleep(chunk);
        left -= chunk;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fingerprint_changes_with_the_image() {
        let a = vec![0u8; 64];
        let mut b = a.clone();
        b[17] = 1;
        assert_eq!(fingerprint(&a), fingerprint(&a));
        assert_ne!(fingerprint(&a), fingerprint(&b));
    }

    #[test]
    fn a_hidden_tab_polls_far_less_often_than_a_visible_one() {
        const _: () = assert!(POLL_HZ_HIDDEN < POLL_HZ_VISIBLE / 5.0);
    }
}
