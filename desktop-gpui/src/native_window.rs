use gpui_kit::{App, Window};
use metra_lib::geometry::WorkArea;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub(super) fn work_area(window: &Window, cx: &App) -> Result<(WorkArea, f64), String> {
    let display = window.display(cx).ok_or("No display available")?;
    let bounds = display.visible_bounds();
    let scale = f64::from(window.scale_factor());
    Ok((
        WorkArea {
            x: f64::from(f32::from(bounds.origin.x)) * scale,
            y: f64::from(f32::from(bounds.origin.y)) * scale,
            width: f64::from(f32::from(bounds.size.width)) * scale,
            height: f64::from(f32::from(bounds.size.height)) * scale,
        },
        scale,
    ))
}
pub(super) fn position(window: &Window) -> Result<(i32, i32), String> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|e| e.to_string())?
        .as_raw();
    #[cfg(target_os = "linux")]
    if let RawWindowHandle::Xcb(handle) = handle {
        use x11rb::{connection::Connection, protocol::xproto::ConnectionExt};
        let (connection, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let result = connection
            .translate_coordinates(
                handle.window.get(),
                connection.setup().roots[screen].root,
                0,
                0,
            )
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?;
        return Ok((i32::from(result.dst_x), i32::from(result.dst_y)));
    }
    #[cfg(windows)]
    if let RawWindowHandle::Win32(handle) = handle {
        let mut rect = windows_sys::Win32::Foundation::RECT::default();
        // SAFETY: GPUI owns this live HWND; the rectangle is writable for the call.
        if unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(
                handle.hwnd.get() as _,
                &mut rect,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        return Ok((rect.left, rect.top));
    }
    #[cfg(target_os = "macos")]
    if let RawWindowHandle::AppKit(handle) = handle {
        use objc2::{MainThreadMarker, msg_send};
        use objc2_app_kit::{NSScreen, NSView};
        use objc2_foundation::NSRect;
        let mtm = MainThreadMarker::new().ok_or("Window access requires main thread")?;
        // SAFETY: the raw view belongs to the live GPUI window and is accessed on the main thread.
        unsafe {
            let view = &*(handle.ns_view.as_ptr() as *const NSView);
            let native = view.window().ok_or("Native window unavailable")?;
            let frame: NSRect = msg_send![&*native, frame];
            let screens = NSScreen::screens(mtm);
            let primary = screens.firstObject().ok_or("No display available")?;
            let top = primary.frame().size.height;
            let scale = f64::from(window.scale_factor());
            return Ok((
                (frame.origin.x * scale).round() as i32,
                ((top - frame.origin.y - frame.size.height) * scale).round() as i32,
            ));
        }
    }
    Err("Positioned overlays require a supported native window (X11/XWayland on Linux)".into())
}
pub(super) fn move_to(window: &Window, x: i32, y: i32) -> Result<(), String> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|e| e.to_string())?
        .as_raw();
    #[cfg(target_os = "linux")]
    if let RawWindowHandle::Xcb(handle) = handle {
        use x11rb::{
            connection::Connection,
            protocol::xproto::{ConfigureWindowAux, ConnectionExt},
        };
        let (connection, _) = x11rb::connect(None).map_err(|e| e.to_string())?;
        connection
            .configure_window(handle.window.get(), &ConfigureWindowAux::new().x(x).y(y))
            .map_err(|e| e.to_string())?
            .check()
            .map_err(|e| e.to_string())?;
        return connection.flush().map_err(|e| e.to_string());
    }
    #[cfg(windows)]
    if let RawWindowHandle::Win32(handle) = handle {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        // SAFETY: live HWND, no pointer arguments; preserve focus and dimensions.
        if unsafe {
            SetWindowPos(
                handle.hwnd.get() as _,
                HWND_TOPMOST,
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    if let RawWindowHandle::AppKit(handle) = handle {
        use objc2::{MainThreadMarker, msg_send};
        use objc2_app_kit::{NSScreen, NSView};
        use objc2_foundation::NSPoint;
        let mtm = MainThreadMarker::new().ok_or("Window access requires main thread")?;
        // SAFETY: live AppKit objects; position change occurs on the main thread.
        unsafe {
            let view = &*(handle.ns_view.as_ptr() as *const NSView);
            let native = view.window().ok_or("Native window unavailable")?;
            let screens = NSScreen::screens(mtm);
            let primary = screens.firstObject().ok_or("No display available")?;
            let scale = f64::from(window.scale_factor());
            let _: () = msg_send![&*native,setFrameTopLeftPoint:NSPoint::new(x as f64/scale,primary.frame().size.height-y as f64/scale)];
        }
        return Ok(());
    }
    Err("Positioned overlays require a supported native window (X11/XWayland on Linux)".into())
}
