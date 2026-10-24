//! The only OS-specific window and startup code. GPUI owns rendering and input.
use gpui_kit::{Pixels, Point, Window, point, px};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Match the native blur surface to the GPUI overlay, including transparent corners.
pub fn finish_overlay_material(window: &Window, radius: Pixels) -> Result<(), String> {
    let raw = match HasWindowHandle::window_handle(window) {
        Ok(handle) => handle.as_raw(),
        // Headless test windows have no native surface.
        Err(raw_window_handle::HandleError::NotSupported) => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    match raw {
        #[cfg(target_os = "macos")]
        RawWindowHandle::AppKit(handle) => {
            use objc2_app_kit::{
                NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSColor, NSView,
            };
            use objc2_foundation::MainThreadMarker;
            let _mtm = MainThreadMarker::new().ok_or("Overlay material requires the UI thread")?;
            // SAFETY: GPUI supplies the live NSView; AppKit is accessed on its main thread.
            let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
            let native = view.window().ok_or("Native window unavailable")?;
            // Clear AppKit's backing as well as GPUI's root; both participate in compositing.
            native.setOpaque(false);
            native.setBackgroundColor(Some(&NSColor::clearColor()));
            let content = native.contentView().ok_or("Native content unavailable")?;
            content.setWantsLayer(true);
            let layer = content.layer().ok_or("Native content layer unavailable")?;
            layer.setOpaque(false);
            layer.setBackgroundColor(None);
            layer.setCornerRadius(f64::from(f32::from(radius)));
            layer.setMasksToBounds(true);
            // SAFETY: this is AppKit's immutable, process-lifetime appearance name.
            let appearance = NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua });
            native.setAppearance(appearance.as_deref());
            native.setHasShadow(false);
            Ok(())
        }
        #[cfg(target_os = "windows")]
        RawWindowHandle::Win32(handle) => {
            use windows_sys::Win32::Graphics::Gdi::{
                CreateRoundRectRgn, DeleteObject, SetWindowRgn,
            };
            let scale = window.scale_factor();
            let size = window.bounds().size;
            let width = (f32::from(size.width) * scale).round().max(1.) as i32;
            let height = (f32::from(size.height) * scale).round().max(1.) as i32;
            let diameter = (f32::from(radius) * scale * 2.).round() as i32;
            // SAFETY: the HWND is borrowed from GPUI. Windows owns the region after success.
            unsafe {
                let region = CreateRoundRectRgn(0, 0, width, height, diameter, diameter);
                if region.is_null() {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                if SetWindowRgn(handle.hwnd.get() as _, region, 1) == 0 {
                    let error = std::io::Error::last_os_error();
                    DeleteObject(region);
                    return Err(error.to_string());
                }
            }
            Ok(())
        }
        #[cfg(target_os = "linux")]
        RawWindowHandle::Xcb(handle) => {
            use x11rb::{
                connection::Connection,
                protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode},
                wrapper::ConnectionExt as _,
            };
            let (connection, _) = x11rb::connect(None).map_err(|error| error.to_string())?;
            let atom = connection
                .intern_atom(false, b"_KDE_NET_WM_BLUR_BEHIND_REGION")
                .map_err(|error| error.to_string())?
                .reply()
                .map_err(|error| error.to_string())?
                .atom;
            let scale = window.scale_factor();
            let size = window.bounds().size;
            let region = rounded_blur_region(
                (f32::from(size.width) * scale).round().max(1.) as u32,
                (f32::from(size.height) * scale).round().max(1.) as u32,
                f64::from(f32::from(radius) * scale),
            );
            // KDE's X11 blur hint uses device-pixel rectangles: x, y, width, height.
            connection
                .change_property32(
                    PropMode::REPLACE,
                    handle.window.get(),
                    atom,
                    AtomEnum::CARDINAL,
                    &region,
                )
                .map_err(|error| error.to_string())?
                .check()
                .map_err(|error| error.to_string())?;
            connection.flush().map_err(|error| error.to_string())
        }
        _ => Ok(()),
    }
}

#[cfg(any(target_os = "linux", test))]
fn rounded_blur_region(width: u32, height: u32, radius: f64) -> Vec<u32> {
    let radius = radius.max(0.).min(f64::from(width.min(height)) / 2.);
    if radius == 0. {
        return vec![0, 0, width, height];
    }
    let mut region = Vec::with_capacity(height as usize * 4);
    for y in 0..height {
        let row = f64::from(y) + 0.5;
        let distance = (radius - row)
            .max(row - (f64::from(height) - radius))
            .max(0.);
        // Include a device pixel when its centre is inside the rounded outline.
        let inset = (radius - (radius * radius - distance * distance).max(0.).sqrt() - 0.5)
            .ceil()
            .max(0.) as u32;
        region.extend_from_slice(&[inset, y, width.saturating_sub(inset * 2), 1]);
    }
    region
}

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

#[cfg(test)]
mod material_tests {
    use super::rounded_blur_region;

    #[test]
    fn blur_region_leaves_round_corners_clear_for_bubbles_capsules_and_panels() {
        for (width, height, radius) in [(56, 56, 28.), (32, 56, 16.), (270, 528, 22.)] {
            let region = rounded_blur_region(width, height, radius);
            let rows = region.chunks_exact(4).collect::<Vec<_>>();
            assert_eq!(rows.len(), height as usize);
            assert!(rows[0][0] > 0 && rows[0][2] < width);
            assert_eq!(rows[height as usize / 2][2], width);
            for (top, bottom) in rows.iter().zip(rows.iter().rev()) {
                assert_eq!((top[0], top[2]), (bottom[0], bottom[2]));
                assert!(top[0] + top[2] <= width);
            }
        }
    }
}
