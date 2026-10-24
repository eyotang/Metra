//! The only OS-specific window and startup code. GPUI owns rendering and input.
use gpui_kit::{Pixels, Point, Window, point, px};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub fn move_window(window: &Window, position: Point<Pixels>) -> Result<(), String> {
    let raw = HasWindowHandle::window_handle(window)
        .map_err(|error| error.to_string())?
        .as_raw();
    match raw {
        #[cfg(target_os = "macos")]
        RawWindowHandle::AppKit(handle) => {
            use objc2_app_kit::{NSScreen, NSView};
            use objc2_foundation::{MainThreadMarker, NSPoint};
            let mtm = MainThreadMarker::new().ok_or("Window movement requires the UI thread")?;
            // SAFETY: GPUI supplies a live NSView for this window on the main thread.
            let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
            let native = view.window().ok_or("Native window unavailable")?;
            let screens = NSScreen::screens(mtm);
            let height = screens
                .firstObject()
                .map(|screen| screen.frame().size.height)
                .unwrap_or(0.0);
            native.setFrameTopLeftPoint(NSPoint::new(
                f64::from(f32::from(position.x)),
                height - f64::from(f32::from(position.y)),
            ));
            Ok(())
        }
        #[cfg(target_os = "windows")]
        RawWindowHandle::Win32(handle) => {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SetWindowPos,
            };
            let scale = window.scale_factor();
            // SAFETY: the HWND is borrowed from a live GPUI window; no ownership is transferred.
            let success = unsafe {
                SetWindowPos(
                    handle.hwnd.get() as _,
                    HWND_TOPMOST,
                    (f32::from(position.x) * scale).round() as i32,
                    (f32::from(position.y) * scale).round() as i32,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE,
                )
            };
            if success == 0 {
                Err(std::io::Error::last_os_error().to_string())
            } else {
                Ok(())
            }
        }
        #[cfg(target_os = "linux")]
        RawWindowHandle::Xcb(handle) => {
            use x11rb::{
                connection::Connection,
                protocol::xproto::{ConfigureWindowAux, ConnectionExt},
            };
            let (connection, _) = x11rb::connect(None).map_err(|error| error.to_string())?;
            let scale = window.scale_factor();
            connection
                .configure_window(
                    handle.window.get(),
                    &ConfigureWindowAux::new()
                        .x((f32::from(position.x) * scale).round() as i32)
                        .y((f32::from(position.y) * scale).round() as i32),
                )
                .map_err(|error| error.to_string())?
                .check()
                .map_err(|error| error.to_string())?;
            connection.flush().map_err(|error| error.to_string())
        }
        _ => Err("This display server does not allow moving overlay windows".into()),
    }
}

pub fn save_position(window: &Window) -> Point<Pixels> {
    window.bounds().origin
}
pub fn position(x: i32, y: i32) -> Point<Pixels> {
    point(px(x as f32), px(y as f32))
}

pub fn set_autostart(enabled: bool) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let path = executable
        .to_str()
        .ok_or("Application path is not valid Unicode")?;
    let launch = auto_launch::AutoLaunchBuilder::new()
        .set_app_name("Metra")
        .set_app_path(path)
        .set_use_launch_agent(true)
        .set_args(&["--autostart"])
        .build()
        .map_err(|error| error.to_string())?;
    if enabled {
        launch.enable()
    } else {
        launch.disable()
    }
    .map_err(|error| error.to_string())
}

pub fn primary_button_pressed() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        #[link(name = "CoreGraphics", kind = "framework")]
        unsafe extern "C" {
            fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
        }
        // SAFETY: reads combined-session primary button state without creating native objects.
        Ok(unsafe { CGEventSourceButtonState(0, 0) })
    }
    #[cfg(windows)]
    {
        // SAFETY: queries the fixed VK_LBUTTON key; does not modify input state.
        Ok(unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(1) } < 0)
    }
    #[cfg(target_os = "linux")]
    {
        use x11rb::{
            connection::Connection,
            protocol::xproto::{ConnectionExt, KeyButMask},
        };
        let (connection, screen) = x11rb::connect(None).map_err(|error| error.to_string())?;
        let pointer = connection
            .query_pointer(connection.setup().roots[screen].root)
            .map_err(|error| error.to_string())?
            .reply()
            .map_err(|error| error.to_string())?;
        Ok(pointer.mask.contains(KeyButMask::BUTTON1))
    }
}

/// Convert the physical coordinates returned by the tray and legacy settings.
pub fn logical_screen_point(x: f32, y: f32) -> Point<Pixels> {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSScreen;
        use objc2_foundation::MainThreadMarker;
        if let Some(mtm) = MainThreadMarker::new() {
            let screens = NSScreen::screens(mtm);
            let height = screens
                .firstObject()
                .map(|screen| screen.frame().size.height)
                .unwrap_or(0.);
            for screen in screens.iter() {
                let frame = screen.frame();
                let scale = screen.backingScaleFactor();
                let left = frame.origin.x * scale;
                let top = (height - frame.origin.y - frame.size.height) * scale;
                if f64::from(x) >= left
                    && f64::from(x) < left + frame.size.width * scale
                    && f64::from(y) >= top
                    && f64::from(y) < top + frame.size.height * scale
                {
                    return point(px(x / scale as f32), px(y / scale as f32));
                }
            }
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::POINT,
            Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromPoint},
            UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        };
        let (mut dpi_x, mut dpi_y) = (96, 96);
        // SAFETY: queries monitor DPI for fixed coordinates; the output pointers are valid locals.
        unsafe {
            let monitor = MonitorFromPoint(
                POINT {
                    x: x as i32,
                    y: y as i32,
                },
                MONITOR_DEFAULTTONEAREST,
            );
            if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == 0 {
                return point(
                    px(x * 96. / dpi_x.max(1) as f32),
                    px(y * 96. / dpi_y.max(1) as f32),
                );
            }
        }
    }
    point(px(x), px(y))
}
