
#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, dead_code)]

use std::ffi::CStr;
use std::os::raw::{c_char, c_void};
use std::path::{Path, PathBuf};

pub type TrackedDeviceIndex_t = u32;
pub type VROverlayHandle_t = u64;
pub type VRActionHandle_t = u64;
pub type VRActionSetHandle_t = u64;
pub type VRInputValueHandle_t = u64;
pub type PropertyTypeTag_t = u32;
pub type EVRInitError = i32;
pub type EVRApplicationType = i32;
pub type EVROverlayError = i32;
pub type EVRInputError = i32;
pub type EVRApplicationError = i32;
pub type EVRSettingsError = i32;
pub type EVRApplicationProperty = i32;
pub type ETrackedPropertyError = i32;
pub type ETrackingUniverseOrigin = i32;
pub type ETrackedControllerRole = i32;
pub type ETrackedDeviceClass = i32;
pub type ETrackedDeviceProperty = i32;
pub type ETrackingResult = i32;
pub type EColorSpace = i32;
pub type VROverlayFlags = i32;
pub type ETextureType = i32;
pub type VROverlayInputMethod = i32;
pub type EVRMouseButton = u32;

pub type UnusedFn = *const c_void;

pub const k_unTrackedDeviceIndex_Hmd: TrackedDeviceIndex_t = 0;
pub const k_unMaxTrackedDeviceCount: usize = 64;
pub const k_unTrackedDeviceIndexInvalid: TrackedDeviceIndex_t = 0xFFFF_FFFF;
pub const k_ulOverlayHandleInvalid: VROverlayHandle_t = 0;
pub const k_ulInvalidActionHandle: VRActionHandle_t = 0;
pub const k_ulInvalidActionSetHandle: VRActionSetHandle_t = 0;
pub const k_ulInvalidInputValueHandle: VRInputValueHandle_t = 0;

pub const EVRApplicationType_VRApplication_Other: EVRApplicationType = 0;
pub const EVRApplicationType_VRApplication_Overlay: EVRApplicationType = 2;
pub const EVRApplicationType_VRApplication_Background: EVRApplicationType = 3;

pub const ETrackingUniverseOrigin_Seated: ETrackingUniverseOrigin = 0;
pub const ETrackingUniverseOrigin_Standing: ETrackingUniverseOrigin = 1;
pub const ETrackingUniverseOrigin_RawAndUncalibrated: ETrackingUniverseOrigin = 2;

pub const ETrackedControllerRole_Invalid: ETrackedControllerRole = 0;
pub const ETrackedControllerRole_LeftHand: ETrackedControllerRole = 1;
pub const ETrackedControllerRole_RightHand: ETrackedControllerRole = 2;

pub const ETrackedDeviceClass_Invalid: ETrackedDeviceClass = 0;
pub const ETrackedDeviceClass_HMD: ETrackedDeviceClass = 1;
pub const ETrackedDeviceClass_Controller: ETrackedDeviceClass = 2;
pub const ETrackedDeviceClass_GenericTracker: ETrackedDeviceClass = 3;

pub const ETrackingResult_Running_OK: ETrackingResult = 200;

pub const Prop_ModelNumber_String: ETrackedDeviceProperty = 1001;
pub const Prop_RenderModelName_String: ETrackedDeviceProperty = 1003;
pub const Prop_DeviceIsCharging_Bool: ETrackedDeviceProperty = 1010;
pub const Prop_DeviceProvidesBatteryStatus_Bool: ETrackedDeviceProperty = 1011;

pub const Prop_DeviceBatteryPercentage_Float: ETrackedDeviceProperty = 1012;
pub const Prop_ResourceRoot_String: ETrackedDeviceProperty = 1035;
pub const Prop_DisplayFrequency_Float: ETrackedDeviceProperty = 2002;
pub const Prop_ControllerType_String: ETrackedDeviceProperty = 7000;

pub const ETextureType_Invalid: ETextureType = -1;
pub const ETextureType_DirectX: ETextureType = 0;
pub const ETextureType_OpenGL: ETextureType = 1;
pub const ETextureType_Vulkan: ETextureType = 2;
pub const ETextureType_DirectX12: ETextureType = 4;
pub const ETextureType_DXGISharedHandle: ETextureType = 5;

pub const EColorSpace_Auto: EColorSpace = 0;
pub const EColorSpace_Gamma: EColorSpace = 1;
pub const EColorSpace_Linear: EColorSpace = 2;

pub const VROverlayInputMethod_None: VROverlayInputMethod = 0;

pub const VROverlayInputMethod_Mouse: VROverlayInputMethod = 1;

pub const VRMouseButton_Left: EVRMouseButton = 1;
pub const VRMouseButton_Right: EVRMouseButton = 2;
pub const VRMouseButton_Middle: EVRMouseButton = 4;

pub const VROverlayFlags_NoDashboardTab: VROverlayFlags = 8;
pub const VROverlayFlags_SortWithNonSceneOverlays: VROverlayFlags = 16384;
pub const VROverlayFlags_VisibleInDashboard: VROverlayFlags = 32768;
pub const VROverlayFlags_MakeOverlaysInteractiveIfVisible: VROverlayFlags = 65536;
pub const VROverlayFlags_HideLaserIntersection: VROverlayFlags = 524288;
pub const VROverlayFlags_IsPremultiplied: VROverlayFlags = 2097152;
pub const VROverlayFlags_IgnoreTextureAlpha: VROverlayFlags = 4194304;
pub const VROverlayFlags_NoBackside: VROverlayFlags = 536870912;

pub const VREvent_TrackedDeviceActivated: u32 = 100;
pub const VREvent_TrackedDeviceDeactivated: u32 = 101;
pub const VREvent_TrackedDeviceUpdated: u32 = 102;
pub const VREvent_TrackedDeviceRoleChanged: u32 = 108;

pub const VREvent_MouseMove: u32 = 300;
pub const VREvent_MouseButtonDown: u32 = 301;
pub const VREvent_MouseButtonUp: u32 = 302;
pub const VREvent_FocusEnter: u32 = 303;
pub const VREvent_FocusLeave: u32 = 304;
pub const VREvent_ScrollDiscrete: u32 = 305;
pub const VREvent_ScrollSmooth: u32 = 309;
pub const VREvent_SceneApplicationChanged: u32 = 404;
pub const VREvent_OverlayShown: u32 = 500;
pub const VREvent_OverlayHidden: u32 = 501;
pub const VREvent_DashboardActivated: u32 = 502;
pub const VREvent_DashboardDeactivated: u32 = 503;
pub const VREvent_Quit: u32 = 700;
pub const VREvent_ProcessQuit: u32 = 701;
pub const VREvent_DriverRequestedQuit: u32 = 704;

pub const VRInitError_None: EVRInitError = 0;
pub const VROverlayError_None: EVROverlayError = 0;
pub const VROverlayError_KeyInUse: EVROverlayError = 17;
pub const VRInputError_None: EVRInputError = 0;
pub const VRApplicationError_None: EVRApplicationError = 0;
pub const VRSettingsError_None: EVRSettingsError = 0;

pub const VRApplicationProperty_Name_String: EVRApplicationProperty = 0;


pub mod interfaces {
    pub const IVRSystem: &str = "FnTable:IVRSystem_026";
    pub const IVRApplications: &str = "FnTable:IVRApplications_008";
    pub const IVROverlay: &str = "FnTable:IVROverlay_028";
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HmdMatrix34_t {

    pub m: [[f32; 4]; 3],
}

impl Default for HmdMatrix34_t {
    fn default() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Texture_t {
    pub handle: *mut c_void,
    pub eType: ETextureType,
    pub eColorSpace: EColorSpace,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct HmdVector3_t {
    pub v: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct HmdVector2_t {
    pub v: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TrackedDevicePose_t {
    pub mDeviceToAbsoluteTracking: HmdMatrix34_t,
    pub vVelocity: HmdVector3_t,
    pub vAngularVelocity: HmdVector3_t,
    pub eTrackingResult: ETrackingResult,
    pub bPoseIsValid: bool,
    pub bDeviceIsConnected: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VREvent_Data_t {
    pub raw: [u64; 6],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VREvent_t {
    pub eventType: u32,
    pub trackedDeviceIndex: TrackedDeviceIndex_t,
    pub eventAgeSeconds: f32,
    pub data: VREvent_Data_t,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VREvent_Mouse_t {

    pub x: f32,

    pub y: f32,

    pub button: EVRMouseButton,
}

impl VREvent_t {

    pub fn mouse(&self) -> VREvent_Mouse_t {

        unsafe { std::mem::transmute_copy(&self.data) }
    }
}

#[repr(C)]
pub struct VR_IVRSystem_FnTable {
    pub GetRecommendedRenderTargetSize: UnusedFn,
    pub GetProjectionMatrix: UnusedFn,
    pub GetProjectionRaw: UnusedFn,
    pub ComputeDistortion: UnusedFn,
    pub ComputeDistortionSet: UnusedFn,
    pub GetEyeToHeadTransform: UnusedFn,
    pub GetTimeSinceLastVsync:
        unsafe extern "C" fn(pfSecondsSinceLastVsync: *mut f32, pulFrameCounter: *mut u64) -> bool,
    pub GetD3D9AdapterIndex: UnusedFn,
    pub GetDXGIOutputInfo: unsafe extern "C" fn(pnAdapterIndex: *mut i32),
    pub GetOutputDevice: UnusedFn,
    pub IsDisplayOnDesktop: UnusedFn,
    pub SetDisplayVisibility: UnusedFn,
    pub GetDeviceToAbsoluteTrackingPose: unsafe extern "C" fn(
        eOrigin: ETrackingUniverseOrigin,
        fPredictedSecondsToPhotonsFromNow: f32,
        pTrackedDevicePoseArray: *mut TrackedDevicePose_t,
        unTrackedDevicePoseArrayCount: u32,
    ),
    pub GetSeatedZeroPoseToStandingAbsoluteTrackingPose: UnusedFn,
    pub GetRawZeroPoseToStandingAbsoluteTrackingPose: UnusedFn,
    pub GetSortedTrackedDeviceIndicesOfClass: unsafe extern "C" fn(
        eTrackedDeviceClass: ETrackedDeviceClass,
        punTrackedDeviceIndexArray: *mut TrackedDeviceIndex_t,
        unTrackedDeviceIndexArrayCount: u32,
        unRelativeToTrackedDeviceIndex: TrackedDeviceIndex_t,
    ) -> u32,
    pub GetTrackedDeviceActivityLevel: UnusedFn,
    pub ApplyTransform: UnusedFn,
    pub GetTrackedDeviceIndexForControllerRole:
        unsafe extern "C" fn(unDeviceType: ETrackedControllerRole) -> TrackedDeviceIndex_t,
    pub GetControllerRoleForTrackedDeviceIndex:
        unsafe extern "C" fn(unDeviceIndex: TrackedDeviceIndex_t) -> ETrackedControllerRole,
    pub GetTrackedDeviceClass:
        unsafe extern "C" fn(unDeviceIndex: TrackedDeviceIndex_t) -> ETrackedDeviceClass,
    pub IsTrackedDeviceConnected: unsafe extern "C" fn(unDeviceIndex: TrackedDeviceIndex_t) -> bool,
    pub GetBoolTrackedDeviceProperty: unsafe extern "C" fn(
        unDeviceIndex: TrackedDeviceIndex_t,
        prop: ETrackedDeviceProperty,
        pError: *mut ETrackedPropertyError,
    ) -> bool,
    pub GetFloatTrackedDeviceProperty: unsafe extern "C" fn(
        unDeviceIndex: TrackedDeviceIndex_t,
        prop: ETrackedDeviceProperty,
        pError: *mut ETrackedPropertyError,
    ) -> f32,
    pub GetInt32TrackedDeviceProperty: UnusedFn,
    pub GetUint64TrackedDeviceProperty: UnusedFn,
    pub GetMatrix34TrackedDeviceProperty: UnusedFn,
    pub GetArrayTrackedDeviceProperty: UnusedFn,
    pub GetStringTrackedDeviceProperty: unsafe extern "C" fn(
        unDeviceIndex: TrackedDeviceIndex_t,
        prop: ETrackedDeviceProperty,
        pchValue: *mut c_char,
        unBufferSize: u32,
        pError: *mut ETrackedPropertyError,
    ) -> u32,
    pub GetPropErrorNameFromEnum: UnusedFn,
    pub PollNextEvent:
        unsafe extern "C" fn(pEvent: *mut VREvent_t, uncbVREvent: u32) -> bool,
    pub PollNextEventWithPose: UnusedFn,
    pub PollNextEventWithPoseAndOverlays: UnusedFn,
    pub GetEventTypeNameFromEnum: unsafe extern "C" fn(eType: u32) -> *const c_char,
    pub GetHiddenAreaMesh: UnusedFn,
    pub GetEyeTrackedFoveationCenter: UnusedFn,
    pub GetEyeTrackedFoveationCenterForProjection: UnusedFn,
    pub GetControllerState: UnusedFn,
    pub GetControllerStateWithPose: UnusedFn,
    pub TriggerHapticPulse: unsafe extern "C" fn(
        unControllerDeviceIndex: TrackedDeviceIndex_t,
        unAxisId: u32,
        usDurationMicroSec: u16,
    ),
    pub GetButtonIdNameFromEnum: UnusedFn,
    pub GetControllerAxisTypeNameFromEnum: UnusedFn,
    pub IsInputAvailable: unsafe extern "C" fn() -> bool,
    pub IsSteamVRDrawingControllers: UnusedFn,
    pub ShouldApplicationPause: UnusedFn,
    pub ShouldApplicationReduceRenderingWork: UnusedFn,
    pub PerformFirmwareUpdate: UnusedFn,
    pub AcknowledgeQuit_Exiting: unsafe extern "C" fn(),
    pub GetAppContainerFilePaths: UnusedFn,
    pub GetRuntimeVersion: unsafe extern "C" fn() -> *const c_char,
    pub SetSDKVersion: UnusedFn,
}

#[repr(C)]
pub struct VR_IVRApplications_FnTable {
    pub AddApplicationManifest: unsafe extern "C" fn(
        pchApplicationManifestFullPath: *const c_char,
        bTemporary: bool,
    ) -> EVRApplicationError,
    pub RemoveApplicationManifest:
        unsafe extern "C" fn(pchApplicationManifestFullPath: *const c_char) -> EVRApplicationError,
    pub IsApplicationInstalled: unsafe extern "C" fn(pchAppKey: *const c_char) -> bool,
    pub GetApplicationCount: UnusedFn,
    pub GetApplicationKeyByIndex: UnusedFn,
    pub GetApplicationKeyByProcessId: unsafe extern "C" fn(
        unProcessId: u32,
        pchAppKey: *mut c_char,
        unAppKeyBufferLen: u32,
    ) -> EVRApplicationError,
    pub LaunchApplication: UnusedFn,
    pub LaunchTemplateApplication: UnusedFn,
    pub LaunchApplicationFromMimeType: UnusedFn,
    pub LaunchDashboardOverlay: UnusedFn,
    pub CancelApplicationLaunch: UnusedFn,
    pub IdentifyApplication: UnusedFn,
    pub GetApplicationProcessId: UnusedFn,
    pub GetApplicationsErrorNameFromEnum:
        unsafe extern "C" fn(error: EVRApplicationError) -> *const c_char,
    pub GetApplicationPropertyString: unsafe extern "C" fn(
        pchAppKey: *const c_char,
        eProperty: EVRApplicationProperty,
        pchPropertyValueBuffer: *mut c_char,
        unPropertyValueBufferLen: u32,
        peError: *mut EVRApplicationError,
    ) -> u32,
    pub GetApplicationPropertyBool: UnusedFn,
    pub GetApplicationPropertyUint64: UnusedFn,
    pub SetApplicationAutoLaunch:
        unsafe extern "C" fn(pchAppKey: *const c_char, bAutoLaunch: bool) -> EVRApplicationError,
    pub GetApplicationAutoLaunch: unsafe extern "C" fn(pchAppKey: *const c_char) -> bool,
    pub SetDefaultApplicationForMimeType: UnusedFn,
    pub GetDefaultApplicationForMimeType: UnusedFn,
    pub GetApplicationSupportedMimeTypes: UnusedFn,
    pub GetApplicationsThatSupportMimeType: UnusedFn,
    pub GetApplicationLaunchArguments: UnusedFn,
    pub GetStartingApplication: UnusedFn,
    pub GetSceneApplicationState: UnusedFn,
    pub PerformApplicationPrelaunchCheck: UnusedFn,
    pub GetSceneApplicationStateNameFromEnum: UnusedFn,
    pub LaunchInternalProcess: UnusedFn,
    pub RegisterSubprocess: UnusedFn,
    pub GetCurrentSceneProcessId: unsafe extern "C" fn() -> u32,
}

#[repr(C)]
pub struct VR_IVROverlay_FnTable {
    pub FindOverlay: unsafe extern "C" fn(
        pchOverlayKey: *const c_char,
        pOverlayHandle: *mut VROverlayHandle_t,
    ) -> EVROverlayError,
    pub CreateOverlay: unsafe extern "C" fn(
        pchOverlayKey: *const c_char,
        pchOverlayName: *const c_char,
        pOverlayHandle: *mut VROverlayHandle_t,
    ) -> EVROverlayError,
    pub CreateSubviewOverlay: UnusedFn,
    pub DestroyOverlay:
        unsafe extern "C" fn(ulOverlayHandle: VROverlayHandle_t) -> EVROverlayError,
    pub GetOverlayKey: UnusedFn,
    pub GetOverlayName: UnusedFn,
    pub SetOverlayName: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        pchName: *const c_char,
    ) -> EVROverlayError,
    pub GetOverlayImageData: UnusedFn,
    pub GetOverlayErrorNameFromEnum:
        unsafe extern "C" fn(error: EVROverlayError) -> *const c_char,
    pub SetOverlayRenderingPid: UnusedFn,
    pub GetOverlayRenderingPid: UnusedFn,
    pub SetOverlayFlag: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        eOverlayFlag: VROverlayFlags,
        bEnabled: bool,
    ) -> EVROverlayError,
    pub GetOverlayFlag: UnusedFn,
    pub GetOverlayFlags: UnusedFn,
    pub SetOverlayColor: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        fRed: f32,
        fGreen: f32,
        fBlue: f32,
    ) -> EVROverlayError,
    pub GetOverlayColor: UnusedFn,
    pub SetOverlayAlpha:
        unsafe extern "C" fn(ulOverlayHandle: VROverlayHandle_t, fAlpha: f32) -> EVROverlayError,
    pub GetOverlayAlpha: UnusedFn,
    pub SetOverlayTexelAspect: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        fTexelAspect: f32,
    ) -> EVROverlayError,
    pub GetOverlayTexelAspect: UnusedFn,
    pub SetOverlaySortOrder: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        unSortOrder: u32,
    ) -> EVROverlayError,
    pub GetOverlaySortOrder: UnusedFn,
    pub SetOverlayWidthInMeters: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        fWidthInMeters: f32,
    ) -> EVROverlayError,
    pub GetOverlayWidthInMeters: UnusedFn,
    pub SetOverlayCurvature: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        fCurvature: f32,
    ) -> EVROverlayError,
    pub GetOverlayCurvature: UnusedFn,
    pub SetOverlayPreCurvePitch: UnusedFn,
    pub GetOverlayPreCurvePitch: UnusedFn,
    pub SetOverlayTextureColorSpace: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        eTextureColorSpace: EColorSpace,
    ) -> EVROverlayError,
    pub GetOverlayTextureColorSpace: UnusedFn,
    pub SetOverlayTextureBounds: UnusedFn,
    pub GetOverlayTextureBounds: UnusedFn,
    pub GetOverlayTransformType: UnusedFn,
    pub SetOverlayTransformAbsolute: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        eTrackingOrigin: ETrackingUniverseOrigin,
        pmatTrackingOriginToOverlayTransform: *const HmdMatrix34_t,
    ) -> EVROverlayError,
    pub GetOverlayTransformAbsolute: UnusedFn,
    pub SetOverlayTransformTrackedDeviceRelative: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        unTrackedDevice: TrackedDeviceIndex_t,
        pmatTrackedDeviceToOverlayTransform: *const HmdMatrix34_t,
    ) -> EVROverlayError,
    pub GetOverlayTransformTrackedDeviceRelative: UnusedFn,
    pub SetOverlayTransformTrackedDeviceComponent: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        unDeviceIndex: TrackedDeviceIndex_t,
        pchComponentName: *const c_char,
    ) -> EVROverlayError,
    pub GetOverlayTransformTrackedDeviceComponent: UnusedFn,
    pub SetOverlayTransformCursor: UnusedFn,
    pub GetOverlayTransformCursor: UnusedFn,
    pub SetOverlayTransformProjection: UnusedFn,
    pub SetSubviewPosition: UnusedFn,
    pub ShowOverlay: unsafe extern "C" fn(ulOverlayHandle: VROverlayHandle_t) -> EVROverlayError,
    pub HideOverlay: unsafe extern "C" fn(ulOverlayHandle: VROverlayHandle_t) -> EVROverlayError,
    pub IsOverlayVisible: unsafe extern "C" fn(ulOverlayHandle: VROverlayHandle_t) -> bool,
    pub GetTransformForOverlayCoordinates: UnusedFn,
    pub WaitFrameSync: unsafe extern "C" fn(nTimeoutMs: u32) -> EVROverlayError,
    pub PollNextOverlayEvent: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        pEvent: *mut VREvent_t,
        uncbVREvent: u32,
    ) -> bool,
    pub GetOverlayInputMethod: UnusedFn,
    pub SetOverlayInputMethod: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        eInputMethod: VROverlayInputMethod,
    ) -> EVROverlayError,
    pub GetOverlayMouseScale: UnusedFn,
    pub SetOverlayMouseScale: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        pvecMouseScale: *const HmdVector2_t,
    ) -> EVROverlayError,
    pub ComputeOverlayIntersection: UnusedFn,
    pub IsHoverTargetOverlay: UnusedFn,
    pub SetOverlayIntersectionMask: UnusedFn,
    pub TriggerLaserMouseHapticVibration: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        fDurationSeconds: f32,
        fFrequency: f32,
        fAmplitude: f32,
    ) -> EVROverlayError,
    pub SetOverlayCursor: UnusedFn,
    pub SetOverlayCursorPositionOverride: UnusedFn,
    pub ClearOverlayCursorPositionOverride: UnusedFn,
    pub SetOverlayTexture: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        pTexture: *const Texture_t,
    ) -> EVROverlayError,
    pub ClearOverlayTexture:
        unsafe extern "C" fn(ulOverlayHandle: VROverlayHandle_t) -> EVROverlayError,
    pub SetOverlayRaw: unsafe extern "C" fn(
        ulOverlayHandle: VROverlayHandle_t,
        pvBuffer: *const c_void,
        unWidth: u32,
        unHeight: u32,
        unBytesPerPixel: u32,
    ) -> EVROverlayError,
    pub SetOverlayFromFile: UnusedFn,
    pub GetOverlayTexture: UnusedFn,
    pub ReleaseNativeOverlayHandle: UnusedFn,
    pub GetOverlayTextureSize: UnusedFn,
    pub CreateDashboardOverlay: unsafe extern "C" fn(
        pchOverlayKey: *const c_char,
        pchOverlayFriendlyName: *const c_char,
        pMainHandle: *mut VROverlayHandle_t,
        pThumbnailHandle: *mut VROverlayHandle_t,
    ) -> EVROverlayError,
    pub IsDashboardVisible: unsafe extern "C" fn() -> bool,
    pub IsActiveDashboardOverlay:
        unsafe extern "C" fn(ulOverlayHandle: VROverlayHandle_t) -> bool,
    pub SetDashboardOverlaySceneProcess: UnusedFn,
    pub GetDashboardOverlaySceneProcess: UnusedFn,
    pub ShowDashboard: unsafe extern "C" fn(pchOverlayToShow: *const c_char),
    pub GetPrimaryDashboardDevice: UnusedFn,
    pub ShowKeyboard: UnusedFn,
    pub ShowKeyboardForOverlay: UnusedFn,
    pub GetKeyboardText: UnusedFn,
    pub HideKeyboard: UnusedFn,
    pub SetKeyboardTransformAbsolute: UnusedFn,
    pub SetKeyboardPositionForOverlay: UnusedFn,
    pub ShowMessageOverlay: UnusedFn,
    pub CloseMessageOverlay: UnusedFn,
}

pub struct OpenVrLib {

    _lib: libloading::Library,

    pub path: PathBuf,
    pub VR_InitInternal2: unsafe extern "C" fn(
        peError: *mut EVRInitError,
        eType: EVRApplicationType,
        pStartupInfo: *const c_char,
    ) -> isize,
    pub VR_ShutdownInternal: unsafe extern "C" fn(),
    pub VR_IsHmdPresent: unsafe extern "C" fn() -> bool,
    pub VR_IsRuntimeInstalled: unsafe extern "C" fn() -> bool,
    pub VR_GetGenericInterface: unsafe extern "C" fn(
        pchInterfaceVersion: *const c_char,
        peError: *mut EVRInitError,
    ) -> isize,
    pub VR_GetVRInitErrorAsEnglishDescription:
        unsafe extern "C" fn(error: EVRInitError) -> *const c_char,
    pub VR_GetVRInitErrorAsSymbol: unsafe extern "C" fn(error: EVRInitError) -> *const c_char,
}

unsafe impl Send for OpenVrLib {}
unsafe impl Sync for OpenVrLib {}

#[derive(Debug)]
pub enum LoadError {
    NotFound {
        searched: String,
    },
    Load {
        path: PathBuf,
        source: libloading::Error,
    },
    MissingSymbol {
        path: PathBuf,
        symbol: &'static str,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::NotFound { searched } => write!(
                f,
                "could not find openvr_api.dll.
                 Looked in: {searched}
                 Install SteamVR, or place openvr_api.dll next to this executable, or set                  the OPENVR_API_DLL environment variable to its full path."
            ),
            LoadError::Load { path, source } => {
                write!(f, "failed to load {}: {source}", path.display())
            }
            LoadError::MissingSymbol { path, symbol } => write!(
                f,
                "{} is missing the exported symbol `{symbol}` (is this really openvr_api.dll?)",
                path.display()
            ),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LoadError::Load { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl OpenVrLib {

    pub fn load() -> Result<Self, LoadError> {
        let candidates = dll_candidates();
        let mut last_err: Option<LoadError> = None;

        for path in &candidates {
            match Self::load_from(path) {
                Ok(lib) => return Ok(lib),
                Err(e @ LoadError::MissingSymbol { .. }) => return Err(e),
                Err(e) => {
                    log::debug!("openvr_api.dll candidate {} rejected: {e}", path.display());
                    last_err = Some(e);
                }
            }
        }

        let searched = candidates
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Err(last_err.unwrap_or(LoadError::NotFound { searched }))
    }

    fn load_from(path: &Path) -> Result<Self, LoadError> {

        let lib = unsafe { libloading::Library::new(path) }.map_err(|source| LoadError::Load {
            path: path.to_path_buf(),
            source,
        })?;

        macro_rules! sym {
            ($name:literal, $ty:ty) => {{
                let s: libloading::Symbol<$ty> = unsafe { lib.get(concat!($name, "\0").as_bytes()) }
                    .map_err(|_| LoadError::MissingSymbol {
                        path: path.to_path_buf(),
                        symbol: $name,
                    })?;
                *s
            }};
        }

        Ok(Self {
            path: path.to_path_buf(),
            VR_InitInternal2: sym!(
                "VR_InitInternal2",
                unsafe extern "C" fn(*mut EVRInitError, EVRApplicationType, *const c_char) -> isize
            ),
            VR_ShutdownInternal: sym!("VR_ShutdownInternal", unsafe extern "C" fn()),
            VR_IsHmdPresent: sym!("VR_IsHmdPresent", unsafe extern "C" fn() -> bool),
            VR_IsRuntimeInstalled: sym!("VR_IsRuntimeInstalled", unsafe extern "C" fn() -> bool),
            VR_GetGenericInterface: sym!(
                "VR_GetGenericInterface",
                unsafe extern "C" fn(*const c_char, *mut EVRInitError) -> isize
            ),
            VR_GetVRInitErrorAsEnglishDescription: sym!(
                "VR_GetVRInitErrorAsEnglishDescription",
                unsafe extern "C" fn(EVRInitError) -> *const c_char
            ),
            VR_GetVRInitErrorAsSymbol: sym!(
                "VR_GetVRInitErrorAsSymbol",
                unsafe extern "C" fn(EVRInitError) -> *const c_char
            ),
            _lib: lib,
        })
    }

    pub fn init_error_message(&self, err: EVRInitError) -> String {

        unsafe {
            let sym = cstr_to_string((self.VR_GetVRInitErrorAsSymbol)(err));
            let desc = cstr_to_string((self.VR_GetVRInitErrorAsEnglishDescription)(err));
            format!("{desc} ({sym}, code {err})")
        }
    }
}

pub unsafe fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned()
    }
}

fn dll_candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if !out.contains(&p) {
            out.push(p);
        }
    };

    if let Some(p) = std::env::var_os("OPENVR_API_DLL") {
        push(PathBuf::from(p));
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        push(dir.join(DLL_NAME));
    }

    for runtime in registered_runtimes() {
        push(runtime.join("bin").join(DLL_ARCH_DIR).join(DLL_NAME));
        push(runtime.join(DLL_NAME));
    }

    push(PathBuf::from(DLL_NAME));

    out
}

#[cfg(windows)]
const DLL_NAME: &str = "openvr_api.dll";
#[cfg(all(unix, not(target_os = "macos")))]
const DLL_NAME: &str = "libopenvr_api.so";
#[cfg(target_os = "macos")]
const DLL_NAME: &str = "libopenvr_api.dylib";

#[cfg(windows)]
const DLL_ARCH_DIR: &str = if cfg!(target_pointer_width = "64") {
    "win64"
} else {
    "win32"
};
#[cfg(all(unix, not(target_os = "macos")))]
const DLL_ARCH_DIR: &str = "linux64";
#[cfg(target_os = "macos")]
const DLL_ARCH_DIR: &str = "osx64";

fn registered_runtimes() -> Vec<PathBuf> {
    let Some(path) = openvrpaths_file() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        log::debug!("{} not readable", path.display());
        return Vec::new();
    };
    let text = text.trim_start_matches('\u{feff}');
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) => v
            .get("runtime")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.as_str())
                    .map(PathBuf::from)
                    .collect()
            })
            .unwrap_or_default(),
        Err(e) => {
            log::debug!("{} is not valid JSON: {e}", path.display());
            Vec::new()
        }
    }
}

fn openvrpaths_file() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let local = std::env::var_os("LOCALAPPDATA")?;
        Some(PathBuf::from(local).join("openvr").join("openvrpaths.vrpath"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join(".config")
                .join("openvr")
                .join("openvrpaths.vrpath"),
        )
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("OpenVR")
                .join(".openvr")
                .join("openvrpaths.vrpath"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn abi_struct_sizes_match_openvr() {
        assert_eq!(size_of::<HmdMatrix34_t>(), 48);
        assert_eq!(size_of::<TrackedDevicePose_t>(), 80);
        assert_eq!(size_of::<VREvent_t>(), 64);
        assert_eq!(size_of::<Texture_t>(), 16);
        assert_eq!(size_of::<VREvent_Mouse_t>(), 12);
    }

    #[test]
    fn the_mouse_arm_fits_inside_the_event_union() {
        assert!(size_of::<VREvent_Mouse_t>() <= size_of::<VREvent_Data_t>());
    }

    #[test]
    fn a_mouse_event_reads_back_the_bytes_steamvr_wrote() {
        let mut event = VREvent_t {
            eventType: VREvent_MouseButtonDown,
            ..Default::default()
        };
        let written = VREvent_Mouse_t {
            x: 640.5,
            y: 12.25,
            button: VRMouseButton_Left,
        };
        unsafe {
            std::ptr::copy_nonoverlapping(
                &written as *const VREvent_Mouse_t as *const u8,
                &mut event.data as *mut VREvent_Data_t as *mut u8,
                size_of::<VREvent_Mouse_t>(),
            );
        }

        let read = event.mouse();
        assert_eq!(read.x, 640.5);
        assert_eq!(read.y, 12.25);
        assert_eq!(read.button, VRMouseButton_Left);
    }

    #[test]
    fn fn_table_entry_counts_match_openvr() {
        let p = size_of::<usize>();
        assert_eq!(size_of::<VR_IVRSystem_FnTable>() / p, 51);
        assert_eq!(size_of::<VR_IVRApplications_FnTable>() / p, 31);
        assert_eq!(size_of::<VR_IVROverlay_FnTable>() / p, 82);
    }
}
