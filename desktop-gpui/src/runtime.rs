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
    bubble: Option<AnyWindowHandle>,
    _state: Subscription,
    tray_locale: String,
    icon: tray_icon::Icon,
    cycle: usize,
    cycle_task: Option<Task<()>>,
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

pub(super) fn toggle_panel(cx: &mut App) {
    let runtime = cx.global::<Runtime>();
    if runtime.dashboard.read(cx).page == Page::Details {
        if let Some(panel) = runtime.panel {
            if panel
                .update(cx, |_, window, _| window.remove_window())
                .is_ok()
            {
                cx.global_mut::<Runtime>().panel = None;
                return;
            }
        }
    }
    open_panel(Page::Details, cx);
}

pub(super) fn open_panel(page: Page, cx: &mut App) {
    let dashboard = cx.global::<Runtime>().dashboard.clone();
    dashboard.update(cx, |view, cx| {
        view.page = page;
        cx.notify();
    });
    if let Some(panel) = cx.global::<Runtime>().panel {
        if panel
            .update(cx, |_, window, cx| {
                position_panel(page, window, cx);
                window.activate_window();
            })
            .is_ok()
        {
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(420.), px(680.)), cx);
    match gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            titlebar: None,
            window_min_size: Some(size(px(280.), px(160.))),
            ..Default::default()
        },
        cx,
        move |window, cx| {
            position_panel(page, window, cx);
            dashboard.update(cx, |view, cx| {
                view.panel_activation =
                    Some(cx.observe_window_activation(window, |_, window, cx| {
                        if !window.is_window_active() {
                            window.remove_window();
                            cx.global_mut::<Runtime>().panel = None;
                        }
                    }));
            });
            dashboard
        },
    ) {
        Ok((panel, _)) => cx.global_mut::<Runtime>().panel = Some(panel),
        Err(error) => eprintln!("Unable to open Metra panel: {error}"),
    }
}

pub(super) fn position_panel(page: Page, window: &mut Window, cx: &mut App) {
    let result = (|| {
        let (mut work, mut scale) = native_window::work_area(window, cx)?;
        let anchor = if let Some(bubble) = cx.global::<Runtime>().bubble {
            bubble
                .update(cx, |_, bubble, cx| {
                    if let Ok((area, factor)) = native_window::work_area(bubble, cx) {
                        work = area;
                        scale = factor;
                    }
                    native_window::position(bubble)
                })
                .ok()
                .and_then(Result::ok)
        } else {
            None
        };
        let width = if page == Page::Settings { 340. } else { 360. };
        let height: f64 = if page == Page::Consent { 300. } else { 560. };
        let height = height.min(work.height / scale - 16.).max(160.);
        window.resize(size(px(width as f32), px(height as f32)));
        let (mut x, mut y) = (
            work.x + work.width - (width + 12.) * scale,
            work.y + 12. * scale,
        );
        if let Some((bx, by)) = anchor {
            x = bx as f64 - (width + 8.) * scale;
            if x < work.x {
                x = bx as f64 + 64. * scale;
            }
            y = by as f64;
        } else if let Some(rect) = cx
            .global::<Runtime>()
            ._tray
            .as_ref()
            .and_then(|tray| tray.rect())
        {
            x = rect.position.x - width * scale;
            y = rect.position.y + 24. * scale;
        }
        let (x, y) = work.clamp(x, y, width * scale, height * scale);
        native_window::move_to(window, x, y)
    })();
    if let Err(error) = result {
        eprintln!("Unable to position panel: {error}");
    }
}

pub(super) fn start(
    service: Arc<RefreshService>,
    config: PathBuf,
    lock: File,
    cx: &mut App,
) -> Result<(), String> {
    #[cfg(feature = "native-smoke")]
    if std::env::var_os("METRA_GPUI_DEMO").is_some() {
        assert!(
            std::env::var_os("METRA_CONFIG_DIR").is_some(),
            "Demo requires isolated configuration"
        );
        use metra_lib::model::{CostUsage, ProviderStatus, QuotaWindow, TokenUsage};
        let mut snapshot = service.snapshot.lock().unwrap();
        snapshot.refreshing = false;
        let captured = chrono::Utc::now();
        for (data, plan, used) in [(&mut snapshot.cursor, "Pro", 23.)] {
            data.status = ProviderStatus::Available;
            data.plan = Some(plan.into());
            data.captured_at = captured;
            data.message = None;
            data.quotas = vec![QuotaWindow::from_used_percent(
                "Monthly",
                used,
                None,
                Some(captured + chrono::Duration::days(4)),
            )];
            data.cost = Some(CostUsage {
                currency: "USD".into(),
                today_used_cents: None,
                included_used_cents: Some(2300),
                included_limit_cents: Some(10000),
                on_demand_used_cents: None,
                on_demand_limit_cents: None,
                on_demand_enabled: Some(false),
                period_end: None,
            });
        }
        snapshot.codex.status = ProviderStatus::Available;
        snapshot.codex.plan = Some("Plus".into());
        snapshot.codex.message = None;
        snapshot.codex.quotas = vec![
            QuotaWindow::from_used_percent(
                "5 hours",
                30.,
                Some(300),
                Some(captured + chrono::Duration::hours(2)),
            ),
            QuotaWindow::from_used_percent(
                "Weekly",
                18.,
                Some(10080),
                Some(captured + chrono::Duration::days(4)),
            ),
        ];
        snapshot.codex.tokens = Some(TokenUsage {
            today: Some(12000),
            lifetime: Some(900000),
            peak_daily: None,
        });
        snapshot.claude.status = ProviderStatus::Available;
        snapshot.claude.plan = Some("Pro".into());
        snapshot.claude.message = None;
        snapshot.claude.quotas = vec![QuotaWindow::from_used_percent(
            "5 hours",
            44.,
            Some(300),
            None,
        )];
    }
    let t = I18n::new(service.payload().settings.ui_language);
    let (sender, receiver) = async_channel::unbounded::<String>();
    let menu = Menu::new();
    for (id, key) in [
        ("details", "native.details"),
        ("settings", "native.settings"),
        ("refresh", "native.refresh"),
        ("display", "menu.displayStatusBar"),
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
        .with_icon(icon.clone())
        .with_icon_as_template(cfg!(target_os = "macos"))
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|error| eprintln!("System tray unavailable: {error}"))
        .ok();
    let dashboard = cx.new(|cx| {
        let mut view = Dashboard {
            panel_activation: None,
            update_notes_expanded: false,
            page: Page::Details,
            updater: Arc::new(metra_lib::updater::Updater::default()),
            update_task: None,
            update_timer: None,
            login_task: None,
            labels: Vec::new(),
            pending_settings: std::collections::VecDeque::new(),
            error: None,
            settings_task: None,
            interval_task: None,
            service,
            refresh_task: None,
            refreshing: Vec::new(),
        };
        if !(cfg!(feature = "native-smoke") && std::env::var_os("METRA_GPUI_DEMO").is_some()) {
            view.refresh(cx);
            view.schedule_refresh(cx);
            view.schedule_update(std::time::Duration::from_secs(20), cx);
        }
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
                "details" => toggle_panel(cx),
                "settings" => open_panel(Page::Settings, cx),
                "refresh" => {
                    let view = cx.global::<Runtime>().dashboard.clone();
                    view.update(cx, |view, cx| view.refresh(cx));
                }
                "display" => {
                    let view = cx.global::<Runtime>().dashboard.clone();
                    view.update(cx, |view, cx| {
                        use metra_lib::{settings::DisplayMode, settings_change::Change};
                        let current = view.service.payload().settings.display_mode;
                        view.save(
                            Change::Display(if current == DisplayMode::Bubble {
                                DisplayMode::StatusBar
                            } else {
                                DisplayMode::Bubble
                            }),
                            cx,
                        );
                    });
                }
                "quit" => cx.quit(),
                _ => {}
            });
        }
    });
    let state = cx.observe(&dashboard, |_, cx| sync(cx));
    cx.set_global(Runtime {
        dashboard,
        panel: None,
        bubble: None,
        _state: state,
        tray_locale: String::new(),
        icon,
        cycle: 0,
        cycle_task: None,
        _tray: tray,
        _lock: lock,
        _events: events,
        _activation: activation,
    });
    sync(cx);
    if std::env::args().any(|a| a == "--settings") {
        open_panel(Page::Settings, cx);
    } else if cfg!(feature = "native-smoke")
        && (std::env::var_os("METRA_GPUI_SMOKE").is_some()
            || std::env::var_os("METRA_GPUI_DEMO").is_some())
    {
        open_panel(Page::Details, cx);
    }
    #[cfg(feature = "native-smoke")]
    if std::env::var_os("METRA_GPUI_SMOKE").is_some() {
        smoke(cx);
    }
    Ok(())
}

pub(super) fn sync(cx: &mut App) {
    use metra_lib::{settings::DisplayMode, status_bar};
    let data = cx.global::<Runtime>().dashboard.read(cx).service.payload();
    let t = I18n::new(data.settings.ui_language);
    let mode = data.settings.display_mode;
    let entries = status_bar::entries(&data.snapshot, &data.settings);
    let runtime = cx.global_mut::<Runtime>();
    if let Some(tray) = &runtime._tray {
        let summary = entries
            .iter()
            .map(status_bar::StatusEntry::headline)
            .collect::<Vec<_>>()
            .join(" | ");
        let _ = tray.set_tooltip(Some(format!("Metra · {summary}")));
        let key = format!("{}:{mode:?}", t.locale);
        if runtime.tray_locale != key {
            let menu = Menu::new();
            for (id, key) in [
                ("details", "native.details"),
                ("settings", "native.settings"),
                ("refresh", "native.refresh"),
                (
                    "display",
                    if mode == DisplayMode::Bubble {
                        "menu.displayStatusBar"
                    } else {
                        "menu.displayBubble"
                    },
                ),
                ("quit", "menu.quit"),
            ] {
                if let Err(error) = menu.append(&MenuItem::with_id(id, t.t(key), true, None)) {
                    eprintln!("{error}");
                }
            }
            tray.set_menu(Some(Box::new(menu)));
            runtime.tray_locale = key;
        }
    }
    if let Some(tray) = &runtime._tray {
        if mode == DisplayMode::StatusBar {
            if let Some(entry) = entries.get(runtime.cycle % entries.len().max(1)) {
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                tray.set_title(Some(entry.headline()));
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                if let Ok(icon) = tray_icon::Icon::from_rgba(status_bar::usage_icon(entry), 32, 32)
                {
                    let _ = tray.set_icon(Some(icon));
                }
            }
        } else {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            tray.set_title(Some(""));
            let _ = tray
                .set_icon_with_as_template(Some(runtime.icon.clone()), cfg!(target_os = "macos"));
        }
    }
    let show = mode == DisplayMode::Bubble || runtime._tray.is_none();
    let existing = runtime.bubble;
    if show && existing.is_none() {
        match bubble::open(cx) {
            Ok(handle) => cx.global_mut::<Runtime>().bubble = Some(handle),
            Err(error) => eprintln!("{error}"),
        }
    } else if !show {
        if let Some(handle) = existing {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            cx.global_mut::<Runtime>().bubble = None;
        }
    }
    if mode == DisplayMode::StatusBar && cx.global::<Runtime>().cycle_task.is_none() {
        let task = cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(3))
                    .await;
                cx.update(|cx| {
                    cx.global_mut::<Runtime>().cycle = cx.global::<Runtime>().cycle.wrapping_add(1);
                    sync(cx);
                });
            }
        });
        cx.global_mut::<Runtime>().cycle_task = Some(task);
    } else if mode == DisplayMode::Bubble {
        cx.global_mut::<Runtime>().cycle_task = None;
    }
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

#[cfg(feature = "native-smoke")]
fn smoke(cx: &mut App) {
    let config = std::env::var_os("METRA_CONFIG_DIR")
        .expect("Native smoke requires isolated METRA_CONFIG_DIR");
    cx.spawn(async move |cx| {
        use metra_lib::{settings::{DisplayMode,UiLanguage},settings_change::Change};
        for phase in 0..6 {
            cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
            cx.update(|cx| {
                let dashboard=cx.global::<Runtime>().dashboard.clone();
                assert!(dashboard.read(cx).error.is_none(),"settings operation failed");
                match phase {
                    0=>{
                        assert!(cx.global::<Runtime>().bubble.is_some());
                        let panel=cx.global::<Runtime>().panel.unwrap();
                        panel.update(cx,|_,window,_|window.remove_window()).unwrap();
                        open_panel(Page::Settings,cx);
                        assert_eq!(cx.windows().len(),2);
                        dashboard.update(cx,|view,cx|view.save(Change::Display(DisplayMode::StatusBar),cx));
                    },
                    1=>{
                        assert!(cx.global::<Runtime>().bubble.is_none());
                        assert_eq!(cx.windows().len(),1);
                        dashboard.update(cx,|view,cx|view.save(Change::Language(UiLanguage::Ja),cx));
                    },
                    2=>{
                        assert!(cx.global::<Runtime>().tray_locale.starts_with("ja:"));
                        dashboard.update(cx,|view,cx|view.save(Change::Display(DisplayMode::Bubble),cx));
                    },
                    3=>{
                        assert!(cx.global::<Runtime>().bubble.is_some());
                        assert!(cx.global::<Runtime>().cycle_task.is_none());
                        assert_eq!(cx.windows().len(),2);
                        dashboard.update(cx,|view,cx| {
                            view.save(Change::Label {provider:metra_lib::model::Provider::Codex,label:"TEST".into()},cx);
                            view.save(Change::Refresh(15),cx);
                        });
                    },
                    4=>{
                        let saved=dashboard.read(cx).service.store.load();
                        assert_eq!(saved.codex_bubble_label,"TES");assert_eq!(saved.refresh_minutes,15);
                        assert!(dashboard.read(cx).pending_settings.is_empty());
                        let bubble=cx.global::<Runtime>().bubble.unwrap();
                        bubble.update(cx,|_,window,cx| {
                            let (x,y)=native_window::position(window).unwrap();
                            let (work,scale)=native_window::work_area(window,cx).unwrap();
                            let width=f64::from(f32::from(window.bounds().size.width))*scale;
                            assert!(x as f64>=work.x-2. && y as f64>=work.y-2.);
                            assert!(x as f64+width<=work.x+work.width+2.);
                            assert!(y as f64+56.*scale<=work.y+work.height+2.);
                        }).unwrap();
                    },
                    _=>{
                        std::fs::write(std::path::Path::new(&config).join("native-smoke.result"),"PASS: panel recreation, display modes, language menu, queued settings persistence").unwrap();
                        cx.quit();
                    },
                }
            });
        }
    }).detach();
}
