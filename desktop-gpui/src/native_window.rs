use gpui_kit::{App, Window};
use metra_lib::geometry::WorkArea;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Enumerate physical work areas instead of GPUI's X11 screen (which can span
/// several monitors). Keep the per-monitor scale for legacy logical positions.
fn monitor_areas(window: &Window, cx: &App) -> Result<Vec<(WorkArea, f64)>, String> {
    #[cfg(target_os = "linux")]
    {
        use x11rb::{
            connection::Connection,
            protocol::{
                randr::ConnectionExt as _,
                xproto::{AtomEnum, ConnectionExt as _},
            },
        };
        let (connection, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let root = connection.setup().roots[screen].root;
        let scale = f64::from(window.scale_factor());
        let mut areas = Vec::new();
        if let Ok(cookie) = connection.randr_get_monitors(root, true) {
            if let Ok(reply) = cookie.reply() {
                // EWMH reports a desktop work area excluding panels/docks. Some
                // compositors omit it; physical monitor bounds remain usable.
                let work = (|| {
                    let atom = connection
                        .intern_atom(true, b"_NET_WORKAREA")
                        .ok()?
                        .reply()
                        .ok()?
                        .atom;
                    let reply = connection
                        .get_property(false, root, atom, AtomEnum::CARDINAL, 0, 4)
                        .ok()?
                        .reply()
                        .ok()?;
                    let values: Vec<_> = reply.value32()?.collect();
                    (values.len() == 4).then(|| WorkArea {
                        x: values[0] as i32 as f64,
                        y: values[1] as i32 as f64,
                        width: values[2] as f64,
                        height: values[3] as f64,
                    })
                })();
                for monitor in reply.monitors {
                    let mut area = WorkArea {
                        x: monitor.x as f64,
                        y: monitor.y as f64,
                        width: monitor.width as f64,
                        height: monitor.height as f64,
                    };
                    if let Some(work) = work {
                        let x = area.x.max(work.x);
                        let y = area.y.max(work.y);
                        let right = (area.x + area.width).min(work.x + work.width);
                        let bottom = (area.y + area.height).min(work.y + work.height);
                        if right > x && bottom > y {
                            area = WorkArea {
                                x,
                                y,
                                width: right - x,
                                height: bottom - y,
                            };
                        }
                    }
                    if monitor.primary {
                        areas.insert(0, (area, scale));
                    } else {
                        areas.push((area, scale));
                    }
                }
            }
        }
        if !areas.is_empty() {
            return Ok(areas);
        }
    }
    #[cfg(target_os = "macos")]
    {
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSScreen;
        let mtm = MainThreadMarker::new().ok_or("Window access requires main thread")?;
        let screens = NSScreen::screens(mtm);
        let primary = screens.firstObject().ok_or("No display available")?;
        let top = primary.frame().size.height;
        let mut areas = Vec::new();
        for screen in screens.iter() {
            let visible = screen.visibleFrame();
            let scale = screen.backingScaleFactor();
            areas.push((
                WorkArea {
                    x: visible.origin.x * scale,
                    y: (top - visible.origin.y - visible.size.height) * scale,
                    width: visible.size.width * scale,
                    height: visible.size.height * scale,
                },
                scale,
            ));
        }
        return Ok(areas);
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::{
            Foundation::{LPARAM, RECT},
            Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO},
            UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        };
        use windows_sys::core::BOOL;
        unsafe extern "system" fn visit(
            monitor: HMONITOR,
            _: HDC,
            _: *mut RECT,
            context: LPARAM,
        ) -> BOOL {
            // SAFETY: EnumDisplayMonitors synchronously passes our live vector;
            // each Win32 output structure has the required size and lifetime.
            unsafe {
                let areas = &mut *(context as *mut Vec<(WorkArea, f64)>);
                let mut info = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                if GetMonitorInfoW(monitor, &mut info) != 0 {
                    let (mut xdpi, mut ydpi) = (96, 96);
                    let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut xdpi, &mut ydpi);
                    let r = info.rcWork;
                    let entry = (
                        WorkArea {
                            x: r.left as f64,
                            y: r.top as f64,
                            width: (r.right - r.left) as f64,
                            height: (r.bottom - r.top) as f64,
                        },
                        xdpi as f64 / 96.,
                    );
                    if info.dwFlags & 1 != 0 {
                        areas.insert(0, entry);
                    } else {
                        areas.push(entry);
                    }
                }
                1
            }
        }
        let mut areas: Vec<(WorkArea, f64)> = Vec::new();
        // SAFETY: callback and output vector live for this synchronous call.
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(visit),
                &mut areas as *mut _ as isize,
            );
        }
        if !areas.is_empty() {
            return Ok(areas);
        }
    }
    #[allow(unreachable_code)]
    {
        let display = window
            .display(cx)
            .or_else(|| cx.primary_display())
            .ok_or("No display available")?;
        let bounds = display.visible_bounds();
        let scale = f64::from(window.scale_factor());
        Ok(vec![(
            WorkArea {
                x: f64::from(f32::from(bounds.origin.x)) * scale,
                y: f64::from(f32::from(bounds.origin.y)) * scale,
                width: f64::from(f32::from(bounds.size.width)) * scale,
                height: f64::from(f32::from(bounds.size.height)) * scale,
            },
            scale,
        )])
    }
}

pub(super) fn work_area(window: &Window, cx: &App) -> Result<(WorkArea, f64), String> {
    let areas = monitor_areas(window, cx)?;
    position(window)
        .ok()
        .and_then(|(x, y)| metra_lib::geometry::nearby_work_area(&areas, x as f64, y as f64))
        .or_else(|| areas.first().copied())
        .ok_or("No display available".into())
}

pub(super) fn initial_work_area(
    window: &Window,
    cx: &App,
    settings: &metra_lib::settings::AppSettings,
) -> Result<(WorkArea, f64), String> {
    let areas = monitor_areas(window, cx)?;
    metra_lib::geometry::saved_work_area(
        &areas,
        settings.bubble_position,
        settings.bubble_position_version,
    )
    .ok_or("No display available".into())
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
        let mut origin = windows_sys::Win32::Foundation::POINT::default();
        // Geometry uses GPUI's content size, so its origin must also be the
        // client origin. Popup HWNDs can retain invisible non-client margins.
        // SAFETY: live HWND and writable POINT initialized to client (0,0).
        if unsafe {
            windows_sys::Win32::Graphics::Gdi::ClientToScreen(handle.hwnd.get() as _, &mut origin)
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        return Ok((origin.x, origin.y));
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
        use windows_sys::Win32::{
            Foundation::{POINT, RECT},
            Graphics::Gdi::ClientToScreen,
        };
        for _ in 0..2 {
            let mut outer = RECT::default();
            let mut client = POINT::default();
            // SAFETY: GPUI owns the live HWND; output structures are writable.
            unsafe {
                if GetWindowRect(handle.hwnd.get() as _, &mut outer) == 0
                    || ClientToScreen(handle.hwnd.get() as _, &mut client) == 0
                {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                if SetWindowPos(
                    handle.hwnd.get() as _,
                    HWND_TOPMOST,
                    x - (client.x - outer.left),
                    y - (client.y - outer.top),
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE,
                ) == 0
                {
                    return Err(std::io::Error::last_os_error().to_string());
                }
            }
            // Re-read once after a possible per-monitor DPI transition.
            if position(window)? == (x, y) {
                break;
            }
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
