use crate::sys;

#[derive(Debug)]
pub enum VrError {
    Library(sys::LoadError),
    RuntimeNotInstalled,
    Init(String),
    InterfaceUnavailable {
        version: &'static str,
        detail:  String,
    },
    Overlay {
        call:    &'static str,
        code:    i32,
        message: String,
    },
    Applications {
        call:    &'static str,
        code:    i32,
        message: String,
    },
}

impl VrError {
    pub fn is_session_lost(&self) -> bool {
        matches!(self, VrError::Init(_) | VrError::RuntimeNotInstalled | VrError::Overlay { .. })
    }
}

impl std::fmt::Display for VrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VrError::Library(e) => write!(f, "{e}"),
            VrError::RuntimeNotInstalled => write!(
                f,
                "SteamVR is not installed, or its runtime could not be located. \
                 Install SteamVR from Steam (Library -> Tools -> SteamVR) and start it once."
            ),
            VrError::Init(detail) => write!(f, "VR_Init failed: {detail}"),
            VrError::InterfaceUnavailable { version, detail } => write!(
                f,
                "this SteamVR runtime does not provide `{version}` ({detail}). \
                 The bindings in heartrate_vr::sys are transcribed from OpenVR SDK 2.15.6, \
                 which needs a reasonably current SteamVR."
            ),
            VrError::Overlay { call, code, message } => write!(f, "IVROverlay::{call} failed: {message} (code {code})"),
            VrError::Applications { call, code, message } => {
                write!(f, "IVRApplications::{call} failed: {message} (code {code})")
            }
        }
    }
}

impl std::error::Error for VrError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VrError::Library(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sys::LoadError> for VrError {
    fn from(e: sys::LoadError) -> Self {
        VrError::Library(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_failures_end_the_session_but_a_missing_interface_does_not() {
        assert!(
            VrError::Overlay {
                call:    "ShowOverlay",
                code:    1,
                message: "whatever".into(),
            }
            .is_session_lost()
        );
        assert!(
            !VrError::InterfaceUnavailable {
                version: "FnTable:IVROverlay_028",
                detail:  String::new(),
            }
            .is_session_lost()
        );
    }
}
