
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cli;
mod console;

use std::panic::AssertUnwindSafe;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use heartrate_core::runtime::Runtime;
use heartrate_core::{log_buffer, settings_manager::AppSettings};
use heartrate_vr::manifest::{self, DASHBOARD_OVERLAY_KEY};
use heartrate_vr::{Dashboard, InstanceLock};

use crate::cli::Cli;

const GUI_RESTART_DELAY: Duration = Duration::from_secs(3);

const GUI_MAX_ATTEMPTS: u32 = 2;

fn main() -> ExitCode {
    let cli = match Cli::from_env() {
        Ok(cli) => cli,
        Err(e) => {
            console::attach_parent();
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };

    if cli.help {
        console::attach_parent();
        println!("{}", cli::USAGE);
        return ExitCode::SUCCESS;
    }
    if cli.version {
        console::attach_parent();
        println!("heartrate-osc {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let _ = log_buffer::init();
    if cli.verbose {
        log_buffer::set_verbose(true);
    }

    log_buffer::set_echo(console::attach_parent());
    install_panic_hook();

    if cli.is_oneshot() {
        return run_oneshot(&cli);
    }

    let Some(_instance) = InstanceLock::acquire(DASHBOARD_OVERLAY_KEY) else {
        console::attach_parent();
        let message = "Heartrate OSC is already running. Close the other copy first - \
                       SteamVR may have started one for you (turn that off with \
                       --autostart false).";
        println!("{message}");
        log::info!("[APP] {message}");
        return ExitCode::SUCCESS;
    };

    log::info!(
        "[APP] Heartrate OSC {} starting{}",
        env!("CARGO_PKG_VERSION"),
        if cli.no_gui { " (headless)" } else { "" }
    );

    let settings = match AppSettings::load_or_create() {
        Ok(settings) => settings,
        Err(e) => {
            log::error!("[APP] Could not load the settings: {e}; using the defaults");
            AppSettings::default()
        }
    };

    let runtime = Arc::new(Runtime::start(settings));
    let dashboard = Arc::new(Dashboard::start(Arc::clone(&runtime), cli.no_gui));

    if cli.no_gui {
        log::info!("[APP] Running without a desktop window");
        wait_for_shutdown(&runtime, &dashboard);
    } else {
        run_desktop(&runtime, &dashboard);
    }

    log::info!("[APP] Shutting down");
    runtime.request_shutdown();
    dashboard.shutdown(Duration::from_secs(2));
    ExitCode::SUCCESS
}

fn run_oneshot(cli: &Cli) -> ExitCode {
    console::attach_parent();

    let register = if cli.register {
        Some(true)
    } else if cli.unregister {
        Some(false)
    } else {
        None
    };

    match manifest::manifest_command(register, cli.autostart) {
        Ok(report) => {
            for line in report {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            eprintln!("SteamVR has to be running for this command.");
            ExitCode::FAILURE
        }
    }
}

fn run_desktop(runtime: &Arc<Runtime>, dashboard: &Arc<Dashboard>) {
    let closed = drive_window(
        || start_window(runtime, dashboard),
        || runtime.is_shutdown() || dashboard.should_exit(),
        GUI_RESTART_DELAY,
        GUI_MAX_ATTEMPTS,
    );
    if closed {
        return;
    }

    log::warn!(
        "[GUI] Giving up on the desktop window. Heart rate monitoring and VRChat output \
         continue, and the readout is still available in the SteamVR dashboard."
    );
    wait_for_shutdown(runtime, dashboard);
}

fn drive_window(
    mut start: impl FnMut() -> WindowEnd,
    stopping: impl Fn() -> bool,
    delay: Duration,
    attempts: u32,
) -> bool {
    for attempt in 1..=attempts {
        match start() {
            WindowEnd::Closed => {
                log::info!("[APP] Desktop window closed");
                return true;
            }
            WindowEnd::Failed(reason) => {
                log::error!("[GUI] The desktop window failed: {reason}");
            }
            WindowEnd::Panicked(reason) => {
                log::error!("[GUI] The desktop window panicked: {reason}");
            }
        }

        if stopping() {
            return true;
        }

        if attempt < attempts {
            log::warn!(
                "[GUI] Heart rate monitoring is unaffected; reopening the window in {}s",
                delay.as_secs()
            );
            std::thread::sleep(delay);
        }
    }

    false
}

enum WindowEnd {
    Closed,
    Failed(String),
    Panicked(String),
}

fn start_window(runtime: &Arc<Runtime>, dashboard: &Arc<Dashboard>) -> WindowEnd {
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("HeartRate OSC")
            .with_inner_size(app::DEFAULT_WINDOW_SIZE)
            .with_min_inner_size(app::DEFAULT_WINDOW_SIZE),
        ..Default::default()
    };
    options.wgpu_options.surface = eframe::SurfaceConfig::LOW_LATENCY;

    let runtime = Arc::clone(runtime);
    let dashboard = Arc::clone(dashboard);

    guarded(move || {
        eframe::run_native(
            "HeartRate OSC",
            options,
            Box::new(move |cc| Ok(Box::new(app::HeartRateApp::new(cc, runtime, dashboard)))),
        )
    })
}

fn guarded(body: impl FnOnce() -> eframe::Result) -> WindowEnd {
    match std::panic::catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(())) => WindowEnd::Closed,
        Ok(Err(e)) => WindowEnd::Failed(format!("{e}")),
        Err(payload) => WindowEnd::Panicked(panic_message(payload.as_ref())),
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_owned()
    }
}

fn wait_for_shutdown(runtime: &Arc<Runtime>, dashboard: &Arc<Dashboard>) {
    while !runtime.is_shutdown() && !dashboard.should_exit() {
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_owned());
        log::error!("[PANIC] {} at {location}", panic_message(info.payload()));
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn a_panicking_window_is_caught_rather_than_killing_the_process() {
        let end = guarded(|| panic!("device lost"));
        match end {
            WindowEnd::Panicked(reason) => assert_eq!(reason, "device lost"),
            _ => panic!("a panic in the window must be reported as one"),
        }

        let formatted = guarded(|| panic!("surface gone: {}", 3));
        assert!(matches!(formatted, WindowEnd::Panicked(r) if r == "surface gone: 3"));
    }

    #[test]
    fn a_window_that_closes_cleanly_is_not_a_failure() {
        assert!(matches!(guarded(|| Ok(())), WindowEnd::Closed));
    }

    #[test]
    fn a_failing_window_is_retried_and_then_left_behind() {
        let tries = Cell::new(0);
        let closed = drive_window(
            || {
                tries.set(tries.get() + 1);
                WindowEnd::Panicked("gpu reset".into())
            },
            || false,
            Duration::ZERO,
            3,
        );

        assert_eq!(tries.get(), 3, "every attempt should be used");
        assert!(
            !closed,
            "giving up must report that the window is gone, so the caller keeps \
             the process alive for the pipeline"
        );
    }

    #[test]
    fn the_window_is_not_reopened_once_the_user_has_closed_it() {
        let tries = Cell::new(0);
        let closed = drive_window(
            || {
                tries.set(tries.get() + 1);
                WindowEnd::Closed
            },
            || false,
            Duration::ZERO,
            3,
        );

        assert_eq!(tries.get(), 1);
        assert!(closed);
    }

    #[test]
    fn a_failure_during_shutdown_does_not_reopen_the_window() {
        let tries = Cell::new(0);
        let closed = drive_window(
            || {
                tries.set(tries.get() + 1);
                WindowEnd::Failed("surface lost".into())
            },
            || true,
            Duration::ZERO,
            3,
        );

        assert_eq!(tries.get(), 1, "there is nothing to reopen it for");
        assert!(closed);
    }

    #[test]
    fn panic_payloads_that_are_not_strings_still_produce_a_message() {
        let end = guarded(|| std::panic::panic_any(42u32));
        assert!(matches!(end, WindowEnd::Panicked(r) if r.contains("unknown")));
    }
}
