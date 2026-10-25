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

/// Return whether native material is active; CSS supplies the compatible translucent fallback.
pub fn apply_glass(window: &tauri::WebviewWindow, radius: f64) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        use tauri::window::{Effect, EffectsBuilder};
        let effects = EffectsBuilder::new()
            .effect(Effect::HudWindow)
            .state(tauri::window::EffectState::Active)
            .radius(radius)
            .build();
        {
            // Tao's per-window theme API changes NSApplication appearance too, so set
            // NSWindow appearance directly while leaving status items and menus alone.
            let overlay = window.clone();
            window
                .run_on_main_thread(move || {
                    use objc2_app_kit::{
                        NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSWindow,
                    };
                    if let Ok(pointer) = overlay.ns_window() {
                        // SAFETY: Tauri owns the live NSWindow and this closure runs on AppKit's main thread.
                        let native = unsafe { &*pointer.cast::<NSWindow>() };
                        let appearance =
                            NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua });
                        native.setAppearance(appearance.as_deref());
                    }
                })
                .map_err(|error| error.to_string())?;
        }
        window
            .set_effects(effects)
            .map_err(|error| error.to_string())?;
        update_shape(window, radius)?;
    }
    #[cfg(windows)]
    {
        // Whole-window DWM/Accent materials fill transparent corners on Windows 11.
        // Keep the transparent WebView2 path used by earlier Tauri releases.
        window
            .set_effects(None)
            .map_err(|error| error.to_string())?;
        update_shape(window, radius)?;
    }
    #[cfg(target_os = "linux")]
    update_shape(window, radius)?;
    Ok(!cfg!(windows))
}

/// Keep native clipping and compositor blur within the current window silhouette.
pub fn update_shape(window: &tauri::WebviewWindow, radius: f64) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn};
        let handle = window.hwnd().map_err(|error| error.to_string())?.0;
        let size = window.outer_size().map_err(|error| error.to_string())?;
        let diameter = (radius * 2. * window.scale_factor().map_err(|error| error.to_string())?)
            .round() as i32;
        // SAFETY: the HWND belongs to this live Tauri window. SetWindowRgn owns the region
        // on success; on failure it remains ours and must be deleted.
        unsafe {
            let region = CreateRoundRectRgn(
                0,
                0,
                size.width as i32 + 1,
                size.height as i32 + 1,
                diameter,
                diameter,
            );
            if region.is_null() {
                return Err("Could not create the glass window region".into());
            }
            if SetWindowRgn(handle, region, 1) == 0 {
                DeleteObject(region);
                return Err("Could not apply the glass window region".into());
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use x11rb::{
            connection::Connection,
            protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode},
            wrapper::ConnectionExt as _,
        };
        let raw = window.window_handle().map_err(|e| e.to_string())?.as_raw();
        let xid = match raw {
            RawWindowHandle::Xlib(handle) => handle.window as u32,
            RawWindowHandle::Xcb(handle) => handle.window.get(),
            _ => return Err("Native glass requires an X11 compositor".into()),
        };
        let (connection, _) = x11rb::connect(None).map_err(|e| e.to_string())?;
        let atom = connection
            .intern_atom(false, b"_KDE_NET_WM_BLUR_BEHIND_REGION")
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?
            .atom;
        let size = window.outer_size().map_err(|e| e.to_string())?;
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        connection
            .change_property32(
                PropMode::REPLACE,
                xid,
                atom,
                AtomEnum::CARDINAL,
                &rounded_blur_region(size.width, size.height, radius * scale),
            )
            .map_err(|e| e.to_string())?
            .check()
            .map_err(|e| e.to_string())?;
        connection.flush().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (window, radius);
    }

    Ok(())
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
