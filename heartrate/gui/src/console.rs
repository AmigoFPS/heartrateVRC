#[cfg(windows)]
pub fn attach_parent() -> bool {
    use windows::{
        Win32::{
            Foundation::INVALID_HANDLE_VALUE,
            Storage::FileSystem::{
                CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            },
            System::Console::{
                ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_ERROR_HANDLE, STD_HANDLE, STD_INPUT_HANDLE,
                STD_OUTPUT_HANDLE, SetStdHandle,
            },
        },
        core::w,
    };

    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;

    let open = |name| unsafe {
        CreateFileW(
            name,
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };

    let missing = |which: STD_HANDLE| match unsafe { GetStdHandle(which) } {
        Ok(handle) => handle.is_invalid(),
        Err(_) => true,
    };

    let had_output = !missing(STD_OUTPUT_HANDLE);

    if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_err() && !had_output {
        return false;
    }

    if missing(STD_OUTPUT_HANDLE) || missing(STD_ERROR_HANDLE) {
        if let Ok(out) = open(w!("CONOUT$")) {
            if out != INVALID_HANDLE_VALUE {
                for which in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
                    if missing(which) {
                        unsafe { SetStdHandle(which, out) }.ok();
                    }
                }
            }
        }
    }

    if missing(STD_INPUT_HANDLE)
        && let Ok(input) = open(w!("CONIN$"))
        && input != INVALID_HANDLE_VALUE
    {
        unsafe { SetStdHandle(STD_INPUT_HANDLE, input) }.ok();
    }

    !missing(STD_OUTPUT_HANDLE)
}

#[cfg(not(windows))]
pub fn attach_parent() -> bool {
    true
}
