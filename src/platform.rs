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
