use std::{
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use tauri::{menu::MenuBuilder, tray::TrayIconBuilder};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use crate::{
    diagnostics,
    model::Provider,
    providers::{
        scrub_sensitive_child_environment,
        cursor::cursor_login_executable,
        discovery::{ResolvedExecutable, command_for, invalidate_shell_cache},
    },
    service::{AppPayload, RefreshService},
    settings::{AppSettings, BubblePercentMode, REFRESH_INTERVALS, SavedPosition, SettingsStore},
};

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    #[link_name = "CGEventSourceButtonState"]
    fn cg_event_source_button_state(state_id: i32, button: u32) -> bool;
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    #[link_name = "GetAsyncKeyState"]
    fn get_async_key_state(vkey: i32) -> i16;
}

const PANEL_MODE_DETAILS: u8 = 1;
const PANEL_MODE_MENU: u8 = 2;
const CURSOR_SETTINGS_DEEP_LINK: &str =
    "cursor://anysphere.cursor-deeplink/settings/plan-usage";
const CURSOR_LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const CURSOR_AGENT_LOGIN_ARGS: &[&str] = &["login"];

#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_DETAILS_ID: &str = "tray-details";
#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_SETTINGS_ID: &str = "tray-settings";
#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_REFRESH_ID: &str = "tray-refresh";
#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_TOGGLE_BUBBLE_ID: &str = "tray-toggle-bubble";
#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_QUIT_ID: &str = "tray-quit";

#[derive(Default)]
struct PanelRequestState {
    latest_request: AtomicU64,
    latest_bubble_request: AtomicU64,
    visible_mode: AtomicU8,
}

#[derive(Default)]
struct CursorLoginState {
    agent_running: AtomicBool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CursorLoginStart {
    method: &'static str,
    already_running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CursorLoginLaunch {
    Agent(ResolvedExecutable),
    Editor,
}

fn cursor_login_launch(
    agent: Option<ResolvedExecutable>,
    compat_enabled: bool,
) -> CursorLoginLaunch {
    if compat_enabled {
        CursorLoginLaunch::Editor
    } else {
        agent.map_or(CursorLoginLaunch::Editor, CursorLoginLaunch::Agent)
    }
}

fn configure_login_command(command: &mut Command) {
    scrub_sensitive_child_environment(command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}

fn open_cursor_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(CURSOR_SETTINGS_DEEP_LINK);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("rundll32.exe");
        command.args(["url.dll,FileProtocolHandler", CURSOR_SETTINGS_DEEP_LINK]);
        command
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return Err("当前系统暂不支持打开 Cursor 登录".into());

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        configure_login_command(&mut command);
        let mut child = command.spawn().map_err(|error| {
            diagnostics::warn(
                "cursor.login.editor_open_failed",
                format!("kind={:?}", error.kind()),
            );
            "无法打开 Cursor 登录页面".to_string()
        })?;
        if wait_for_login_child(&mut child, Duration::from_secs(3)) {
            Ok(())
        } else {
            Err("Cursor 登录页面未能打开".into())
        }
    }
}

fn wait_for_login_child(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

fn should_hide_panel(toggle: bool, requested_mode: u8, visible_mode: u8, visible: bool) -> bool {
    toggle && visible && requested_mode == PANEL_MODE_DETAILS && visible_mode == PANEL_MODE_DETAILS
}

fn next_panel_request(requests: &PanelRequestState) -> u64 {
    requests.latest_request.fetch_add(1, Ordering::AcqRel) + 1
}

#[tauri::command]
fn is_primary_mouse_button_pressed() -> bool {
    #[cfg(target_os = "macos")]
    {
        // SAFETY: This reads the combined-session state for the fixed primary mouse button.
        unsafe { cg_event_source_button_state(0, 0) }
    }
    #[cfg(target_os = "windows")]
    {
        // SAFETY: This reads process-independent input state for the fixed VK_LBUTTON code.
        (unsafe { get_async_key_state(1) }) < 0
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    false
}

fn claim_bubble_panel_request(requests: &PanelRequestState, request_id: u64) -> bool {
    requests
        .latest_bubble_request
        .fetch_max(request_id, Ordering::AcqRel)
        < request_id
}

fn panel_bubble_x(
    bubble_x: i32,
    bubble_width: u32,
    full_width: u32,
    docked_right: bool,
) -> i32 {
    if !docked_right || bubble_width >= full_width {
        return bubble_x;
    }
    let adjusted = i64::from(bubble_x) + i64::from(bubble_width) - i64::from(full_width);
    adjusted.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn calculate_panel_position(
    bubble_x: i32,
    bubble_y: i32,
    bubble_width: u32,
    panel_width: u32,
    panel_height: u32,
    work_area: Option<(i32, i32, u32, u32)>,
    scale: f64,
) -> (i32, i32) {
    let gap = (3.0 * scale).round() as i64;
    let edge = (8.0 * scale).round() as i64;
    let left_candidate = i64::from(bubble_x) - i64::from(panel_width) - gap;
    let right_candidate = i64::from(bubble_x) + i64::from(bubble_width) + gap;
    let mut x = right_candidate;
    let mut y = i64::from(bubble_y);
    if let Some((work_x, work_y, work_width, work_height)) = work_area {
        let left = i64::from(work_x) + edge;
        let top = i64::from(work_y) + edge;
        let right = i64::from(work_x) + i64::from(work_width) - edge;
        let bottom = i64::from(work_y) + i64::from(work_height) - edge;
        x = if left_candidate >= left {
            left_candidate
        } else {
            right_candidate
        };
        x = x.clamp(left, (right - i64::from(panel_width)).max(left));
        y = y.clamp(top, (bottom - i64::from(panel_height)).max(top));
    }
    (
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    )
}

fn show_panel_window(
    mode: &str,
    toggle: bool,
    request_id: u64,
    app: &AppHandle,
    requests: &PanelRequestState,
) -> Result<u64, String> {
    let started = Instant::now();
    let (width, height, mode_code) = match mode {
        "details" => (340.0, 480.0, PANEL_MODE_DETAILS),
        "menu" => (252.0, 432.0, PANEL_MODE_MENU),
        _ => return Err("未知弹窗模式".into()),
    };
    let bubble = app
        .get_webview_window("bubble")
        .ok_or_else(|| "气泡窗口不可用".to_string())?;
    let panel = app
        .get_webview_window("panel")
        .ok_or_else(|| "详情窗口不可用".to_string())?;
    let panel_visible = panel.is_visible().unwrap_or(false);
    if should_hide_panel(
        toggle,
        mode_code,
        requests.visible_mode.load(Ordering::Acquire),
        panel_visible,
    ) {
        panel.hide().map_err(|_| "无法收起详情窗口".to_string())?;
        let _ = app.emit("panel-visibility-changed", serde_json::json!({ "visible": false }));
        return Ok(started.elapsed().as_millis() as u64);
    }
    panel
        .set_size(LogicalSize::new(width, height))
        .map_err(|_| "无法调整弹窗大小".to_string())?;
    if requests.latest_request.load(Ordering::Acquire) != request_id {
        return Ok(started.elapsed().as_millis() as u64);
    }
    let bubble_position = bubble
        .outer_position()
        .map_err(|_| "无法读取气泡位置".to_string())?;
    let bubble_size = bubble
        .outer_size()
        .map_err(|_| "无法读取气泡大小".to_string())?;
    let monitor = bubble.current_monitor().ok().flatten();
    let scale = monitor
        .as_ref()
        .map_or(1.0, |monitor| monitor.scale_factor());
    let work_area = monitor.as_ref().map(|monitor| {
        let work = monitor.work_area();
        (
            work.position.x,
            work.position.y,
            work.size.width,
            work.size.height,
        )
    });
    let full_bubble_width = (56.0 * scale).round() as u32;
    let docked_right = work_area.is_some_and(|(work_x, _, work_width, _)| {
        i64::from(bubble_position.x) + i64::from(bubble_size.width / 2)
            >= i64::from(work_x) + i64::from(work_width / 2)
    });
    let dock_side = if docked_right { "right" } else { "left" };
    let bubble_x = panel_bubble_x(
        bubble_position.x,
        bubble_size.width,
        full_bubble_width,
        docked_right,
    );
    if bubble_size.width < full_bubble_width {
        let _ = app.emit(
            "bubble-reveal-requested",
            serde_json::json!({ "side": dock_side }),
        );
    }
    let (x, y) = calculate_panel_position(
        bubble_x,
        bubble_position.y,
        full_bubble_width,
        (width * scale).round() as u32,
        (height * scale).round() as u32,
        work_area,
        scale,
    );
    panel
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|_| "无法定位弹窗".to_string())?;
    if requests.latest_request.load(Ordering::Acquire) != request_id {
        return Ok(started.elapsed().as_millis() as u64);
    }
    panel
        .emit(
            "panel-mode",
            serde_json::json!({ "mode": mode, "dockSide": dock_side }),
        )
        .map_err(|_| "无法切换弹窗内容".to_string())?;
    panel.show().map_err(|_| "无法显示弹窗".to_string())?;
    let _ = app.emit("panel-visibility-changed", serde_json::json!({ "visible": true }));
    requests.visible_mode.store(mode_code, Ordering::Release);
    panel.set_focus().map_err(|_| "无法聚焦弹窗".to_string())?;
    Ok(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64)
}

#[tauri::command]
fn show_panel(
    mode: String,
    toggle: bool,
    request_id: u64,
    app: AppHandle,
    requests: State<'_, PanelRequestState>,
) -> Result<u64, String> {
    if !claim_bubble_panel_request(requests.inner(), request_id) {
        return Ok(0);
    }
    let request_id = next_panel_request(requests.inner());
    show_panel_window(&mode, toggle, request_id, &app, requests.inner())
}
#[tauri::command]
fn get_app_payload(service: State<'_, Arc<RefreshService>>) -> AppPayload {
    service.payload()
}

#[tauri::command]
fn refresh_now(
    include_cursor: Option<bool>,
    app: AppHandle,
    service: State<'_, Arc<RefreshService>>,
) -> AppPayload {
    invalidate_shell_cache();
    let include_cursor = include_cursor.unwrap_or_else(|| {
        service
            .cursor_compat
            .load(std::sync::atomic::Ordering::Acquire)
    });
    spawn_refresh(app, service.inner().clone(), include_cursor);
    service.payload()
}

#[tauri::command]
fn start_cursor_login(
    app: AppHandle,
    service: State<'_, Arc<RefreshService>>,
    login: State<'_, Arc<CursorLoginState>>,
) -> Result<CursorLoginStart, String> {
    let compat_enabled = service.cursor_compat.load(Ordering::Acquire);
    match cursor_login_launch(cursor_login_executable(), compat_enabled) {
        CursorLoginLaunch::Agent(executable) => {
            if login.agent_running.swap(true, Ordering::AcqRel) {
                return Ok(CursorLoginStart {
                    method: "agent",
                    already_running: true,
                });
            }
            let mut command = command_for(&executable, CURSOR_AGENT_LOGIN_ARGS);
            configure_login_command(&mut command);
            let mut child = command.spawn().map_err(|error| {
                login.agent_running.store(false, Ordering::Release);
                diagnostics::warn(
                    "cursor.login.agent_spawn_failed",
                    format!("kind={:?}", error.kind()),
                );
                "无法启动 Cursor 登录".to_string()
            })?;
            let app = app.clone();
            let service = service.inner().clone();
            let login = login.inner().clone();
            std::thread::spawn(move || {
                let success = wait_for_login_child(&mut child, CURSOR_LOGIN_TIMEOUT);
                login.agent_running.store(false, Ordering::Release);
                if success {
                    spawn_cursor_login_refresh(app, service);
                } else {
                    let _ = app.emit(
                        "cursor-login-finished",
                        serde_json::json!({
                            "success": false,
                            "message": "Cursor 登录未完成，请重试"
                        }),
                    );
                }
            });
            Ok(CursorLoginStart {
                method: "agent",
                already_running: false,
            })
        }
        CursorLoginLaunch::Editor => {
            open_cursor_settings()?;
            Ok(CursorLoginStart {
                method: "editor",
                already_running: false,
            })
        }
    }
}

#[tauri::command]
async fn recheck_cursor_login(
    service: State<'_, Arc<RefreshService>>,
) -> Result<AppPayload, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = service.refresh_cursor_login_status();
        service.payload()
    })
    .await
    .map_err(|_| "Cursor 登录状态检测异常".to_string())
}

#[tauri::command]
fn set_refresh_interval(
    minutes: u64,
    service: State<'_, Arc<RefreshService>>,
) -> Result<AppSettings, String> {
    if !REFRESH_INTERVALS.contains(&minutes) {
        return Err("不支持的刷新间隔".into());
    }
    service.update_settings(|settings| settings.refresh_minutes = minutes)
}

#[tauri::command]
fn set_bubble_percent_mode(
    mode: BubblePercentMode,
    app: AppHandle,
    service: State<'_, Arc<RefreshService>>,
) -> Result<AppSettings, String> {
    let settings = service.update_settings(|settings| settings.bubble_percent_mode = mode)?;
    app.emit("settings-updated", settings.clone())
        .map_err(|_| "无法同步气泡百分比设置".to_string())?;
    Ok(settings)
}

#[tauri::command]
fn set_bubble_snap_enabled(
    enabled: bool,
    app: AppHandle,
    service: State<'_, Arc<RefreshService>>,
) -> Result<AppSettings, String> {
    let settings = service.update_settings(|settings| settings.bubble_snap_enabled = enabled)?;
    app.emit("settings-updated", settings.clone())
        .map_err(|_| "无法同步自动吸边设置".to_string())?;
    Ok(settings)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn set_bubble_display_config(
    order: Vec<Provider>,
    visible_providers: Vec<Provider>,
    cursor_label: String,
    codex_label: String,
    claude_label: String,
    cursor_color: String,
    codex_color: String,
    claude_color: String,
    app: AppHandle,
    service: State<'_, Arc<RefreshService>>,
) -> Result<AppSettings, String> {
    let settings = service.update_settings(|settings| {
        settings.bubble_provider_order = order;
        settings.bubble_visible_providers = visible_providers;
        settings.cursor_bubble_label = cursor_label;
        settings.codex_bubble_label = codex_label;
        settings.claude_bubble_label = claude_label;
        settings.cursor_bubble_color = cursor_color;
        settings.codex_bubble_color = codex_color;
        settings.claude_bubble_color = claude_color;
    })?;
    app.emit("settings-updated", settings.clone())
        .map_err(|_| "无法同步悬浮球显示设置".to_string())?;
    Ok(settings)
}

#[tauri::command]
fn set_cursor_compat(
    enabled: bool,
    app: AppHandle,
    service: State<'_, Arc<RefreshService>>,
) -> Result<AppSettings, String> {
    let settings = service.update_settings(|settings| settings.cursor_compat_enabled = enabled)?;
    queue_refresh(app, service.inner().clone());
    Ok(settings)
}

#[tauri::command]
fn set_autostart(
    enabled: bool,
    app: AppHandle,
    service: State<'_, Arc<RefreshService>>,
) -> Result<AppSettings, String> {
    if enabled {
        app.autolaunch()
            .enable()
            .map_err(|_| "无法启用开机启动".to_string())?;
    } else {
        app.autolaunch()
            .disable()
            .map_err(|_| "无法关闭开机启动".to_string())?;
    }
    service.update_settings(|settings| settings.autostart = enabled)
}

#[tauri::command]
fn save_window_position(
    x: i32,
    y: i32,
    service: State<'_, Arc<RefreshService>>,
) -> Result<AppSettings, String> {
    service.update_settings(|settings| {
        settings.bubble_position = Some(SavedPosition { x, y });
        settings.bubble_position_version = 1;
    })
}
#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn spawn_refresh(app: AppHandle, service: Arc<RefreshService>, include_cursor: bool) {
    if service
        .refreshing
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return;
    }
    std::thread::spawn(move || {
        let _ = service.refresh(include_cursor);
        let _ = app.emit("usage-updated", service.payload());
    });
}

fn spawn_cursor_login_refresh(app: AppHandle, service: Arc<RefreshService>) {
    std::thread::spawn(move || {
        while service.refreshing.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = service.refresh_cursor_login_status();
        let _ = app.emit("usage-updated", service.payload());
    });
}

fn queue_refresh(app: AppHandle, service: Arc<RefreshService>) {
    std::thread::spawn(move || {
        while service
            .refreshing
            .load(std::sync::atomic::Ordering::Acquire)
        {
            std::thread::sleep(Duration::from_millis(100));
        }
        spawn_refresh(app, service, true);
    });
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn setup_status_item(app: &mut tauri::App) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(TRAY_DETAILS_ID, "打开详情")
        .text(TRAY_SETTINGS_ID, "设置")
        .separator()
        .text(TRAY_REFRESH_ID, "立即刷新")
        .text(TRAY_TOGGLE_BUBBLE_ID, "显示/隐藏悬浮球")
        .separator()
        .text(TRAY_QUIT_ID, "退出 Metra")
        .build()?;

    let tray = TrayIconBuilder::with_id("metra-status")
        .icon({
            #[cfg(target_os = "macos")]
            {
                tauri::include_image!("./icons/trayTemplate.png")
            }
            #[cfg(target_os = "windows")]
            {
                tauri::include_image!("./icons/trayColor.png")
            }
        })
        .tooltip("Metra · AI 用量")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_DETAILS_ID | TRAY_SETTINGS_ID => {
                if let Some(bubble) = app.get_webview_window("bubble") {
                    let _ = bubble.show();
                }
                let mode = if event.id() == TRAY_DETAILS_ID {
                    "details"
                } else {
                    "menu"
                };
                let requests = app.state::<PanelRequestState>();
                let request_id = next_panel_request(requests.inner());
                let _ = show_panel_window(mode, false, request_id, app, requests.inner());
            }
            TRAY_REFRESH_ID => {
                invalidate_shell_cache();
                let service = app.state::<Arc<RefreshService>>().inner().clone();
                let include_cursor = service.cursor_compat.load(Ordering::Acquire);
                spawn_refresh(app.clone(), service, include_cursor);
            }
            TRAY_TOGGLE_BUBBLE_ID => {
                if let Some(bubble) = app.get_webview_window("bubble") {
                    if bubble.is_visible().unwrap_or(false) {
                        if let Some(panel) = app.get_webview_window("panel") {
                            let _ = panel.hide();
                            let _ = app.emit(
                                "panel-visibility-changed",
                                serde_json::json!({ "visible": false }),
                            );
                        }
                        let _ = bubble.hide();
                    } else {
                        let _ = bubble.show();
                    }
                }
            }
            TRAY_QUIT_ID => app.exit(0),
            _ => {}
        });
    #[cfg(target_os = "macos")]
    let tray = tray.icon_as_template(true);
    tray.build(app)?;
    Ok(())
}

fn start_scheduler(app: AppHandle, service: Arc<RefreshService>) {
    std::thread::spawn(move || {
        spawn_refresh(app.clone(), service.clone(), true);
        let mut last_refresh = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let minutes = service
                .settings
                .lock()
                .map(|settings| settings.refresh_minutes)
                .unwrap_or(5);
            if last_refresh.elapsed() >= Duration::from_secs(minutes * 60) {
                let include_cursor = service
                    .cursor_compat
                    .load(std::sync::atomic::Ordering::Acquire);
                spawn_refresh(app.clone(), service.clone(), include_cursor);
                last_refresh = Instant::now();
            }
        }
    });
}

pub fn run() {
    let _ = diagnostics::init();
    tauri::Builder::default()
        .manage(PanelRequestState::default())
        .manage(Arc::new(CursorLoginState::default()))
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("bubble") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .invoke_handler(tauri::generate_handler![
            show_panel,
            is_primary_mouse_button_pressed,
            get_app_payload,
            refresh_now,
            start_cursor_login,
            recheck_cursor_login,
            set_refresh_interval,
            set_bubble_percent_mode,
            set_bubble_snap_enabled,
            set_bubble_display_config,
            set_cursor_compat,
            set_autostart,
            save_window_position,
            quit_app
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let config_dir = app
                .path()
                .app_config_dir()
                .map_err(|error| error.to_string())?;
            let service = Arc::new(RefreshService::new(SettingsStore::new(
                config_dir.join("settings.db"),
            )));
            let settings = service.settings.lock().expect("settings lock").clone();

            if let Some(window) = app.get_webview_window("bubble") {
                let position = if settings.bubble_position_version >= 1 {
                    settings.bubble_position
                } else {
                    window
                        .primary_monitor()
                        .ok()
                        .flatten()
                        .zip(window.outer_size().ok())
                        .map(|(monitor, size)| {
                            let work = monitor.work_area();
                            SavedPosition {
                                x: work.position.x + work.size.width as i32 - size.width as i32 - 8,
                                y: work.position.y + work.size.height as i32
                                    - size.height as i32
                                    - 8,
                            }
                        })
                };
                if let Some(position) = position {
                    let _ = window.set_position(PhysicalPosition::new(position.x, position.y));
                    if settings.bubble_position_version < 1 {
                        let _ = service.update_settings(|settings| {
                            settings.bubble_position = Some(position);
                            settings.bubble_position_version = 1;
                        });
                    }
                }
                let _ = window.show();
            }
            let autostart_app = app.handle().clone();
            let autostart_enabled = settings.autostart;
            std::thread::spawn(move || {
                if autostart_enabled {
                    let _ = autostart_app.autolaunch().enable();
                } else {
                    let _ = autostart_app.autolaunch().disable();
                }
            });
            app.manage(service.clone());
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            setup_status_item(app)?;
            start_scheduler(app.handle().clone(), service);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Metra");
}
#[cfg(test)]
mod panel_geometry_tests {
    use super::{
        PANEL_MODE_DETAILS, PANEL_MODE_MENU, PanelRequestState, calculate_panel_position,
        claim_bubble_panel_request, next_panel_request, panel_bubble_x, should_hide_panel,
    };

    #[test]
    fn every_panel_entry_point_can_share_one_monotonic_sequence() {
        let requests = PanelRequestState::default();
        assert_eq!(next_panel_request(&requests), 1);
        assert_eq!(next_panel_request(&requests), 2);
        assert!(claim_bubble_panel_request(&requests, 1));
        assert_eq!(next_panel_request(&requests), 3);
    }

    #[test]
    fn older_bubble_request_is_rejected_without_affecting_tray_sequence() {
        let requests = PanelRequestState::default();
        assert!(claim_bubble_panel_request(&requests, 2));
        assert!(!claim_bubble_panel_request(&requests, 2));
        assert!(!claim_bubble_panel_request(&requests, 1));
        assert_eq!(next_panel_request(&requests), 1);
    }

    #[test]
    fn right_peek_position_is_restored_to_the_full_bubble_anchor_for_the_panel() {
        assert_eq!(panel_bubble_x(1168, 32, 56, true), 1144);
        assert_eq!(panel_bubble_x(1144, 56, 56, true), 1144);
    }

    #[test]
    fn left_peek_position_keeps_its_existing_anchor() {
        assert_eq!(panel_bubble_x(0, 32, 56, false), 0);
    }

    #[test]
    fn panel_prefers_the_left_when_both_sides_have_room() {
        assert_eq!(
            calculate_panel_position(600, 100, 56, 340, 480, Some((0, 0, 1200, 800)), 1.0),
            (257, 100)
        );
    }

    #[test]
    fn panel_falls_back_close_to_the_right_when_the_left_has_no_room() {
        assert_eq!(
            calculate_panel_position(69, 100, 70, 425, 600, Some((0, 0, 1500, 1000)), 1.25),
            (143, 100)
        );
    }

    #[test]
    fn panel_opens_on_the_left_when_the_bubble_is_near_the_right_edge() {
        assert_eq!(
            calculate_panel_position(1136, 100, 56, 340, 480, Some((0, 0, 1200, 800)), 1.0),
            (793, 100)
        );
    }

    #[test]
    fn visible_details_are_hidden_only_by_a_details_toggle() {
        assert!(should_hide_panel(
            true,
            PANEL_MODE_DETAILS,
            PANEL_MODE_DETAILS,
            true
        ));
        assert!(!should_hide_panel(
            false,
            PANEL_MODE_DETAILS,
            PANEL_MODE_DETAILS,
            true
        ));
        assert!(!should_hide_panel(
            true,
            PANEL_MODE_DETAILS,
            PANEL_MODE_MENU,
            true
        ));
        assert!(!should_hide_panel(
            true,
            PANEL_MODE_DETAILS,
            PANEL_MODE_DETAILS,
            false
        ));
    }
    #[test]
    fn panel_is_clamped_inside_the_monitor_work_area() {
        assert_eq!(
            calculate_panel_position(10, 760, 56, 340, 480, Some((0, 0, 1200, 800)), 1.0),
            (69, 312)
        );
    }
}

#[cfg(test)]
mod cursor_login_tests {
    use super::{
        CURSOR_AGENT_LOGIN_ARGS, CURSOR_SETTINGS_DEEP_LINK, CursorLoginLaunch,
        configure_login_command, cursor_login_launch,
    };
    use crate::providers::{
        ANTHROPIC_ADMIN_KEY_ENV, CLAUDE_API_KEY_NAME_ENV,
        discovery::ResolvedExecutable,
    };
    use std::{ffi::OsStr, path::PathBuf, process::Command};

    #[test]
    fn installed_agent_uses_only_the_official_login_subcommand() {
        let executable = ResolvedExecutable::from_path(PathBuf::from("/tmp/cursor-agent"));
        assert_eq!(
            cursor_login_launch(Some(executable.clone()), false),
            CursorLoginLaunch::Agent(executable)
        );
        assert_eq!(CURSOR_AGENT_LOGIN_ARGS, &["login"]);
    }

    #[test]
    fn missing_agent_opens_the_fixed_cursor_settings_deep_link() {
        assert_eq!(cursor_login_launch(None, false), CursorLoginLaunch::Editor);
        assert_eq!(
            CURSOR_SETTINGS_DEEP_LINK,
            "cursor://anysphere.cursor-deeplink/settings/plan-usage"
        );
    }

    #[test]
    fn authorized_compat_mode_uses_the_editor_login_even_when_agent_is_installed() {
        assert_eq!(
            cursor_login_launch(
                Some(ResolvedExecutable::from_path(PathBuf::from(
                    "/tmp/cursor-agent",
                ))),
                true,
            ),
            CursorLoginLaunch::Editor
        );
    }

    #[test]
    fn editor_login_commands_scrub_claude_usage_credentials() {
        let mut command = Command::new("open");
        command
            .env(ANTHROPIC_ADMIN_KEY_ENV, "sk-ant-admin01-secret")
            .env(CLAUDE_API_KEY_NAME_ENV, "Claude Code");

        configure_login_command(&mut command);

        let removed = command
            .get_envs()
            .filter_map(|(name, value)| value.is_none().then_some(name))
            .collect::<Vec<_>>();
        assert!(removed.contains(&OsStr::new(ANTHROPIC_ADMIN_KEY_ENV)));
        assert!(removed.contains(&OsStr::new(CLAUDE_API_KEY_NAME_ENV)));
    }
}
