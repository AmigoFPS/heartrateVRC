use std::path::{Path, PathBuf};

use crate::{
    applications::Applications,
    context::{AppKind, VrContext},
    error::VrError,
};

pub const APP_KEY: &str = "heartrate.osc.overlay";

pub const DASHBOARD_OVERLAY_KEY: &str = "heartrate.osc.dashboard";

pub const DISPLAY_NAME: &str = "Heartrate OSC";

const MANIFEST_FILE: &str = "heartrate-osc.vrmanifest";

pub fn manifest_path() -> PathBuf {
    if let Some(dir) = exe_dir() {
        if is_writable(&dir) {
            return dir.join(MANIFEST_FILE);
        }
        log::debug!(
            "[VR] {} is not writable; keeping the manifest in the user profile",
            dir.display()
        );
    }

    user_data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(MANIFEST_FILE)
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

fn user_data_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(base).join("HeartrateOSC"))
}

fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".heartrate-write-probe");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

pub fn build_manifest(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    let manifest = serde_json::json!({
        "source": "builtin",
        "applications": [{
            "app_key": APP_KEY,
            "launch_type": "binary",
            "binary_path_windows": path,
            "binary_path_linux": path,
            "arguments": "--no-gui",
            "is_dashboard_overlay": true,
            "strings": {
                "en_us": {
                    "name": DISPLAY_NAME,
                    "description": "Sends your heart rate to VRChat and shows it in the SteamVR dashboard."
                }
            }
        }],
    });
    serde_json::to_string_pretty(&manifest).unwrap_or_default()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Registration {
    pub registered:  bool,
    pub auto_launch: bool,
}

pub fn registration(apps: &Applications) -> Registration {
    Registration {
        registered:  apps.is_installed(APP_KEY),
        auto_launch: apps.auto_launch(APP_KEY),
    }
}

pub fn register(apps: &Applications, temporary: bool) -> Result<PathBuf, VrError> {
    let path = manifest_path();
    let exe = std::env::current_exe().map_err(|e| VrError::Applications {
        call:    "AddApplicationManifest",
        code:    -1,
        message: format!("could not find our own executable: {e}"),
    })?;

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, build_manifest(&exe)).map_err(|e| VrError::Applications {
        call:    "AddApplicationManifest",
        code:    -1,
        message: format!("could not write {}: {e}", path.display()),
    })?;

    apps.add_manifest(&path, temporary)?;
    Ok(path)
}

pub fn unregister(apps: &Applications) -> Result<PathBuf, VrError> {
    let path = manifest_path();
    apps.remove_manifest(&path)?;
    Ok(path)
}

pub fn manifest_command(register_wanted: Option<bool>, autostart: Option<bool>) -> Result<Vec<String>, VrError> {
    let ctx = VrContext::init(AppKind::Background)?;
    let apps = ctx.applications.as_ref().ok_or_else(|| VrError::Applications {
        call:    "GetGenericInterface",
        code:    -1,
        message: "this SteamVR runtime does not expose IVRApplications".into(),
    })?;

    let mut report = Vec::new();

    match register_wanted {
        Some(true) => {
            let path = register(apps, false)?;
            report.push(format!("Registered with SteamVR: {}", path.display()));
        }
        Some(false) => {
            let path = unregister(apps)?;
            report.push(format!("Removed from SteamVR: {}", path.display()));
        }
        None => {}
    }

    if let Some(on) = autostart {
        if !apps.is_installed(APP_KEY) {
            if !on {
                report.push("Not registered with SteamVR, so auto-start is already off.".into());
                return Ok(report);
            }
            let path = register(apps, false)?;
            report.push(format!("Registered with SteamVR first: {}", path.display()));
        }

        if !apps.is_installed(APP_KEY) {
            return Err(VrError::Applications {
                call:    "SetApplicationAutoLaunch",
                code:    -1,
                message: format!(
                    "SteamVR still does not know the application key {APP_KEY}. \
                     Restart SteamVR and run --register again."
                ),
            });
        }

        apps.set_auto_launch(APP_KEY, on)?;
        report.push(format!("SteamVR auto-start {}", if on { "enabled" } else { "disabled" }));
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_names_the_binary_and_declares_an_overlay() {
        let text = build_manifest(Path::new("C:\\Apps\\heartrate_gui.exe"));
        let v: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        let app = &v["applications"][0];

        assert_eq!(app["app_key"], APP_KEY);
        assert!(
            app["binary_path_windows"]
                .as_str()
                .unwrap()
                .ends_with("heartrate_gui.exe")
        );
        assert_eq!(app["is_dashboard_overlay"], true);
        assert_eq!(app["arguments"], "--no-gui");
        assert_eq!(app["launch_type"], "binary");
    }

    #[test]
    fn the_manifest_path_is_absolute_and_keeps_its_name() {
        let path = manifest_path();
        assert_eq!(path.file_name().unwrap(), MANIFEST_FILE);
        assert!(path.is_absolute() || path.starts_with("."));
    }

    #[test]
    fn the_overlay_key_and_the_app_key_are_distinct() {
        assert_ne!(APP_KEY, DASHBOARD_OVERLAY_KEY);
    }
}
