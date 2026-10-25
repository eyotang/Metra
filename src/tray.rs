use crate::{
    app, i18n::I18n, model::Provider, service::AppPayload, settings::DisplayMode, status_bar,
};
use tauri::{
    AppHandle,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

fn icon() -> Image<'static> {
    #[cfg(target_os = "macos")]
    let bytes = include_bytes!("../assets/icons/trayTemplate.png");
    #[cfg(not(target_os = "macos"))]
    let bytes = include_bytes!("../assets/icons/trayColor.png");
    Image::from_bytes(bytes).expect("embedded tray icon")
}
fn menu(app: &AppHandle, payload: &AppPayload) -> tauri::Result<Menu<tauri::Wry>> {
    let t = I18n::new(payload.settings.ui_language);
    let menu = Menu::new(app)?;
    for (id, key) in [
        ("details", "native.details"),
        ("settings", "native.settings"),
        ("refresh", "native.refresh"),
        (
            "display",
            if payload.settings.display_mode == DisplayMode::Bubble {
                "menu.displayStatusBar"
            } else {
                "menu.displayBubble"
            },
        ),
    ] {
        menu.append(&MenuItem::with_id(app, id, t.t(key), true, None::<&str>)?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        t.t("menu.quit"),
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}
pub(crate) fn setup(app: &AppHandle, payload: &AppPayload) -> tauri::Result<()> {
    TrayIconBuilder::with_id("metra-status")
        .icon(icon())
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Metra")
        .menu(&menu(app, payload)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| app::tray_action(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                app::tray_action(tray.app_handle(), "details");
            }
        })
        .build(app)?;
    Ok(())
}
pub(crate) fn rebuild(app: &AppHandle, payload: &AppPayload) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id("metra-status") {
        tray.set_menu(Some(menu(app, payload)?))?;
    }
    Ok(())
}
pub(crate) fn update(
    app: &AppHandle,
    payload: &AppPayload,
    cycle: usize,
) -> Result<Option<Provider>, String> {
    let tray = app
        .tray_by_id("metra-status")
        .ok_or("System tray unavailable")?;
    let entries = status_bar::entries(&payload.snapshot, &payload.settings);
    let summary = entries
        .iter()
        .map(status_bar::StatusEntry::headline)
        .collect::<Vec<_>>()
        .join("  |  ");
    tray.set_tooltip(Some(format!("Metra · {summary}")))
        .map_err(|e| e.to_string())?;
    if payload.settings.display_mode == DisplayMode::StatusBar {
        if let Some(entry) = entries.get(cycle % entries.len().max(1)) {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            tray.set_title(Some(entry.headline()))
                .map_err(|e| e.to_string())?;
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            tray.set_icon(Some(Image::new_owned(
                status_bar::usage_icon(entry),
                32,
                32,
            )))
            .map_err(|e| e.to_string())?;
            return Ok(Some(entry.provider));
        }
    } else {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        tray.set_title(Some("")).map_err(|e| e.to_string())?;
        tray.set_icon(Some(icon())).map_err(|e| e.to_string())?;
    }
    Ok(None)
}
