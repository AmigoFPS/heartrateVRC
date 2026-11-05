use std::{ffi::CString, sync::OnceLock};

use crate::{
    applications::Applications,
    error::VrError,
    overlay::Overlay,
    sys::{self, OpenVrLib},
    system::System,
};

static LIB: OnceLock<OpenVrLib> = OnceLock::new();

fn lib() -> Result<&'static OpenVrLib, VrError> {
    if let Some(l) = LIB.get() {
        return Ok(l);
    }
    let loaded = OpenVrLib::load()?;
    log::info!("[VR] Loaded OpenVR runtime from {}", loaded.path.display());

    Ok(LIB.get_or_init(|| loaded))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKind {
    Overlay,
    Background,
}

impl AppKind {
    fn to_raw(self) -> sys::EVRApplicationType {
        match self {
            AppKind::Overlay => sys::EVRApplicationType_VRApplication_Overlay,
            AppKind::Background => sys::EVRApplicationType_VRApplication_Background,
        }
    }
}

pub struct VrContext {
    lib:              &'static OpenVrLib,
    pub system:       System,
    pub overlay:      Overlay,
    pub applications: Option<Applications>,
    runtime_version:  String,
}

impl VrContext {
    pub fn init(kind: AppKind) -> Result<Self, VrError> {
        let lib = lib()?;

        if !unsafe { (lib.VR_IsRuntimeInstalled)() } {
            return Err(VrError::RuntimeNotInstalled);
        }

        let mut err: sys::EVRInitError = sys::VRInitError_None;
        let token = unsafe { (lib.VR_InitInternal2)(&mut err, kind.to_raw(), std::ptr::null()) };
        if err != sys::VRInitError_None || token == 0 {
            return Err(VrError::Init(lib.init_error_message(err)));
        }

        let built = (|| -> Result<Self, VrError> {
            let system = System::new(get_interface(lib, sys::interfaces::IVRSystem)?);
            let overlay = Overlay::new(get_interface(lib, sys::interfaces::IVROverlay)?);
            let applications = match get_interface(lib, sys::interfaces::IVRApplications) {
                Ok(p) => Some(Applications::new(p)),
                Err(e) => {
                    log::debug!("[VR] IVRApplications unavailable: {e}");
                    None
                }
            };

            let runtime_version = system.runtime_version();
            Ok(Self {
                lib,
                system,
                overlay,
                applications,
                runtime_version,
            })
        })();

        match built {
            Ok(ctx) => {
                log::info!("[VR] OpenVR session established (runtime {}, {:?})", ctx.runtime_version, kind);
                Ok(ctx)
            }
            Err(e) => {
                unsafe { (lib.VR_ShutdownInternal)() };
                Err(e)
            }
        }
    }

    pub fn runtime_version(&self) -> &str {
        &self.runtime_version
    }
}

impl Drop for VrContext {
    fn drop(&mut self) {
        log::debug!("[VR] Shutting down the OpenVR session");
        unsafe { (self.lib.VR_ShutdownInternal)() };
    }
}

fn get_interface<T>(lib: &OpenVrLib, version: &'static str) -> Result<*const T, VrError> {
    let c = CString::new(version).expect("interface version has no interior NUL");
    let mut err: sys::EVRInitError = sys::VRInitError_None;

    let ptr = unsafe { (lib.VR_GetGenericInterface)(c.as_ptr(), &mut err) };
    if ptr == 0 || err != sys::VRInitError_None {
        return Err(VrError::InterfaceUnavailable {
            version,
            detail: lib.init_error_message(err),
        });
    }
    Ok(ptr as *const T)
}
