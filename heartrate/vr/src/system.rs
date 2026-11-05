use crate::sys::{self, VR_IVRSystem_FnTable};

pub struct System {
    table: *const VR_IVRSystem_FnTable,
}

impl System {
    pub(crate) fn new(table: *const VR_IVRSystem_FnTable) -> Self {
        Self { table }
    }

    #[inline]
    fn t(&self) -> &VR_IVRSystem_FnTable {
        unsafe { &*self.table }
    }

    pub fn runtime_version(&self) -> String {
        unsafe { sys::cstr_to_string((self.t().GetRuntimeVersion)()) }
    }

    pub fn poll_event(&self) -> Option<sys::VREvent_t> {
        let mut event = sys::VREvent_t::default();
        let got = unsafe { (self.t().PollNextEvent)(&mut event, std::mem::size_of::<sys::VREvent_t>() as u32) };
        got.then_some(event)
    }

    pub fn event_name(&self, event_type: u32) -> String {
        let name = unsafe { sys::cstr_to_string((self.t().GetEventTypeNameFromEnum)(event_type)) };
        if name.is_empty() {
            format!("event {event_type}")
        } else {
            name
        }
    }

    pub fn acknowledge_quit(&self) {
        unsafe { (self.t().AcknowledgeQuit_Exiting)() };
    }

    pub fn dxgi_adapter_index(&self) -> Option<u32> {
        let mut index: i32 = -1;
        unsafe { (self.t().GetDXGIOutputInfo)(&mut index) };
        (index >= 0).then_some(index as u32)
    }
}
