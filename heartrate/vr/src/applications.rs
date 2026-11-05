use std::{ffi::CString, path::Path};

use crate::{
    error::VrError,
    sys::{self, VR_IVRApplications_FnTable},
};

pub struct Applications {
    table: *const VR_IVRApplications_FnTable,
}

impl Applications {
    pub(crate) fn new(table: *const VR_IVRApplications_FnTable) -> Self {
        Self { table }
    }

    #[inline]
    fn t(&self) -> &VR_IVRApplications_FnTable {
        unsafe { &*self.table }
    }

    fn check(&self, call: &'static str, code: sys::EVRApplicationError) -> Result<(), VrError> {
        if code == sys::VRApplicationError_None {
            return Ok(());
        }

        let message = unsafe { sys::cstr_to_string((self.t().GetApplicationsErrorNameFromEnum)(code)) };
        Err(VrError::Applications { call, code, message })
    }

    pub fn add_manifest(&self, path: &Path, temporary: bool) -> Result<(), VrError> {
        let c = CString::new(path.to_string_lossy().into_owned()).map_err(|_| VrError::Applications {
            call:    "AddApplicationManifest",
            code:    -1,
            message: "path contains a NUL byte".into(),
        })?;

        let code = unsafe { (self.t().AddApplicationManifest)(c.as_ptr(), temporary) };
        self.check("AddApplicationManifest", code)
    }

    pub fn remove_manifest(&self, path: &Path) -> Result<(), VrError> {
        let c = CString::new(path.to_string_lossy().into_owned()).map_err(|_| VrError::Applications {
            call:    "RemoveApplicationManifest",
            code:    -1,
            message: "path contains a NUL byte".into(),
        })?;

        let code = unsafe { (self.t().RemoveApplicationManifest)(c.as_ptr()) };
        self.check("RemoveApplicationManifest", code)
    }

    pub fn is_installed(&self, app_key: &str) -> bool {
        let Ok(c) = CString::new(app_key) else {
            return false;
        };

        unsafe { (self.t().IsApplicationInstalled)(c.as_ptr()) }
    }

    pub fn set_auto_launch(&self, app_key: &str, enabled: bool) -> Result<(), VrError> {
        let c = CString::new(app_key).unwrap_or_default();

        let code = unsafe { (self.t().SetApplicationAutoLaunch)(c.as_ptr(), enabled) };
        self.check("SetApplicationAutoLaunch", code)
    }

    pub fn auto_launch(&self, app_key: &str) -> bool {
        let Ok(c) = CString::new(app_key) else {
            return false;
        };

        unsafe { (self.t().GetApplicationAutoLaunch)(c.as_ptr()) }
    }
}
