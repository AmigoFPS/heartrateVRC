use std::ffi::CString;
use std::os::raw::c_void;

use crate::error::VrError;
use crate::sys::{self, VR_IVROverlay_FnTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayHandle(pub sys::VROverlayHandle_t);

impl OverlayHandle {
    pub fn is_valid(self) -> bool {
        self.0 != sys::k_ulOverlayHandleInvalid
    }
}

pub struct Overlay {
    table: *const VR_IVROverlay_FnTable,
}

impl Overlay {
    pub(crate) fn new(table: *const VR_IVROverlay_FnTable) -> Self {
        Self { table }
    }

    #[inline]
    fn t(&self) -> &VR_IVROverlay_FnTable {
        unsafe { &*self.table }
    }

    fn check(&self, call: &'static str, code: sys::EVROverlayError) -> Result<(), VrError> {
        if code == sys::VROverlayError_None {
            return Ok(());
        }

        let message = unsafe { sys::cstr_to_string((self.t().GetOverlayErrorNameFromEnum)(code)) };
        Err(VrError::Overlay {
            call,
            code,
            message,
        })
    }

    pub fn create_dashboard(
        &self,
        key: &str,
        friendly_name: &str,
    ) -> Result<(OverlayHandle, OverlayHandle), VrError> {
        let c_key = CString::new(key).unwrap_or_default();
        let c_name = CString::new(friendly_name).unwrap_or_default();
        let mut main: sys::VROverlayHandle_t = sys::k_ulOverlayHandleInvalid;
        let mut thumb: sys::VROverlayHandle_t = sys::k_ulOverlayHandleInvalid;

        let code = unsafe {
            (self.t().CreateDashboardOverlay)(c_key.as_ptr(), c_name.as_ptr(), &mut main, &mut thumb)
        };
        if code == sys::VROverlayError_KeyInUse {
            return Err(VrError::Init(format!(
                "the dashboard overlay key {key} is already in use. Another copy of this \
                 program is probably running: SteamVR's auto-start and a manual launch \
                 each start one."
            )));
        }
        self.check("CreateDashboardOverlay", code)?;
        Ok((OverlayHandle(main), OverlayHandle(thumb)))
    }

    pub fn destroy(&self, h: OverlayHandle) -> Result<(), VrError> {
        if !h.is_valid() {
            return Ok(());
        }

        let code = unsafe { (self.t().DestroyOverlay)(h.0) };
        self.check("DestroyOverlay", code)
    }

    pub fn set_width_meters(&self, h: OverlayHandle, width: f32) -> Result<(), VrError> {
        let code = unsafe { (self.t().SetOverlayWidthInMeters)(h.0, width.max(0.001)) };
        self.check("SetOverlayWidthInMeters", code)
    }

    pub fn set_alpha(&self, h: OverlayHandle, alpha: f32) -> Result<(), VrError> {
        let code = unsafe { (self.t().SetOverlayAlpha)(h.0, alpha.clamp(0.0, 1.0)) };
        self.check("SetOverlayAlpha", code)
    }

    pub fn set_flag(
        &self,
        h: OverlayHandle,
        flag: sys::VROverlayFlags,
        enabled: bool,
    ) -> Result<(), VrError> {
        let code = unsafe { (self.t().SetOverlayFlag)(h.0, flag, enabled) };
        self.check("SetOverlayFlag", code)
    }

    pub fn set_texture_color_space(
        &self,
        h: OverlayHandle,
        space: sys::EColorSpace,
    ) -> Result<(), VrError> {
        let code = unsafe { (self.t().SetOverlayTextureColorSpace)(h.0, space) };
        self.check("SetOverlayTextureColorSpace", code)
    }

    pub fn set_raw_rgba(
        &self,
        h: OverlayHandle,
        pixels: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), VrError> {
        debug_assert_eq!(pixels.len(), (width as usize) * (height as usize) * 4);

        let code = unsafe {
            (self.t().SetOverlayRaw)(h.0, pixels.as_ptr() as *const c_void, width, height, 4)
        };
        self.check("SetOverlayRaw", code)
    }

    pub fn set_texture(&self, h: OverlayHandle, texture: &sys::Texture_t) -> Result<(), VrError> {
        let code = unsafe { (self.t().SetOverlayTexture)(h.0, texture) };
        self.check("SetOverlayTexture", code)
    }

    pub fn clear_texture(&self, h: OverlayHandle) -> Result<(), VrError> {
        let code = unsafe { (self.t().ClearOverlayTexture)(h.0) };
        self.check("ClearOverlayTexture", code)
    }

    pub fn is_visible(&self, h: OverlayHandle) -> bool {
        unsafe { (self.t().IsOverlayVisible)(h.0) }
    }

    pub fn is_dashboard_visible(&self) -> bool {
        unsafe { (self.t().IsDashboardVisible)() }
    }

    pub fn is_active_dashboard_overlay(&self, h: OverlayHandle) -> bool {
        unsafe { (self.t().IsActiveDashboardOverlay)(h.0) }
    }

    pub fn poll_event(&self, h: OverlayHandle) -> Option<sys::VREvent_t> {
        let mut event = sys::VREvent_t::default();
        let got = unsafe {
            (self.t().PollNextOverlayEvent)(
                h.0,
                &mut event,
                std::mem::size_of::<sys::VREvent_t>() as u32,
            )
        };
        got.then_some(event)
    }

    pub fn set_input_method(
        &self,
        h: OverlayHandle,
        method: sys::VROverlayInputMethod,
    ) -> Result<(), VrError> {
        let code = unsafe { (self.t().SetOverlayInputMethod)(h.0, method) };
        self.check("SetOverlayInputMethod", code)
    }

    pub fn set_mouse_scale(&self, h: OverlayHandle, width: f32, height: f32) -> Result<(), VrError> {
        let scale = sys::HmdVector2_t {
            v: [width.max(1.0), height.max(1.0)],
        };

        let code = unsafe { (self.t().SetOverlayMouseScale)(h.0, &scale) };
        self.check("SetOverlayMouseScale", code)
    }

    pub fn laser_haptic(&self, h: OverlayHandle, seconds: f32, frequency: f32, amplitude: f32) {
        unsafe {
            (self.t().TriggerLaserMouseHapticVibration)(h.0, seconds, frequency, amplitude);
        }
    }
}
