//! Small platform boundaries shared by native application features.

#[cfg(target_os = "windows")]
/// Opens a backend-owned URL through the registered Windows URL handler.
pub(crate) fn open_url(url: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

    let operation = "open\0".encode_utf16().collect::<Vec<_>>();
    let target = url
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: ShellExecuteW only borrows these NUL-terminated UTF-16 buffers
    // for the duration of the call. Calling the API directly avoids shell and
    // PATH lookup, which matters when Portable runs from a writable folder.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    let status = result as isize;
    if status > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW failed with status {status}"))
    }
}
