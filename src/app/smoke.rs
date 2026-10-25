//! Native acceptance checks, compiled only for CI with `desktop-smoke`.
use super::*;
use crate::model::{ProviderStatus, QuotaWindow};
use serde_json::json;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub(super) fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let result = if std::env::var_os("METRA_CONFIG_DIR").is_some() {
            exercise(&app)
        } else {
            Err("Native smoke checks require an isolated METRA_CONFIG_DIR".into())
        };
        let directory = app
            .state::<Desktop>()
            .service
            .store
            .path()
            .parent()
            .map(std::path::Path::to_path_buf);
        let report = match &result {
            Ok(report) => report.clone(),
            Err(error) => json!({ "ok": false, "error": error }),
        };
        if let Some(directory) = directory {
            let _ = std::fs::write(directory.join("desktop-smoke.json"), report.to_string());
        }
        if let Err(error) = &result {
            eprintln!("Native desktop smoke failed: {error}");
        }
        if result.is_ok() {
            let _ = dispatch_tray_action(&app, "quit");
        } else {
            app.exit(1);
        }
    });
}
fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn settle() {
    std::thread::sleep(Duration::from_millis(200));
}
fn await_native(condition: impl Fn() -> bool, message: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(7);
    while !condition() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    require(condition(), message)
}
fn exercise(app: &AppHandle) -> Result<serde_json::Value> {
    publish(app);
    #[cfg(target_os = "macos")]
    macos::check_tray_template(app)?;
    let state = app.state::<Desktop>();
    let bubble = app.get_webview_window("bubble").ok_or("Missing bubble")?;
    let panel = app.get_webview_window("panel").ok_or("Missing panel")?;
    require(
        app.tray_by_id("metra-status").is_some(),
        "Native tray was not created",
    )?;
    {
        let mut snapshot = state.service.snapshot.lock().map_err(|e| e.to_string())?;
        let snapshot = &mut *snapshot;
        for data in [
            &mut snapshot.cursor,
            &mut snapshot.codex,
            &mut snapshot.claude,
        ] {
            data.status = ProviderStatus::Available;
            data.quotas = vec![QuotaWindow::from_used_percent("Smoke", 37., None, None)];
        }
        snapshot.refreshing = false;
    }
    state.service.update_settings(|settings| {
        settings.bubble_provider_order = vec![Provider::Codex, Provider::Cursor, Provider::Claude];
        settings.bubble_visible_providers = vec![Provider::Codex, Provider::Cursor];
        settings.bubble_percent_mode = BubblePercentMode::Used;
        settings.cursor_bubble_label = "CU".into();
    })?;
    dispatch_tray_action(app, "display")?;
    await_native(
        || state.service.payload().settings.display_mode == DisplayMode::StatusBar,
        "Tray display action did not run",
    )?;
    settle();
    require(
        !bubble.is_visible().map_err(|e| e.to_string())?,
        "Status-bar mode did not hide the bubble",
    )?;
    let payload = state.service.payload();
    let mut rotation = Vec::new();
    for cycle in 0..3 {
        rotation.push(tray::update(app, &payload, cycle)?);
        settle();
    }
    require(
        rotation
            == vec![
                Some(Provider::Codex),
                Some(Provider::Cursor),
                Some(Provider::Codex),
            ],
        "Tray rotation lost visibility/order",
    )?;
    state.tray_cycle.store(1, Ordering::Relaxed);
    publish(app);
    require(
        state.tray_observed_provider.load(Ordering::Relaxed) == provider_mask(Provider::Cursor),
        "State publishing reset the displayed provider",
    )?;
    await_native(
        || {
            state.tray_cycle.load(Ordering::Relaxed) != 1
                && state.tray_observed_provider.load(Ordering::Relaxed)
                    == provider_mask(Provider::Codex)
        },
        "Production five-second tray timer did not advance",
    )?;
    require(
        state.tray_observed_provider.load(Ordering::Relaxed) == provider_mask(Provider::Codex),
        "Tray timer did not present the next provider",
    )?;
    dispatch_tray_action(app, "display")?;
    await_native(
        || state.service.payload().settings.display_mode == DisplayMode::Bubble,
        "Tray display action did not restore bubble mode",
    )?;
    settle();
    require(
        bubble.is_visible().map_err(|e| e.to_string())?,
        "Bubble mode did not restore the bubble",
    )?;
    dispatch_tray_action(app, "details")?;
    await_native(
        || panel.is_visible().unwrap_or(false),
        "Tray details action did not open panel",
    )?;
    settle();
    require(
        panel.is_visible().map_err(|e| e.to_string())?,
        "Details panel did not open",
    )?;
    dispatch_tray_action(app, "settings")?;
    await_native(
        || *state.panel.lock().unwrap() == "settings",
        "Tray settings action did not run",
    )?;
    settle();
    require(
        *state.panel.lock().map_err(|e| e.to_string())? == "settings",
        "Settings menu action was lost",
    )?;
    dispatch_tray_action(app, "details")?;
    await_native(
        || *state.panel.lock().unwrap() == "details",
        "Tray details reopen action did not run",
    )?;
    dispatch_tray_action(app, "details")?;
    await_native(
        || !panel.is_visible().unwrap_or(true),
        "Tray click did not toggle details closed",
    )?;
    settle();
    require(
        !panel.is_visible().map_err(|e| e.to_string())?,
        "Panel did not hide",
    )?;
    bubble
        .set_size(LogicalSize::new(80., 80.))
        .map_err(|e| e.to_string())?;
    settle();
    check_region(&bubble)?;
    bubble
        .set_size(LogicalSize::new(56., 56.))
        .map_err(|e| e.to_string())?;
    settle();
    change_settings(app, Change::Snap(true))?;
    settle();
    let scale = bubble.scale_factor().map_err(|e| e.to_string())?;
    require(
        (bubble.outer_size().map_err(|e| e.to_string())?.width as f64 / scale - 32.).abs() <= 1.,
        "Docked capsule width is incorrect",
    )?;
    check_region(&bubble)?;
    set_peek(app, false, false)?;
    settle();
    require(
        (bubble.outer_size().map_err(|e| e.to_string())?.width as f64 / scale - 56.).abs() <= 1.,
        "Hover expansion width is incorrect",
    )?;
    check_region(&bubble)?;
    #[cfg(target_os = "macos")]
    macos::check_tray_template(app)?;
    #[cfg(windows)]
    windows::check_transparent_edges(app, &bubble, &panel)?;
    Ok(
        json!({ "ok": true, "os": std::env::consts::OS, "version": env!("CARGO_PKG_VERSION"),
        "nativeTray": true, "rotation": rotation, "modeSwitch": true, "panels": true, "menuDispatcher": true, "productionTimer": true,
        "capsule": true, "nativeRegion": cfg!(windows), "transparentEdges": cfg!(windows),
        "nativeTemplate": cfg!(target_os = "macos"), "isolated": true }),
    )
}
fn check_region(window: &WebviewWindow) -> Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::RECT,
            Graphics::Gdi::{CreateRectRgn, DeleteObject, GetRgnBox, GetWindowRgn},
        };
        let handle = window.hwnd().map_err(|e| e.to_string())?.0;
        let size = window.outer_size().map_err(|e| e.to_string())?;
        // SAFETY: the native window is alive; the temporary region and RECT are ours.
        unsafe {
            let region = CreateRectRgn(0, 0, 0, 0);
            require(
                !region.is_null(),
                "Could not allocate region for inspection",
            )?;
            let mut bounds: RECT = std::mem::zeroed();
            let result = GetWindowRgn(handle, region);
            let boxed = GetRgnBox(region, &mut bounds);
            DeleteObject(region);
            require(
                result != 0 && boxed != 0,
                "Native rounded region is missing",
            )?;
            require(
                (bounds.right - bounds.left - size.width as i32).abs() <= 2
                    && (bounds.bottom - bounds.top - size.height as i32).abs() <= 2,
                "Native region was not updated after resizing/scaling",
            )?;
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window;
    }
    Ok(())
}
