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
    pub positions: async_channel::Sender<metra_lib::settings::SavedPosition>,
    bubble: Option<AnyWindowHandle>,
    _state: Subscription,
    _displays: Subscription,
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
#[cfg(feature = "native-smoke")]
mod idle;
#[cfg(all(feature = "native-smoke", target_os = "windows"))]
mod paint_windows;

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

pub(super) fn tray_available(cx: &App) -> bool {
    cx.global::<Runtime>()._tray.is_some()
}

pub(super) fn panel_key_down(event: &KeyDownEvent, window: &mut Window, cx: &mut App) {
    if event.keystroke.key == "escape" {
        if !cx.stop_active_drag(window) {
            window.remove_window();
            cx.global_mut::<Runtime>().panel = None;
        }
        cx.stop_propagation();
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
                let focus = dashboard.read(cx).focus.clone();
                window.focus(&focus, cx);
            })
            .is_ok()
        {
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(420.), px(680.)), cx);
    match open_overlay(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            titlebar: None,
            window_min_size: Some(size(px(280.), px(160.))),
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        cx,
        move |window, cx| {
            position_panel(page, window, cx);
            dashboard.update(cx, |view, cx| {
                window.focus(&view.focus, cx);
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

fn recover_windows(cx: &mut App) {
    if let Some(bubble) = cx.global::<Runtime>().bubble {
        let _ = bubble.update(cx, |_, window, cx| {
            let result = (|| {
                let (work, scale) = native_window::work_area(window, cx)?;
                let (x, y) = native_window::position(window)?;
                let bounds = window.bounds();
                let (x, y) = work.clamp(
                    x as f64,
                    y as f64,
                    f64::from(f32::from(bounds.size.width)) * scale,
                    f64::from(f32::from(bounds.size.height)) * scale,
                );
                native_window::move_to(window, x, y)
            })();
            if let Err(error) = result {
                eprintln!("Unable to recover bubble position: {error}");
            }
        });
    }
    if let Some(panel) = cx.global::<Runtime>().panel {
        let page = cx.global::<Runtime>().dashboard.read(cx).page;
        let _ = panel.update(cx, |_, window, cx| position_panel(page, window, cx));
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
    let (positions, position_updates) =
        async_channel::unbounded::<metra_lib::settings::SavedPosition>();
    let position_service = service.clone();
    cx.background_executor()
        .spawn(async move {
            // One FIFO writer prevents an older drag/hover save from overwriting a
            // later position when background tasks start in a different order.
            while let Ok(position) = position_updates.recv().await {
                if let Err(error) = position_service.update_settings(|settings| {
                    settings.bubble_position = Some(position);
                    settings.bubble_position_version = 3;
                }) {
                    eprintln!("{error}");
                }
            }
        })
        .detach();
    let dashboard = cx.new(|cx| {
        let mut view = Dashboard {
            focus: cx.focus_handle(),
            panel_activation: None,
            saved_labels: Vec::new(),
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
            last_refresh: std::time::Instant::now(),
            service,
            refresh_task: None,
            refreshing: Vec::new(),
        };
        if !(cfg!(feature = "native-smoke") && std::env::var_os("METRA_GPUI_DEMO").is_some()) {
            view.refresh(cx);
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
    let displays = cx.observe_displays(|_, cx| cx.defer(recover_windows));
    cx.set_global(Runtime {
        dashboard,
        panel: None,
        positions,
        bubble: None,
        _state: state,
        _displays: displays,
        tray_locale: String::new(),
        icon,
        cycle: 0,
        cycle_task: None,
        _tray: tray,
        _lock: lock,
        _events: events,
        _activation: activation,
    });
    let settings = cx
        .global::<Runtime>()
        .dashboard
        .read(cx)
        .service
        .payload()
        .settings;
    if settings.bubble_position_version != 3
        && settings.display_mode == metra_lib::settings::DisplayMode::StatusBar
    {
        bubble::initialize_position(cx)?;
    }
    sync(cx);
    if std::env::args().any(|a| a == "--settings") {
        open_panel(Page::Settings, cx);
    } else if cfg!(feature = "native-smoke")
        && std::env::var_os("METRA_GPUI_MIGRATION_SMOKE").is_none()
        && std::env::var_os("METRA_GPUI_IDLE_SMOKE").is_none()
        && std::env::var_os("METRA_GPUI_PAINT_SMOKE").is_none()
        && (std::env::var_os("METRA_GPUI_SMOKE").is_some()
            || std::env::var_os("METRA_GPUI_DEMO").is_some())
    {
        open_panel(Page::Details, cx);
    }
    #[cfg(feature = "native-smoke")]
    if std::env::var_os("METRA_GPUI_SMOKE").is_some() {
        if std::env::var_os("METRA_GPUI_MIGRATION_SMOKE").is_some() {
            migration_smoke(cx);
        } else if std::env::var_os("METRA_GPUI_IDLE_SMOKE").is_some() {
            idle::start(cx);
        } else if std::env::var_os("METRA_GPUI_PAINT_SMOKE").is_some() {
            #[cfg(target_os = "windows")]
            paint_windows::start(cx);
        } else {
            smoke(cx);
        }
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
                    .timer(std::time::Duration::from_secs(5))
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
        for phase in 0..12 {
            cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
            cx.update(|cx| {
                let dashboard=cx.global::<Runtime>().dashboard.clone();
                assert!(dashboard.read(cx).error.is_none(),"settings operation failed");
                match phase {
                    0=>{
                        assert!(cx.global::<Runtime>().bubble.is_some());
                        #[cfg(target_os = "windows")]
                        cx.global::<Runtime>().bubble.unwrap().update(cx, |_,window,_| idle::check_windows_surface(window)).unwrap();
                        let panel=cx.global::<Runtime>().panel.unwrap();
                        panel.update(cx,|_,window,_|window.remove_window()).unwrap();
                        open_panel(Page::Settings,cx);
                        assert_eq!(cx.windows().len(),2);
                        dashboard.update(cx,|view,cx| {
                            view.save(Change::Display(DisplayMode::StatusBar),cx);
                            if !tray_available(cx) {
                                assert!(view.error.as_deref().is_some_and(|e| e.contains("Status bar is unavailable")));
                                view.error=None;
                            }
                        });
                    },
                    1=>{
                        let tray_available = cx.global::<Runtime>()._tray.is_some();
                        assert_eq!(cx.global::<Runtime>().bubble.is_none(), tray_available);
                        assert_eq!(cx.windows().len(),if tray_available {1} else {2});
                        cx.global::<Runtime>().panel.unwrap().update(cx, |_,window,_| window.remove_window()).unwrap();
                        cx.global_mut::<Runtime>().panel = None;
                        dashboard.update(cx,|view,cx|view.save(Change::Language(UiLanguage::Ja),cx));
                    },
                    2=>{
                        assert!(cx.global::<Runtime>().panel.is_none());
                        open_panel(Page::Settings,cx);
                        assert_eq!(dashboard.read(cx).service.payload().settings.ui_language, UiLanguage::Ja);
                        if cx.global::<Runtime>()._tray.is_some() {
                            assert!(cx.global::<Runtime>().tray_locale.starts_with("ja:"));
                        }
                        dashboard.update(cx,|view,cx|view.save(Change::Display(DisplayMode::Bubble),cx));
                    },
                    3=>{
                        assert!(cx.global::<Runtime>().bubble.is_some());
                        assert!(cx.global::<Runtime>().cycle_task.is_none());
                        assert_eq!(cx.windows().len(),2);
                        cx.global::<Runtime>().panel.unwrap().update(cx, |_, window, cx| {
                            let input = dashboard.read(cx).labels.iter().find(|(provider, ..)| *provider == metra_lib::model::Provider::Codex).unwrap().1.clone();
                            input.update(cx, |input, cx| input.set_value("TEST", window, cx));
                        }).unwrap();
                        dashboard.update(cx,|view,cx| {
                            view.save(Change::Label {provider:metra_lib::model::Provider::Codex,label:"TEST".into()},cx);
                            view.save(Change::Refresh(15),cx);
                        });
                    },
                    4=>{
                        let saved=dashboard.read(cx).service.store.load();
                        assert_eq!(saved.codex_bubble_label,"TES");assert_eq!(saved.refresh_minutes,15);
                        assert_eq!(saved.bubble_position_version,3);
                        assert!(dashboard.read(cx).pending_settings.is_empty());
                        let input = dashboard.read(cx).labels.iter().find(|(provider, ..)| *provider == metra_lib::model::Provider::Codex).unwrap().1.clone();
                        assert_eq!(input.read(cx).value().as_ref(), "TES");
                        dashboard.update(cx, |view,cx| view.save(Change::Label {provider:metra_lib::model::Provider::Codex,label:"OLD".into()},cx));
                        cx.global::<Runtime>().panel.unwrap().update(cx, |_,window,cx| {
                            input.update(cx, |input,cx| input.set_value("NEW",window,cx));
                        }).unwrap();
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
                    5=>{
                        let bubble = cx.global::<Runtime>().bubble.unwrap();
                        bubble.update(cx, |_,window,cx| {
                            let (work,_) = native_window::work_area(window,cx).unwrap();
                            native_window::move_to(window,(work.x+work.width+100.) as i32,(work.y+work.height+100.) as i32).unwrap();
                        }).unwrap();
                        recover_windows(cx);
                        bubble.update(cx, |_,window,cx| {
                            let (work,scale) = native_window::work_area(window,cx).unwrap();
                            let (x,y) = native_window::position(window).unwrap();
                            assert!(x as f64 >= work.x && y as f64 >= work.y);
                            assert!(x as f64 + 32.*scale <= work.x+work.width+2.);
                            assert!(y as f64 + 56.*scale <= work.y+work.height+2.);
                        }).unwrap();
                        let input = &dashboard.read(cx).labels.iter().find(|(provider, ..)| *provider == metra_lib::model::Provider::Codex).unwrap().1;
                        assert_eq!(input.read(cx).value().as_ref(), "NEW");
                        assert_eq!(dashboard.read(cx).service.store.load().codex_bubble_label, "OLD");
                    },
                    6=> smoke_click(gpui_kit::MouseButton::Left, cx),
                    7=> {
                        assert!(dashboard.read(cx).page == Page::Details);
                        assert!(cx.global::<Runtime>().panel.is_some());
                        smoke_click(gpui_kit::MouseButton::Left, cx);
                    },
                    8=> {
                        assert!(cx.global::<Runtime>().panel.is_none(), "bubble click must close details");
                        smoke_click(gpui_kit::MouseButton::Right, cx);
                    },
                    9=>{
                        assert!(dashboard.read(cx).page == Page::Settings);
                        let panel = cx.global::<Runtime>().panel.unwrap();
                        panel.update(cx, |_,window,cx| {
                            dashboard.update(cx, |view,cx| view.navigate(Page::Consent,window,cx));
                        }).unwrap();
                    },
                    10=>{
                        assert!(dashboard.read(cx).page == Page::Consent);
                        let panel = cx.global::<Runtime>().panel.unwrap();
                        panel.update(cx, |_,window,cx| {
                            let (work,scale) = native_window::work_area(window,cx).unwrap();
                            let expected = 300f64.min(work.height/scale-16.).max(160.);
                            assert!((f32::from(window.bounds().size.height) as f64-expected).abs()<=1., "Consent must resize the panel");
                            dashboard.update(cx, |view,cx| view.navigate(Page::Details,window,cx));
                        }).unwrap();
                    },
                    _=>{
                        assert!(dashboard.read(cx).page == Page::Details);
                        assert!(cx.global::<Runtime>().panel.is_some());
                        let panel = cx.global::<Runtime>().panel.unwrap();
                        panel.update(cx, |_,window,cx| {
                            window.dispatch_event(PlatformInput::KeyDown(KeyDownEvent { keystroke:Keystroke::parse("escape").unwrap(),is_held:false,prefer_character_input:false }),cx);
                        }).unwrap();
                        assert!(cx.global::<Runtime>().panel.is_none(), "Escape must close the panel");
                        std::fs::write(std::path::Path::new(&config).join("native-smoke.result"),"PASS: panel recreation, display modes, locale, settings persistence, label drafts, offscreen recovery, bubble clicks, consent navigation, Escape close").unwrap();
                        cx.quit();
                    },
                }
            });
        }
    }).detach();
}

#[cfg(feature = "native-smoke")]
fn smoke_click(button: gpui_kit::MouseButton, cx: &mut App) {
    cx.global::<Runtime>()
        .bubble
        .unwrap()
        .update(cx, |_, window, cx| {
            let position = point(
                window.bounds().size.width / 2.,
                window.bounds().size.height / 2.,
            );
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    modifiers: Default::default(),
                    pressed_button: None,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button,
                    position,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    button,
                    position,
                    modifiers: Default::default(),
                    click_count: 1,
                }),
                cx,
            );
        })
        .unwrap();
}

#[cfg(feature = "native-smoke")]
fn migration_smoke(cx: &mut App) {
    let config = std::env::var_os("METRA_CONFIG_DIR").expect("isolated configuration required");
    cx.spawn(async move |cx| {
        for _ in 0..100 {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            let done = cx.update(|cx| {
                let runtime = cx.global::<Runtime>();
                let saved = runtime.dashboard.read(cx).service.store.load();
                if saved.bubble_position_version != 3 {
                    return false;
                }
                use metra_lib::{
                    model::Provider,
                    settings::{BubblePercentMode, DisplayMode},
                };
                assert_eq!(saved.display_mode, DisplayMode::StatusBar);
                assert_eq!(saved.bubble_percent_mode, BubblePercentMode::Used);
                assert_eq!(saved.cursor_bubble_label, "CU");
                assert_eq!(
                    saved.bubble_provider_order,
                    vec![Provider::Codex, Provider::Cursor, Provider::Claude]
                );
                assert_eq!(
                    saved.bubble_visible_providers,
                    vec![Provider::Codex, Provider::Cursor]
                );
                assert!(!saved.autostart);
                assert_eq!(runtime.bubble.is_none(), runtime._tray.is_some());
                std::fs::write(
                    std::path::Path::new(&config).join("native-smoke.result"),
                    "PASS: legacy status-bar preferences and coordinate migration",
                )
                .unwrap();
                cx.quit();
                true
            });
            if done {
                return;
            }
        }
        panic!("Legacy coordinate migration did not finish");
    })
    .detach();
}
