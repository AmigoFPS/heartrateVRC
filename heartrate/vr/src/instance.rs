
pub struct InstanceLock {
    #[cfg(windows)]
    handle: windows::Win32::Foundation::HANDLE,
}

impl InstanceLock {
    #[cfg(windows)]
    pub fn acquire(key: &str) -> Option<Self> {
        use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
        use windows::Win32::System::Threading::CreateMutexW;
        use windows::core::PCWSTR;

        let name: Vec<u16> = format!("Local\\heartrate-osc.{key}\0")
            .encode_utf16()
            .collect();

        let handle = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) }.ok()?;

        let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        if existed {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return None;
        }

        Some(Self { handle })
    }

    #[cfg(not(windows))]
    pub fn acquire(_key: &str) -> Option<Self> {
        Some(Self {})
    }
}

#[cfg(windows)]
impl Drop for InstanceLock {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_lock_on_the_same_key_is_refused() {
        let key = "test.second.lock.refused";
        let first = InstanceLock::acquire(key).expect("the first copy should get it");
        assert!(
            InstanceLock::acquire(key).is_none(),
            "a second copy must not get the same key"
        );

        drop(first);
        assert!(
            InstanceLock::acquire(key).is_some(),
            "the key must be free again once the holder exits"
        );
    }

    #[test]
    fn different_keys_do_not_collide() {
        let a = InstanceLock::acquire("test.two.keys.a");
        let b = InstanceLock::acquire("test.two.keys.b");
        assert!(a.is_some() && b.is_some());
    }
}
