//! Small platform boundaries shared by native application features.

#[cfg(target_os = "linux")]
pub(crate) fn prepare_linux_backend() {
    // Run before Tauri initializes GTK. Global window positioning and edge
    // snapping require X11; Ubuntu Wayland sessions provide it via XWayland.
    // Avoid changing process environment after worker threads have started.
    gdk::set_allowed_backends("x11");
}

#[cfg(target_os = "linux")]
pub(crate) fn primary_mouse_button_pressed() -> Result<bool, String> {
    use gdk::prelude::SeatExt;

    let display = gdk::Display::default().ok_or("Linux display is unavailable")?;
    let pointer = display
        .default_seat()
        .and_then(|seat| seat.pointer())
        .ok_or("Linux pointer is unavailable")?;
    let root = display
        .default_screen()
        .root_window()
        .ok_or("Linux root window is unavailable")?;
    // Query the root, including while the window manager owns the drag grab.
    // The returned child may be None over another application's window; the
    // modifier mask is still valid and must not be discarded in that case.
    let (_, _, _, modifiers) = root.device_position(&pointer);
    Ok(modifiers.contains(gdk::ModifierType::BUTTON1_MASK))
}

#[cfg(target_os = "linux")]
pub(crate) fn constrain_bubble_size(
    window: &tauri::WebviewWindow,
    width: u32,
    height: u32,
) -> tauri::Result<()> {
    // GTK's non-resizable mode uses WebKit's natural size (200x200 logical
    // pixels). Keep GTK resizable, but pin both bounds to the requested frame
    // so the WM cannot resize the bubble. Update atomically for peek/reveal.
    let width = tauri::PhysicalUnit::new(width).into();
    let height = tauri::PhysicalUnit::new(height).into();
    window.set_size_constraints(tauri::WindowSizeConstraints {
        min_width: Some(width),
        max_width: Some(width),
        min_height: Some(height),
        max_height: Some(height),
    })
}

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
