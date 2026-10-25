//! Tauri desktop shell. Collectors and persisted settings remain UI-independent.
use crate::{
    diagnostics,
    geometry::WorkArea,
    i18n::I18n,
    model::Provider,
    platform,
    service::{AppPayload, RefreshService},
    settings::{
        AppSettings, BUBBLE_COLOR_PALETTE, BubblePercentMode, DisplayMode, SavedPosition,
        SettingsStore, UiLanguage,
    },
    status_bar, tray,
    updater::{UpdateStatus, Updater},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

type Result<T> = std::result::Result<T, String>;

struct Desktop {
    service: Arc<RefreshService>,
    updater: Arc<Updater>,
    panel: Mutex<String>,
    peek: AtomicBool,
    native_glass: AtomicBool,
    dragging: AtomicBool,
    login_running: AtomicBool,
    refreshing_providers: AtomicU8,
    tray_cycle: AtomicUsize,
    #[cfg(feature = "desktop-smoke")]
    tray_observed_provider: AtomicU8,
    settings_gate: Mutex<()>,
    bubble_gate: Mutex<()>,
    refresh_gate: Mutex<()>,
    last_refresh: Mutex<Instant>,
    _instance_lock: File,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopPayload {
    #[serde(flatten)]
    data: AppPayload,
    entries: Vec<status_bar::StatusEntry>,
    locale: String,
    translations: std::collections::BTreeMap<String, String>,
    palette: &'static [&'static str],
    update: UpdateStatus,
    can_install: bool,
    version: &'static str,
    panel: String,
    peek: bool,
    native_glass: bool,
    login_running: bool,
    refreshing_providers: Vec<Provider>,
}
impl Desktop {
    fn payload(&self) -> DesktopPayload {
        let mut data = self.service.payload();
        let t = I18n::new(data.settings.ui_language);
        for snapshot in [
            &mut data.snapshot.cursor,
            &mut data.snapshot.codex,
            &mut data.snapshot.claude,
        ] {
            snapshot.message = snapshot
                .message
                .as_deref()
                .map(|message| t.message(message));
        }
        DesktopPayload {
            entries: status_bar::entries(&data.snapshot, &data.settings),
            locale: t.locale.into(),
            translations: t.catalog(),
            data,
            palette: &BUBBLE_COLOR_PALETTE,
            update: self.updater.status(),
            can_install: self.updater.supports_install(),
            version: env!("CARGO_PKG_VERSION"),
            panel: self.panel.lock().expect("panel mode").clone(),
            peek: self.peek.load(Ordering::Acquire),
            native_glass: self.native_glass.load(Ordering::Acquire),
            login_running: self.login_running.load(Ordering::Acquire),
            refreshing_providers: [Provider::Cursor, Provider::Codex, Provider::Claude]
                .into_iter()
                .filter(|provider| {
                    self.refreshing_providers.load(Ordering::Acquire) & provider_mask(*provider)
                        != 0
                })
                .collect(),
        }
    }
}
fn publish(app: &AppHandle) {
    let state = app.state::<Desktop>();
    let _ = app.emit("metra-state", state.payload());
    update_tray(app, state.tray_cycle.load(Ordering::Relaxed));
}
fn update_tray(app: &AppHandle, cycle: usize) {
    let state = app.state::<Desktop>();
    match tray::update(app, &state.service.payload(), cycle) {
        Ok(_provider) => {
            #[cfg(feature = "desktop-smoke")]
            state
                .tray_observed_provider
                .store(_provider.map(provider_mask).unwrap_or(0), Ordering::Relaxed);
        }
        Err(error) => diagnostics::warn("tray.update_failed", error),
    }
}

fn report(app: &AppHandle, result: Result<()>) {
    if let Err(error) = result {
        diagnostics::warn("desktop.operation_failed", &error);
        let _ = app.emit("metra-error", error);
    }
}
#[tauri::command]
fn get_state(state: State<'_, Desktop>) -> DesktopPayload {
    state.payload()
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
enum Change {
    Display(DisplayMode),
    Percent(BubblePercentMode),
    Refresh(u64),
    Language(UiLanguage),
    Snap(bool),
    Autostart(bool),
    Compat(bool),
    Visible { provider: Provider, visible: bool },
    Label { provider: Provider, label: String },
    Color { provider: Provider, color: String },
    Order(Vec<Provider>),
}
impl Change {
    fn apply(&self, settings: &mut AppSettings) {
        match self {
            Self::Display(v) => settings.display_mode = *v,
            Self::Percent(v) => settings.bubble_percent_mode = *v,
            Self::Refresh(v) => settings.refresh_minutes = *v,
            Self::Language(v) => settings.ui_language = *v,
            Self::Snap(v) => settings.bubble_snap_enabled = *v,
            Self::Autostart(v) => settings.autostart = *v,
            Self::Compat(v) => settings.cursor_compat_enabled = *v,
            Self::Visible { provider, visible } => {
                settings.bubble_visible_providers.retain(|p| p != provider);
                if *visible {
                    settings.bubble_visible_providers.push(*provider);
                }
            }
            Self::Order(order) => settings.bubble_provider_order = order.clone(),
            Self::Label { provider, label } => {
                *match provider {
                    Provider::Cursor => &mut settings.cursor_bubble_label,
                    Provider::Codex => &mut settings.codex_bubble_label,
                    Provider::Claude => &mut settings.claude_bubble_label,
                } = label.clone()
            }
            Self::Color { provider, color } => {
                *match provider {
                    Provider::Cursor => &mut settings.cursor_bubble_color,
                    Provider::Codex => &mut settings.codex_bubble_color,
                    Provider::Claude => &mut settings.claude_bubble_color,
                } = color.clone()
            }
        }
    }
}
fn change_settings(app: &AppHandle, change: Change) -> Result<()> {
    let state = app.state::<Desktop>();
    let _guard = state.settings_gate.lock().map_err(|e| e.to_string())?;
    let previous = state.service.payload().settings;
    if let Change::Visible {
        provider,
        visible: false,
    } = &change
        && previous.bubble_visible_providers.len() == 1
        && previous.bubble_visible_providers.contains(provider)
    {
        return Err(I18n::new(previous.ui_language).t("config.keepOne"));
    }
    if matches!(change, Change::Display(DisplayMode::StatusBar))
        && app.tray_by_id("metra-status").is_none()
    {
        return Err("Status bar is unavailable on this desktop".into());
    }
    if let Change::Autostart(enabled) = change {
        platform::set_autostart(enabled)?;
    }
    let saved = state
        .service
        .update_settings(|settings| change.apply(settings));
    if let Err(error) = saved {
        if matches!(change, Change::Autostart(_)) {
            let _ = platform::set_autostart(previous.autostart);
        }
        return Err(error);
    }
    if let Some(bubble) = app.get_webview_window("bubble") {
        match change {
            Change::Display(DisplayMode::Bubble) => {
                bubble.show().map_err(|e| e.to_string())?;
            }
            Change::Display(DisplayMode::StatusBar) => {
                bubble.hide().map_err(|e| e.to_string())?;
            }
            Change::Snap(enabled) => set_peek(app, enabled, enabled)?,
            _ => {}
        }
    }
    tray::rebuild(app, &state.service.payload()).map_err(|e| e.to_string())?;
    publish(app);
    if matches!(change, Change::Compat(true)) {
        spawn_refresh(app, false);
    }
    Ok(())
}
#[tauri::command]
async fn change_setting(app: AppHandle, change: Change) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || change_settings(&app, change))
        .await
        .map_err(|e| e.to_string())?
}
fn provider_mask(provider: Provider) -> u8 {
    match provider {
        Provider::Cursor => 1,
        Provider::Codex => 2,
        Provider::Claude => 4,
    }
}
fn run_refresh(app: &AppHandle, recheck: bool) {
    let state = app.state::<Desktop>();
    // Tray clicks and sign-in rechecks share the same provider progress lifecycle.
    let Ok(_guard) = state.refresh_gate.try_lock() else {
        return;
    };
    *state.last_refresh.lock().expect("refresh time") = Instant::now();
    let progress = |provider, running| {
        if running {
            state
                .refreshing_providers
                .fetch_or(provider_mask(provider), Ordering::AcqRel);
        } else {
            state
                .refreshing_providers
                .fetch_and(!provider_mask(provider), Ordering::AcqRel);
        }
        publish(app);
    };
    crate::providers::discovery::invalidate_shell_cache();
    if recheck {
        state
            .service
            .refresh_cursor_login_status_with_progress(progress);
    } else {
        state.service.refresh_with_progress(true, progress);
    }
    publish(app);
}
fn spawn_refresh(app: &AppHandle, recheck: bool) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || run_refresh(&app, recheck));
}
#[tauri::command]
async fn refresh(app: AppHandle, recheck: bool) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || run_refresh(&app, recheck))
        .await
        .map_err(|e| e.to_string())
}
fn monitor_work(monitor: &tauri::Monitor) -> (WorkArea, f64) {
    let work = monitor.work_area();
    (
        WorkArea {
            x: work.position.x as f64,
            y: work.position.y as f64,
            width: work.size.width as f64,
            height: work.size.height as f64,
        },
        monitor.scale_factor(),
    )
}
fn work_area(window: &WebviewWindow) -> Result<(WorkArea, f64)> {
    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or(window.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("No display available")?;
    Ok(monitor_work(&monitor))
}
fn refresh_glass(app: &AppHandle, window: &WebviewWindow, radius: f64) {
    if let Err(error) = platform::apply_glass(window, radius) {
        diagnostics::warn("desktop.glass_unavailable", error);
        app.state::<Desktop>()
            .native_glass
            .store(false, Ordering::Release);
    }
}
fn save_position(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble unavailable")?;
    let state = app.state::<Desktop>();
    let mut p = window.outer_position().map_err(|e| e.to_string())?;
    if state.peek.load(Ordering::Acquire) {
        let (work, scale) = work_area(&window)?;
        let (x, y, _) = work.dock(p.x as f64, p.y as f64, scale, false);
        p = PhysicalPosition::new(x, y);
    }
    state.service.update_settings(|s| {
        s.bubble_position = Some(SavedPosition { x: p.x, y: p.y });
        s.bubble_position_version = 3; // Tauri physical desktop coordinates.
    })?;
    Ok(())
}
fn set_peek(app: &AppHandle, peek: bool, force: bool) -> Result<()> {
    let state = app.state::<Desktop>();
    let _guard = state.bubble_gate.lock().map_err(|e| e.to_string())?;
    if state.dragging.load(Ordering::Acquire) {
        return Ok(());
    }
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble unavailable")?;
    let (work, scale) = work_area(&window)?;
    let p = window.outer_position().map_err(|e| e.to_string())?;
    if peek {
        if !state.service.payload().settings.bubble_snap_enabled {
            return Ok(());
        }
        let near_edge = (p.x as f64 - work.x).abs() < 12. * scale
            || (p.x as f64 + 56. * scale - work.x - work.width).abs() < 12. * scale;
        if !force && !near_edge {
            return Ok(());
        }
    }
    let was_peek = state.peek.load(Ordering::Acquire);
    if !peek && !was_peek {
        return Ok(());
    }
    let (x, y) = if peek || was_peek {
        let (x, y, _) = work.dock(p.x as f64, p.y as f64, scale, peek);
        (x, y)
    } else {
        work.clamp(p.x as f64, p.y as f64, 56. * scale, 56. * scale)
    };
    window
        .set_size(LogicalSize::new(if peek { 32. } else { 56. }, 56.))
        .map_err(|e| e.to_string())?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    refresh_glass(app, &window, if peek { 16. } else { 28. });
    state.peek.store(peek, Ordering::Release);
    save_position(app)?;
    publish(app);
    Ok(())
}
#[tauri::command]
async fn bubble_hover(app: AppHandle, hovered: bool) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || set_peek(&app, !hovered, false))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn drag_bubble(app: AppHandle) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Desktop>();
        set_peek(&app, false, false)?;
        if state.dragging.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let window = app
            .get_webview_window("bubble")
            .ok_or("Bubble unavailable")?;
        let result: Result<()> = (|| {
            // Acrylic is expensive during native window moves on some Windows builds.
            #[cfg(windows)]
            window.set_effects(None).map_err(|e| e.to_string())?;
            window.start_dragging().map_err(|e| e.to_string())?;
            // Windows returns after its native move loop; macOS/Linux return immediately.
            let deadline = Instant::now() + Duration::from_secs(120);
            while platform::primary_button_pressed()? && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(())
        })();
        state.dragging.store(false, Ordering::Release);
        refresh_glass(&app, &window, 28.);
        result?;
        if state.service.payload().settings.bubble_snap_enabled {
            set_peek(&app, true, true)?;
        } else {
            let (work, scale) = work_area(&window)?;
            let p = window.outer_position().map_err(|e| e.to_string())?;
            let (x, y) = work.clamp(p.x as f64, p.y as f64, 56. * scale, 56. * scale);
            window
                .set_position(PhysicalPosition::new(x, y))
                .map_err(|e| e.to_string())?;
        }
        save_position(&app)
    })
    .await
    .map_err(|e| e.to_string())?
}
fn show_panel(app: &AppHandle, mode: &str, toggle: bool) -> Result<()> {
    if !["details", "settings", "consent"].contains(&mode) {
        return Err("Unknown panel".into());
    }
    let state = app.state::<Desktop>();
    let panel = app.get_webview_window("panel").ok_or("Panel unavailable")?;
    if toggle
        && panel.is_visible().unwrap_or(false)
        && *state.panel.lock().expect("panel mode") == mode
    {
        return panel.hide().map_err(|e| e.to_string());
    }
    *state.panel.lock().expect("panel mode") = mode.into();
    let settings = state.service.payload().settings;
    let bubble = app
        .get_webview_window("bubble")
        .ok_or("Bubble unavailable")?;
    let (mut work, mut scale) = work_area(&bubble)?;
    let tray_position = if settings.display_mode == DisplayMode::StatusBar {
        app.tray_by_id("metra-status")
            .and_then(|tray| tray.rect().ok().flatten())
            .map(|rect| rect.position.to_physical::<f64>(scale))
    } else {
        None
    };
    if let Some(p) = tray_position
        && let Some(monitor) = bubble
            .monitor_from_point(p.x, p.y)
            .map_err(|e| e.to_string())?
    {
        (work, scale) = monitor_work(&monitor);
    }
    let width: f64 = if mode == "settings" { 340. } else { 360. };
    let height: f64 = if mode == "consent" { 300. } else { 560. };
    let height = height.min(work.height / scale - 16.).max(160.);
    panel
        .set_size(tauri::PhysicalSize::new(
            (width * scale).round() as u32,
            (height * scale).round() as u32,
        ))
        .map_err(|e| e.to_string())?;
    let mut x = work.x + work.width - (width + 12.) * scale;
    let mut y = work.y + 12. * scale;
    if settings.display_mode == DisplayMode::Bubble {
        let p = bubble.outer_position().map_err(|e| e.to_string())?;
        x = p.x as f64 - (width + 8.) * scale;
        if x < work.x {
            x = p.x as f64 + 64. * scale;
        }
        y = p.y as f64;
    } else if let Some(p) = tray_position {
        x = p.x - width * scale;
        y = p.y + 24. * scale;
    }
    let (x, y) = work.clamp(x, y, width * scale, height * scale);
    panel
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    refresh_glass(app, &panel, 22.);
    publish(app);
    panel
        .show()
        .and_then(|_| panel.set_focus())
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn open_panel(app: AppHandle, mode: String, toggle: bool) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || show_panel(&app, &mode, toggle))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn hide_panel(app: AppHandle) -> Result<()> {
    app.get_webview_window("panel")
        .ok_or("Panel unavailable")?
        .hide()
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

fn open_url(url: &str) -> Result<()> {
    let mut command = std::process::Command::new(if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer.exe"
    } else {
        "xdg-open"
    });
    command.arg(url);
    crate::providers::scrub_sensitive_child_environment(&mut command);
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}
#[tauri::command]
async fn login_cursor(app: AppHandle) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Desktop>();
        if state.login_running.swap(true, Ordering::AcqRel) {
            return Err("Cursor sign-in is already running".into());
        }
        publish(&app);
        let result = (|| {
            let executable = crate::providers::cursor::cursor_login_executable();
            if !state.service.cursor_compat.load(Ordering::Acquire)
                && let Some(executable) = executable
            {
                let mut command = crate::providers::discovery::command_for(&executable, &["login"]);
                crate::providers::scrub_sensitive_child_environment(&mut command);
                command
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(0x08000000);
                }
                let mut child = command.spawn().map_err(|e| e.to_string())?;
                let deadline = Instant::now() + Duration::from_secs(300);
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) if status.success() => break,
                        Ok(Some(_)) => return Err("Cursor sign-in was not completed".into()),
                        Ok(None) if Instant::now() < deadline => {
                            std::thread::sleep(Duration::from_millis(100))
                        }
                        _ => {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err("Cursor sign-in timed out".into());
                        }
                    }
                }
                run_refresh(&app, true);
            } else {
                open_url("cursor://anysphere.cursor-deeplink/settings/plan-usage")?;
            }
            Ok(())
        })();
        state.login_running.store(false, Ordering::Release);
        publish(&app);
        result
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Desktop>();
        state.updater.check();
        publish(&app);
        state.updater.status().error.map_or(Ok(()), Err)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Desktop>();
        if state.updater.supports_install() {
            state.updater.install_windows()?;
            app.exit(0);
            Ok(())
        } else {
            let version = state
                .updater
                .status()
                .version
                .ok_or("No update available")?;
            open_url(&crate::updater::release_page(&version).ok_or("Invalid version")?)
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
fn dispatch_tray_action(app: &AppHandle, id: &str) -> Result<()> {
    match id {
        "details" => show_panel(app, "details", true),
        "settings" => show_panel(app, "settings", false),
        "refresh" => {
            spawn_refresh(app, false);
            Ok(())
        }
        "display" => {
            let current = app
                .state::<Desktop>()
                .service
                .payload()
                .settings
                .display_mode;
            change_settings(
                app,
                Change::Display(if current == DisplayMode::Bubble {
                    DisplayMode::StatusBar
                } else {
                    DisplayMode::Bubble
                }),
            )
        }
        "quit" => {
            app.exit(0);
            Ok(())
        }
        _ => Err(format!("Unknown tray action: {id}")),
    }
}
pub(crate) fn tray_action(app: &AppHandle, id: &str) {
    let app = app.clone();
    let id = id.to_owned();
    tauri::async_runtime::spawn_blocking(move || report(&app, dispatch_tray_action(&app, &id)));
}
fn watch_overlay_shape(window: &WebviewWindow, bubble: bool) {
    let overlay = window.clone();
    let previous = Mutex::new(None);
    window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. }
        ) {
            let Ok(size) = overlay.outer_size() else {
                return;
            };
            let Ok(scale) = overlay.scale_factor() else {
                return;
            };
            let shape = (size.width, size.height, scale);
            let Ok(mut last) = previous.lock() else {
                return;
            };
            if *last == Some(shape) {
                return;
            }
            *last = Some(shape);
            drop(last);
            let radius = if bubble {
                size.width.min(size.height) as f64 / scale / 2.
            } else {
                22.
            };
            if let Err(error) = platform::update_shape(&overlay, radius) {
                diagnostics::warn("overlay.shape_failed", error);
            }
        }
    });
}
fn create_windows(app: &AppHandle) -> Result<()> {
    let bubble = WebviewWindowBuilder::new(
        app,
        "bubble",
        WebviewUrl::App("index.html?view=bubble".into()),
    )
    .title("Metra")
    .inner_size(56., 56.)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(false)
    .disable_drag_drop_handler()
    .build()
    .map_err(|e| e.to_string())?;
    let panel = WebviewWindowBuilder::new(
        app,
        "panel",
        WebviewUrl::App("index.html?view=panel".into()),
    )
    .title("Metra")
    .inner_size(360., 560.)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .disable_drag_drop_handler()
    .build()
    .map_err(|e| e.to_string())?;
    watch_overlay_shape(&bubble, true);
    watch_overlay_shape(&panel, false);
    bubble.on_window_event(|event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
        }
    });
    let glass =
        platform::apply_glass(&bubble, 28.).and_then(|_| platform::apply_glass(&panel, 22.));
    app.state::<Desktop>()
        .native_glass
        .store(glass.is_ok(), Ordering::Release);
    if let Err(error) = glass {
        diagnostics::warn("desktop.glass_unavailable", error);
    }
    let handle = app.clone();
    panel.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = hide_panel(handle.clone());
        }
        if matches!(event, tauri::WindowEvent::Focused(false)) {
            let _ = hide_panel(handle.clone());
        }
    });
    let settings = app.state::<Desktop>().service.payload().settings;
    let monitors = bubble.available_monitors().map_err(|e| e.to_string())?;
    let saved = settings.bubble_position;
    let monitor = monitors
        .into_iter()
        .find(|m| {
            saved.is_some_and(|p| {
                let (x, y) = crate::geometry::saved_physical(
                    p,
                    settings.bubble_position_version,
                    m.scale_factor(),
                );
                let w = m.work_area();
                x >= w.position.x as f64
                    && x < w.position.x as f64 + w.size.width as f64
                    && y >= w.position.y as f64
                    && y < w.position.y as f64 + w.size.height as f64
            })
        })
        .or(bubble.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("No display available")?;
    let w = monitor.work_area();
    let scale = monitor.scale_factor();
    let work = WorkArea {
        x: w.position.x as f64,
        y: w.position.y as f64,
        width: w.size.width as f64,
        height: w.size.height as f64,
    };
    let (x, y) = saved
        .map(|p| crate::geometry::saved_physical(p, settings.bubble_position_version, scale))
        .unwrap_or((
            work.x + work.width - 64. * scale,
            work.y + work.height - 64. * scale,
        ));
    let (x, y) = work.clamp(x, y, 56. * scale, 56. * scale);
    bubble
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    save_position(app)?;
    if settings.bubble_snap_enabled {
        set_peek(app, true, true)?;
    }
    if settings.display_mode == DisplayMode::Bubble || app.tray_by_id("metra-status").is_none() {
        bubble.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn run() {
    let _ = diagnostics::init();
    if let Err(error) = run_desktop() {
        diagnostics::warn("desktop.start_failed", &error);
        eprintln!("{error}");
    }
}
fn run_desktop() -> Result<()> {
    #[cfg(target_os = "linux")]
    if std::env::var_os("GDK_BACKEND").is_none() {
        // SAFETY: the desktop entry point has not initialized GTK or spawned any threads.
        unsafe { std::env::set_var("GDK_BACKEND", "x11") };
    }
    let config = crate::settings::config_dir()?;
    std::fs::create_dir_all(&config).map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(config.join("instance.lock"))
        .map_err(|e| e.to_string())?;
    if lock.try_lock().is_err() {
        std::fs::write(config.join("activate"), b"show").map_err(|e| e.to_string())?;
        return Ok(());
    }
    let service = Arc::new(RefreshService::new(SettingsStore::new(
        config.join("settings.db"),
    )));
    let app = tauri::Builder::default()
        .manage(Desktop {
            service,
            updater: Arc::new(Updater::default()),
            panel: Mutex::new("details".into()),
            peek: AtomicBool::new(false),
            native_glass: AtomicBool::new(false),
            dragging: AtomicBool::new(false),
            login_running: AtomicBool::new(false),
            refreshing_providers: AtomicU8::new(0),
            tray_cycle: AtomicUsize::new(0),
            #[cfg(feature = "desktop-smoke")]
            tray_observed_provider: AtomicU8::new(0),
            settings_gate: Mutex::new(()),
            bubble_gate: Mutex::new(()),
            refresh_gate: Mutex::new(()),
            last_refresh: Mutex::new(Instant::now()),
            _instance_lock: lock,
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            change_setting,
            refresh,
            bubble_hover,
            drag_bubble,
            open_panel,
            hide_panel,
            quit,
            login_cursor,
            check_update,
            install_update
        ])
        .setup(move |app| {
            // System menus must follow the OS; only overlay windows use a dark appearance.
            app.set_theme(None);
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            if let Err(error) = tray::setup(app.handle(), &app.state::<Desktop>().service.payload())
            {
                diagnostics::warn("tray.setup_failed", error.to_string());
            }
            create_windows(app.handle())?;
            if std::env::args().any(|arg| arg == "--settings") {
                show_panel(app.handle(), "settings", false)?;
            }
            let smoke_enabled = cfg!(feature = "desktop-smoke")
                && std::env::args().any(|arg| arg == "--desktop-smoke-test");
            if !smoke_enabled {
                spawn_refresh(app.handle(), false);
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let started = Instant::now();
                let mut last_update = None;
                let mut tray_tick = 0u8;
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let state = handle.state::<Desktop>();
                    if config.join("activate").is_file() {
                        let _ = std::fs::remove_file(config.join("activate"));
                        report(&handle, show_panel(&handle, "details", false));
                    }
                    let settings = state.service.payload().settings;
                    if !smoke_enabled
                        && state.last_refresh.lock().expect("refresh time").elapsed()
                            >= Duration::from_secs(settings.refresh_minutes * 60)
                    {
                        spawn_refresh(&handle, false);
                    }
                    tray_tick = (tray_tick + 1) % 5;
                    if tray_tick == 0 {
                        let cycle = state
                            .tray_cycle
                            .fetch_add(1, Ordering::Relaxed)
                            .wrapping_add(1);
                        update_tray(&handle, cycle);
                    }
                    if !smoke_enabled
                        && !cfg!(debug_assertions)
                        && started.elapsed() >= Duration::from_secs(20)
                        && last_update
                            .is_none_or(|at: Instant| at.elapsed() >= Duration::from_secs(86400))
                    {
                        last_update = Some(Instant::now());
                        let app = handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let _ = check_update(app).await;
                        });
                    }
                }
            });
            #[cfg(feature = "desktop-smoke")]
            if smoke_enabled {
                smoke::start(app.handle().clone());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .map_err(|e| e.to_string())?;
    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => {
            let _ = show_panel(app, "details", false);
        }
        _ => {
            let _ = app;
        }
    });
    Ok(())
}

#[cfg(feature = "desktop-smoke")]
mod smoke;
