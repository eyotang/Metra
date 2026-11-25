use super::*;
use notify::Watcher;
use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
};
use tray_icon::{
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

pub(super) struct Runtime {
    pub dashboard: Entity<Dashboard>,
    pub panel: Option<AnyWindowHandle>,
    _tray: Option<TrayIcon>,
    _lock: File,
    _events: Task<()>,
    _activation: notify::RecommendedWatcher,
}
impl Global for Runtime {}

pub(super) fn instance(config: &std::path::Path) -> Result<Option<File>, String> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(config.join("instance.lock"))
        .map_err(|e| e.to_string())?;
    match lock.try_lock() {
        Ok(()) => Ok(Some(lock)),
        Err(std::fs::TryLockError::WouldBlock) => {
            std::fs::write(config.join("activate"), b"show").map_err(|e| e.to_string())?;
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn open_panel(page: Page, cx: &mut App) {
    let dashboard = cx.global::<Runtime>().dashboard.clone();
    dashboard.update(cx, |view, cx| {
        view.page = page;
        cx.notify();
    });
    if let Some(panel) = cx.global::<Runtime>().panel {
        if panel
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(420.), px(680.)), cx);
    match gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        cx,
        move |_, _| dashboard,
    ) {
        Ok((panel, _)) => cx.global_mut::<Runtime>().panel = Some(panel),
        Err(error) => eprintln!("Unable to open Metra panel: {error}"),
    }
}

pub(super) fn start(
    service: Arc<RefreshService>,
    config: PathBuf,
    lock: File,
    cx: &mut App,
) -> Result<(), String> {
    let t = I18n::new(service.payload().settings.ui_language);
    let (sender, receiver) = async_channel::unbounded::<String>();
    let menu = Menu::new();
    for (id, key) in [
        ("details", "native.details"),
        ("settings", "native.settings"),
        ("refresh", "native.refresh"),
    ] {
        menu.append(&MenuItem::with_id(id, t.t(key), true, None))
            .map_err(|e| e.to_string())?;
    }
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|e| e.to_string())?;
    menu.append(&MenuItem::with_id("quit", t.t("menu.quit"), true, None))
        .map_err(|e| e.to_string())?;
    let menu_sender = sender.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = menu_sender.try_send(event.id.as_ref().to_string());
    }));
    let activation_sender = sender.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            let _ = sender.try_send("details".into());
        }
    }));
    #[cfg(target_os = "macos")]
    let bytes = include_bytes!("../../assets/icons/trayTemplate.png");
    #[cfg(not(target_os = "macos"))]
    let bytes = include_bytes!("../../assets/icons/trayColor.png");
    let image = image::load_from_memory(bytes)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let icon =
        tray_icon::Icon::from_rgba(image.into_raw(), width, height).map_err(|e| e.to_string())?;
    let tray = TrayIconBuilder::new()
        .with_tooltip("Metra")
        .with_icon(icon)
        .with_icon_as_template(cfg!(target_os = "macos"))
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|error| eprintln!("System tray unavailable: {error}"))
        .ok();
    let dashboard = cx.new(|cx| {
        let mut view = Dashboard {
            page: Page::Details,
            error: None,
            settings_task: None,
            interval_task: None,
            service,
            refresh_task: None,
            refreshing: Vec::new(),
        };
        view.refresh(cx);
        view.schedule_refresh(cx);
        view
    });
    let activation_path = config.join("activate");
    let mut activation =
        notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
            if let Ok(event) = event {
                if !matches!(event.kind, notify::EventKind::Remove(_))
                    && event
                        .paths
                        .iter()
                        .any(|p| p.file_name().is_some_and(|n| n == "activate"))
                {
                    let _ = activation_sender.try_send("activate".into());
                }
            }
        })
        .map_err(|e| e.to_string())?;
    activation
        .watch(&config, notify::RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;
    let events = cx.spawn(async move |cx| {
        while let Ok(action) = receiver.recv().await {
            let _ = cx.update(|cx| match action.as_str() {
                "activate" => {
                    let _ = std::fs::remove_file(&activation_path);
                    open_panel(Page::Details, cx);
                }
                "details" => open_panel(Page::Details, cx),
                "settings" => open_panel(Page::Settings, cx),
                "refresh" => {
                    let view = cx.global::<Runtime>().dashboard.clone();
                    view.update(cx, |view, cx| view.refresh(cx));
                }
                "quit" => cx.quit(),
                _ => {}
            });
        }
    });
    cx.set_global(Runtime {
        dashboard,
        panel: None,
        _tray: tray,
        _lock: lock,
        _events: events,
        _activation: activation,
    });
    open_panel(
        if std::env::args().any(|a| a == "--settings") {
            Page::Settings
        } else {
            Page::Details
        },
        cx,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[::core::prelude::v1::test]
    fn second_instance_requests_activation_without_taking_lock() {
        let directory = tempfile::tempdir().unwrap();
        let first = super::instance(directory.path()).unwrap().unwrap();
        assert!(super::instance(directory.path()).unwrap().is_none());
        assert_eq!(
            std::fs::read(directory.path().join("activate")).unwrap(),
            b"show"
        );
        drop(first);
        assert!(super::instance(directory.path()).unwrap().is_some());
    }
}
