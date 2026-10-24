use crate::{
    appearance, diagnostics,
    i18n::I18n,
    model::{Provider, ProviderSnapshot, ProviderStatus},
    platform,
    service::{AppPayload, RefreshService},
    settings::{
        AppSettings, BUBBLE_COLOR_PALETTE, BubblePercentMode, DisplayMode, REFRESH_INTERVALS,
        SavedPosition, SettingsStore, UiLanguage,
    },
    status_bar,
    tray::StatusTray,
    updater::Updater,
};
use gpui_kit::component::{
    Disableable, IconName, IndexPath, Sizable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    progress::Progress,
    select::{Select, SelectEvent, SelectState},
    switch::Switch,
};
use gpui_kit::{self as gpui, *};
use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};
use tray_icon::{
    MouseButton as TrayButton, MouseButtonState as TrayButtonState, TrayIconEvent, menu::MenuEvent,
};

const BUBBLE_SIZE: f32 = 56.;
const PEEK_SIZE: f32 = 32.;
const PANEL_WIDTH: f32 = 340.;
const MENU_WIDTH: f32 = 270.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Bubble,
    Details,
    Settings,
    Consent,
}
#[derive(Clone)]
enum Change {
    Display(DisplayMode),
    Percent(BubblePercentMode),
    Refresh(u64),
    Language(UiLanguage),
    Snap(bool),
    Autostart(bool),
    Compat(bool),
    Visible(Provider, bool),
    Label(Provider, String),
    Color(Provider, String),
    Reorder(Provider, i32),
    MoveBefore(Provider, Provider),
}
enum Event {
    Refreshed,
    Message(String),
    LoginFinished(bool),
    UpdateReady,
    #[cfg(windows)]
    InstallerStarted,
}
struct DesktopGlobal(Entity<Desktop>);
impl Global for DesktopGlobal {}

struct Desktop {
    service: Arc<RefreshService>,
    updater: Arc<Updater>,
    sender: Sender<Event>,
    receiver: Receiver<Event>,
    tray: Option<StatusTray>,
    bubble: Option<AnyWindowHandle>,
    panel: Option<AnyWindowHandle>,
    panel_mode: View,
    cycle: usize,
    last_refresh: Instant,
    last_cycle: Instant,
    last_second: Instant,
    started: Instant,
    update_checked: bool,
    message: Option<(String, Instant)>,
    login_running: Arc<AtomicBool>,
    activation_file: PathBuf,
    _instance_lock: File,
}

pub fn run() {
    let _ = diagnostics::init();
    let config = match crate::settings::config_dir() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    if let Err(error) = std::fs::create_dir_all(&config) {
        eprintln!("{error}");
        return;
    }
    let lock = match OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(config.join("instance.lock"))
    {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    if lock.try_lock().is_err() {
        let _ = std::fs::write(config.join("activate"), b"show");
        return;
    }
    let service = Arc::new(RefreshService::new(SettingsStore::new(
        config.join("settings.db"),
    )));
    #[cfg(target_os = "linux")]
    let application = gpui::platform::linux(WindowingModes::X11);
    #[cfg(not(target_os = "linux"))]
    let application = gpui::application();
    let application = application.with_assets(appearance::Assets);
    application.on_reopen(|cx| {
        if let Some(state) = cx.try_global::<DesktopGlobal>() {
            let state = state.0.clone();
            state.update(cx, |state, cx| state.open_panel(View::Details, cx));
        }
    });
    application.run(move |cx| {
        gpui::init(cx);
        appearance::init(cx);
        cx.set_app_identity("dev.metra.desktop", "Metra");
        cx.set_quit_mode(QuitMode::Explicit);
        #[cfg(target_os = "macos")]
        cx.set_activation_policy(ActivationPolicy::Accessory);
        let (sender, receiver) = mpsc::channel();
        let desktop = cx.new(|cx| {
            let payload = service.payload();
            let tray = StatusTray::new(&payload)
                .map_err(|error| diagnostics::warn("tray.setup_failed", error))
                .ok();
            let mut desktop = Desktop {
                service,
                updater: Arc::new(Updater::default()),
                sender,
                receiver,
                tray,
                bubble: None,
                panel: None,
                panel_mode: View::Details,
                cycle: 0,
                last_refresh: Instant::now(),
                last_cycle: Instant::now(),
                last_second: Instant::now(),
                started: Instant::now(),
                update_checked: false,
                message: None,
                login_running: Arc::new(AtomicBool::new(false)),
                activation_file: config.join("activate"),
                _instance_lock: lock,
            };
            if desktop.service.payload().settings.display_mode == DisplayMode::Bubble
                || desktop.tray.is_none()
            {
                desktop.open_bubble(cx);
            }
            if std::env::args().any(|arg| arg == "--settings") {
                desktop.open_panel(View::Settings, cx);
            }
            desktop.refresh(false);
            cx.spawn(async move |weak, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    if weak.update(cx, |desktop, cx| desktop.tick(cx)).is_err() {
                        break;
                    }
                }
            })
            .detach();
            desktop
        });
        cx.set_global(DesktopGlobal(desktop));
    });
}

impl Desktop {
    fn refresh(&mut self, recheck_login: bool) {
        if self.service.refreshing.load(Ordering::Acquire) {
            return;
        }
        self.last_refresh = Instant::now();
        let service = self.service.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            crate::providers::discovery::invalidate_shell_cache();
            if recheck_login {
                service.refresh_cursor_login_status();
            } else {
                // CursorProvider enforces compatibility consent before reading credentials.
                service.refresh(true);
            }
            let _ = sender.send(Event::Refreshed);
        });
    }
    fn tick(&mut self, cx: &mut Context<Self>) {
        #[cfg(target_os = "linux")]
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
        let mut changed = false;
        while let Ok(event) = self.receiver.try_recv() {
            changed = true;
            match event {
                Event::Refreshed | Event::UpdateReady => {}
                Event::Message(message) => self.message = Some((message, Instant::now())),
                Event::LoginFinished(success) => {
                    self.login_running.store(false, Ordering::Release);
                    if success {
                        self.refresh(true);
                    } else {
                        self.message = Some(("Cursor 登录未完成，请重试".into(), Instant::now()));
                    }
                }
                #[cfg(windows)]
                Event::InstallerStarted => {
                    cx.quit();
                    return;
                }
            }
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id.as_ref() {
                "details" => self.open_panel(View::Details, cx),
                "settings" => self.open_panel(View::Settings, cx),
                "refresh" => self.refresh(false),
                "display" => {
                    let mode =
                        if self.service.payload().settings.display_mode == DisplayMode::Bubble {
                            DisplayMode::StatusBar
                        } else {
                            DisplayMode::Bubble
                        };
                    self.change(Change::Display(mode), cx);
                }
                "quit" => cx.quit(),
                _ => {}
            }
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: TrayButton::Left,
                    button_state: TrayButtonState::Up,
                    ..
                }
            ) {
                self.toggle_details(cx);
            }
        }
        if self.activation_file.is_file() {
            let _ = std::fs::remove_file(&self.activation_file);
            self.open_panel(View::Details, cx);
        }
        let settings = self.service.payload().settings;
        if self.last_refresh.elapsed() >= Duration::from_secs(settings.refresh_minutes * 60) {
            self.refresh(false);
            changed = true;
        }
        if self.last_cycle.elapsed() >= Duration::from_secs(5) && self.panel.is_none() {
            self.cycle = self.cycle.wrapping_add(1);
            self.last_cycle = Instant::now();
            changed = true;
        }
        if !cfg!(debug_assertions)
            && ((!self.update_checked && self.started.elapsed() >= Duration::from_secs(20))
                || (self.update_checked
                    && self.started.elapsed() >= Duration::from_secs(24 * 60 * 60)))
        {
            self.check_update();
            self.started = Instant::now();
            self.update_checked = true;
        }
        if self
            .message
            .as_ref()
            .is_some_and(|(_, since)| since.elapsed() > Duration::from_secs(6))
        {
            self.message = None;
            changed = true;
        }
        if self.last_second.elapsed() >= Duration::from_secs(1) {
            self.last_second = Instant::now();
            changed = true;
        }
        if changed {
            if let Some(tray) = &mut self.tray {
                tray.update(&self.service.payload(), self.cycle);
            }
            cx.notify();
        }
    }
    fn open_bubble(&mut self, cx: &mut Context<Self>) {
        if self.bubble.is_some() {
            return;
        }
        let mut settings = self.service.payload().settings;
        if settings.bubble_position_version < 2
            && let Some(position) = &mut settings.bubble_position
        {
            let logical = platform::logical_screen_point(position.x as f32, position.y as f32);
            position.x = f32::from(logical.x).round() as i32;
            position.y = f32::from(logical.y).round() as i32;
        }
        let display = cx
            .displays()
            .into_iter()
            .find(|display| {
                settings.bubble_position.is_some_and(|position| {
                    display
                        .bounds()
                        .contains(&platform::position(position.x, position.y))
                })
            })
            .or_else(|| cx.primary_display());
        let work = display
            .as_ref()
            .map(|display| display.visible_bounds())
            .unwrap_or(Bounds::new(
                point(px(0.), px(0.)),
                size(px(1280.), px(800.)),
            ));
        let origin = settings
            .bubble_position
            .map(|position| platform::position(position.x, position.y))
            .unwrap_or(point(
                work.right() - px(BUBBLE_SIZE + 8.),
                work.bottom() - px(BUBBLE_SIZE + 8.),
            ));
        let origin = clamp_origin(origin, size(px(BUBBLE_SIZE), px(BUBBLE_SIZE)), work);
        let state = cx.entity();
        let service = self.service.clone();
        let updater = self.updater.clone();
        match gpui::open_window(
            overlay_options(
                Bounds::new(origin, size(px(BUBBLE_SIZE), px(BUBBLE_SIZE))),
                false,
                display.map(|display| display.id()),
            ),
            cx,
            |window, cx| {
                cx.new(|cx| AppView::new(state, View::Bubble, service, updater, window, cx))
            },
        ) {
            Ok((handle, _)) => self.bubble = Some(handle),
            Err(error) => self.message = Some((error.to_string(), Instant::now())),
        }
    }
    fn toggle_details(&mut self, cx: &mut Context<Self>) {
        if self.panel_mode == View::Details
            && let Some(panel) = self.panel.take()
        {
            let _ = panel.update(cx, |_, window, _| window.remove_window());
            cx.notify();
        } else {
            self.open_panel(View::Details, cx);
        }
    }
    fn open_panel(&mut self, mode: View, cx: &mut Context<Self>) {
        if let Some(panel) = self.panel.take() {
            let _ = panel.update(cx, |_, window, _| window.remove_window());
        }
        self.panel_mode = mode;
        let state = cx.entity();
        let service = self.service.clone();
        let updater = self.updater.clone();
        let payload = self.service.payload();
        let display = cx.primary_display();
        let mut work = display
            .as_ref()
            .map(|display| display.visible_bounds())
            .unwrap_or(Bounds::new(
                point(px(0.), px(0.)),
                size(px(1280.), px(800.)),
            ));
        let mut origin = point(
            work.right() - px(PANEL_WIDTH + 12.),
            work.origin.y + px(12.),
        );
        if payload.settings.display_mode == DisplayMode::Bubble {
            if let Some(bubble) = self.bubble {
                let _ = bubble.update(cx, |_, window, cx| {
                    let bounds = window.bounds();
                    if let Some(display) = window.display(cx) {
                        work = display.visible_bounds();
                    }
                    origin = point(bounds.origin.x - px(PANEL_WIDTH + 8.), bounds.origin.y);
                });
            }
        } else if let Some((x, y)) = self.tray.as_ref().and_then(StatusTray::position) {
            let logical = platform::logical_screen_point(x, y);
            let (x, y) = (f32::from(logical.x), f32::from(logical.y));
            if let Some(display) = cx
                .displays()
                .into_iter()
                .find(|display| display.bounds().contains(&point(px(x), px(y))))
            {
                work = display.visible_bounds();
            }
            origin = point(px(x) - px(PANEL_WIDTH), px(y) + px(24.));
        }
        let width = if mode == View::Settings {
            MENU_WIDTH
        } else {
            PANEL_WIDTH
        };
        let height: f32 = if mode == View::Consent {
            260.
        } else if mode == View::Settings {
            528.
        } else {
            500.
        };
        let height = height.min(f32::from(work.size.height) - 24.);
        let origin = clamp_origin(origin, size(px(width), px(height)), work);
        match gpui::open_window(
            overlay_options(Bounds::new(origin, size(px(width), px(height))), true, None),
            cx,
            |window, cx| cx.new(|cx| AppView::new(state, mode, service, updater, window, cx)),
        ) {
            Ok((handle, _)) => {
                self.panel = Some(handle);
                cx.activate(true);
            }
            Err(error) => self.message = Some((error.to_string(), Instant::now())),
        }
        cx.notify();
    }
    fn change(&mut self, change: Change, cx: &mut Context<Self>) {
        let previous = self.service.payload().settings;
        if let Change::Visible(provider, false) = change
            && previous.bubble_visible_providers.len() <= 1
            && previous.bubble_visible_providers.contains(&provider)
        {
            self.message = Some((
                I18n::new(previous.ui_language).t("config.keepOne"),
                Instant::now(),
            ));
            cx.notify();
            return;
        }
        if matches!(change, Change::Display(DisplayMode::StatusBar)) && self.tray.is_none() {
            self.message = Some(("状态栏不可用，请使用悬浮球".into(), Instant::now()));
            cx.notify();
            return;
        }
        if let Change::Autostart(enabled) = change
            && let Err(error) = platform::set_autostart(enabled)
        {
            self.message = Some((error, Instant::now()));
            cx.notify();
            return;
        }
        let result = self.service.update_settings(|settings| match &change {
            Change::Display(mode) => settings.display_mode = *mode,
            Change::Percent(mode) => settings.bubble_percent_mode = *mode,
            Change::Refresh(minutes) => settings.refresh_minutes = *minutes,
            Change::Language(language) => settings.ui_language = *language,
            Change::Snap(enabled) => settings.bubble_snap_enabled = *enabled,
            Change::Autostart(enabled) => settings.autostart = *enabled,
            Change::Compat(enabled) => settings.cursor_compat_enabled = *enabled,
            Change::Visible(provider, visible) => {
                if *visible {
                    settings.bubble_visible_providers.push(*provider);
                } else if settings.bubble_visible_providers.len() > 1 {
                    settings
                        .bubble_visible_providers
                        .retain(|candidate| candidate != provider);
                }
            }
            Change::Label(provider, label) => match provider {
                Provider::Cursor => settings.cursor_bubble_label = label.clone(),
                Provider::Codex => settings.codex_bubble_label = label.clone(),
                Provider::Claude => settings.claude_bubble_label = label.clone(),
            },
            Change::Color(provider, color) => match provider {
                Provider::Cursor => settings.cursor_bubble_color = color.clone(),
                Provider::Codex => settings.codex_bubble_color = color.clone(),
                Provider::Claude => settings.claude_bubble_color = color.clone(),
            },
            Change::MoveBefore(provider, target) => {
                if provider != target {
                    settings
                        .bubble_provider_order
                        .retain(|candidate| candidate != provider);
                    let index = settings
                        .bubble_provider_order
                        .iter()
                        .position(|candidate| candidate == target)
                        .unwrap_or(0);
                    settings.bubble_provider_order.insert(index, *provider);
                }
            }
            Change::Reorder(provider, direction) => {
                if let Some(index) = settings
                    .bubble_provider_order
                    .iter()
                    .position(|candidate| candidate == provider)
                {
                    let target = (index as i32 + direction)
                        .clamp(0, settings.bubble_provider_order.len() as i32 - 1)
                        as usize;
                    settings.bubble_provider_order.swap(index, target);
                }
            }
        });
        match result {
            Ok(settings) => {
                if settings.display_mode != previous.display_mode {
                    if settings.display_mode == DisplayMode::StatusBar {
                        if let Some(bubble) = self.bubble.take() {
                            let _ = bubble.update(cx, |_, window, _| window.remove_window());
                        }
                    } else {
                        self.open_bubble(cx);
                    }
                }
                if settings.cursor_compat_enabled != previous.cursor_compat_enabled {
                    self.refresh(false);
                }
                if let Some(tray) = &mut self.tray {
                    let _ = tray.rebuild_menu(&self.service.payload());
                    tray.update(&self.service.payload(), self.cycle);
                }
                self.message = Some((
                    I18n::new(settings.ui_language).t("native.updated"),
                    Instant::now(),
                ));
            }
            Err(error) => {
                if let Change::Autostart(_) = change {
                    let _ = platform::set_autostart(previous.autostart);
                }
                self.message = Some((error, Instant::now()));
            }
        }
        cx.notify();
    }
    fn login(&mut self) {
        if self.login_running.swap(true, Ordering::AcqRel) {
            return;
        }
        let service = self.service.clone();
        let sender = self.sender.clone();
        let running = self.login_running.clone();
        std::thread::spawn(move || {
            let executable = crate::providers::cursor::cursor_login_executable();
            if !service.cursor_compat.load(Ordering::Acquire)
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
                match command.spawn() {
                    Ok(mut child) => {
                        let deadline = Instant::now() + Duration::from_secs(300);
                        loop {
                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    let _ = sender.send(Event::LoginFinished(status.success()));
                                    break;
                                }
                                Ok(None) if Instant::now() < deadline => {
                                    std::thread::sleep(Duration::from_millis(100))
                                }
                                _ => {
                                    let _ = child.kill();
                                    let _ = child.wait();
                                    let _ = sender.send(Event::LoginFinished(false));
                                    break;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        running.store(false, Ordering::Release);
                        let _ = sender.send(Event::Message(error.to_string()));
                    }
                }
            } else {
                let mut command = std::process::Command::new(if cfg!(target_os = "macos") {
                    "open"
                } else if cfg!(windows) {
                    "explorer.exe"
                } else {
                    "xdg-open"
                });
                command.arg("cursor://anysphere.cursor-deeplink/settings/plan-usage");
                crate::providers::scrub_sensitive_child_environment(&mut command);
                let result = command.spawn();
                running.store(false, Ordering::Release);
                let _ = sender.send(Event::Message(
                    result
                        .map(|_| "已打开 Cursor 登录页面".into())
                        .unwrap_or_else(|error| error.to_string()),
                ));
            }
        });
    }
    fn check_update(&self) {
        let updater = self.updater.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            updater.check();
            let _ = sender.send(Event::UpdateReady);
        });
    }
    #[cfg(windows)]
    fn install_update(&self) {
        let updater = self.updater.clone();
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let event = match updater.install_windows() {
                Ok(()) => Event::InstallerStarted,
                Err(error) => Event::Message(error),
            };
            let _ = sender.send(event);
        });
    }
}

fn overlay_options(
    bounds: Bounds<Pixels>,
    focus: bool,
    display_id: Option<DisplayId>,
) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: true,
        app_owns_titlebar_drag: true,
        is_resizable: false,
        is_minimizable: false,
        display_id,
        window_background: WindowBackgroundAppearance::Transparent,
        app_id: Some("dev.metra.desktop".into()),
        ..Default::default()
    }
}
fn clamp_origin(origin: Point<Pixels>, size: Size<Pixels>, work: Bounds<Pixels>) -> Point<Pixels> {
    point(
        px(f32::from(origin.x).clamp(
            f32::from(work.origin.x),
            (f32::from(work.right() - size.width)).max(f32::from(work.origin.x)),
        )),
        px(f32::from(origin.y).clamp(
            f32::from(work.origin.y),
            (f32::from(work.bottom() - size.height)).max(f32::from(work.origin.y)),
        )),
    )
}
fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Cursor => "Cursor",
        Provider::Codex => "Codex",
        Provider::Claude => "Claude Code",
    }
}
fn provider_index(provider: Provider) -> usize {
    match provider {
        Provider::Cursor => 0,
        Provider::Codex => 1,
        Provider::Claude => 2,
    }
}
fn provider_data(payload: &AppPayload, provider: Provider) -> &ProviderSnapshot {
    match provider {
        Provider::Cursor => &payload.snapshot.cursor,
        Provider::Codex => &payload.snapshot.codex,
        Provider::Claude => &payload.snapshot.claude,
    }
}
fn color(value: &str) -> Hsla {
    rgb(u32::from_str_radix(value.trim_start_matches('#'), 16).unwrap_or(0x9c83ff)).into()
}
fn compact(value: Option<u64>) -> String {
    value.map_or_else(
        || "—".into(),
        |value| {
            if value >= 1_000_000 {
                format!("{:.1}M", value as f64 / 1_000_000.)
            } else if value >= 1_000 {
                format!("{:.1}K", value as f64 / 1_000.)
            } else {
                value.to_string()
            }
        },
    )
}
fn money(value: Option<u64>) -> String {
    value.map_or_else(
        || "—".into(),
        |value| format!("${:.2}", value as f64 / 100.),
    )
}

struct ProviderDrag(Provider);
impl Render for ProviderDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(6.))
            .rounded(px(8.))
            .bg(rgb(0x28243d))
            .text_color(rgb(0xd5ceff))
            .text_size(px(11.))
            .child(provider_name(self.0))
    }
}

struct AppView {
    desktop: Entity<Desktop>,
    service: Arc<RefreshService>,
    updater: Arc<Updater>,
    message: Option<String>,
    mode: View,
    labels: Vec<Entity<InputState>>,
    language: Entity<SelectState<Vec<SharedString>>>,
    language_locale: &'static str,
    subscriptions: Vec<Subscription>,
    color_provider: Option<Provider>,
    drag_origin: Option<Point<Pixels>>,
    moved: bool,
    dragging: bool,
    peek: bool,
    dock_right: bool,
    position_revision: u64,
    idle_revision: u64,
    hovered: bool,
}
impl AppView {
    fn new(
        desktop: Entity<Desktop>,
        mode: View,
        service: Arc<RefreshService>,
        updater: Arc<Updater>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.set_rem_size(px(13.));
        let payload = service.payload();
        let language_options = vec![
            I18n::new(payload.settings.ui_language).t("menu.languageSystem"),
            "简体中文".into(),
            "English".into(),
            "日本語".into(),
            "한국어".into(),
        ]
        .into_iter()
        .map(SharedString::from)
        .collect::<Vec<_>>();
        let language_index = [
            UiLanguage::System,
            UiLanguage::ZhCn,
            UiLanguage::En,
            UiLanguage::Ja,
            UiLanguage::Ko,
        ]
        .iter()
        .position(|value| *value == payload.settings.ui_language)
        .unwrap_or(0);
        let language = cx.new(|cx| {
            SelectState::new(
                language_options,
                Some(IndexPath::new(language_index)),
                window,
                cx,
            )
        });
        let subscriptions = vec![
            cx.observe_in(&desktop, window, |view, desktop, window, cx| {
                view.message = desktop
                    .read(cx)
                    .message
                    .as_ref()
                    .map(|(message, _)| message.clone());
                let settings = desktop.read(cx).service.payload().settings;
                let locale = I18n::new(settings.ui_language);
                if view.language_locale != locale.locale {
                    let labels = vec![
                        locale.t("menu.languageSystem"),
                        "简体中文".into(),
                        "English".into(),
                        "日本語".into(),
                        "한국어".into(),
                    ]
                    .into_iter()
                    .map(SharedString::from)
                    .collect::<Vec<_>>();
                    let index = [
                        UiLanguage::System,
                        UiLanguage::ZhCn,
                        UiLanguage::En,
                        UiLanguage::Ja,
                        UiLanguage::Ko,
                    ]
                    .iter()
                    .position(|candidate| *candidate == settings.ui_language)
                    .unwrap_or(0);
                    view.language.update(cx, |state, cx| {
                        state.set_items(labels, window, cx);
                        state.set_selected_index(Some(IndexPath::new(index)), window, cx);
                    });
                    view.language_locale = locale.locale;
                }
                cx.notify();
            }),
        ];
        let labels = if matches!(mode, View::Settings | View::Details) {
            [
                &payload.settings.cursor_bubble_label,
                &payload.settings.codex_bubble_label,
                &payload.settings.claude_bubble_label,
            ]
            .into_iter()
            .map(|label| cx.new(|cx| InputState::new(window, cx).default_value(label.clone())))
            .collect()
        } else {
            vec![]
        };
        let mut view = Self {
            desktop,
            service,
            updater,
            message: None,
            mode,
            labels,
            language,
            language_locale: I18n::new(payload.settings.ui_language).locale,
            subscriptions,
            color_provider: None,
            drag_origin: None,
            moved: false,
            dragging: false,
            peek: false,
            dock_right: false,
            position_revision: 0,
            idle_revision: 0,
            hovered: false,
        };
        let language = view.language.clone();
        view.subscriptions
            .push(cx.subscribe_in(&language, window, |view, _, event, _, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                let language = match value.as_ref() {
                    "简体中文" => UiLanguage::ZhCn,
                    "English" => UiLanguage::En,
                    "日本語" => UiLanguage::Ja,
                    "한국어" => UiLanguage::Ko,
                    _ => UiLanguage::System,
                };
                view.dispatch(Change::Language(language), cx);
            }));
        if matches!(mode, View::Settings | View::Details) {
            for provider in [Provider::Cursor, Provider::Codex, Provider::Claude] {
                let input = view.labels[provider_index(provider)].clone();
                view.subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    move |view, input, event, window, cx| {
                        if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                            let value = input.read(cx).value().to_string();
                            let trimmed = value.trim().chars().take(3).collect::<String>();
                            let normalized = if trimmed.is_empty() {
                                match provider {
                                    Provider::Cursor => "C",
                                    Provider::Codex => "X",
                                    Provider::Claude => "A",
                                }
                                .to_string()
                            } else {
                                trimmed
                            };
                            if normalized != value {
                                input.update(cx, |input, cx| {
                                    input.set_value(normalized.clone(), window, cx)
                                });
                            }
                            view.dispatch(Change::Label(provider, normalized), cx);
                        }
                    },
                ));
            }
        }
        if mode != View::Bubble {
            let handle = window.window_handle();
            view.subscriptions.push(cx.observe_window_activation(
                window,
                move |view, window, cx| {
                    if !window.is_window_active() {
                        let desktop = view.desktop.clone();
                        cx.defer(move |cx| {
                            desktop.update(cx, |desktop, cx| {
                                if desktop.panel == Some(handle) {
                                    desktop.panel = None;
                                    let _ =
                                        handle.update(cx, |_, window, _| window.remove_window());
                                    cx.notify();
                                }
                            })
                        });
                    }
                },
            ));
        }
        if mode == View::Bubble {
            view.subscriptions
                .push(cx.observe_window_bounds(window, |view, window, cx| {
                    let mut position = platform::save_position(window);
                    if view.peek && view.dock_right {
                        position.x -= px(BUBBLE_SIZE - PEEK_SIZE);
                    }
                    view.position_revision = view.position_revision.wrapping_add(1);
                    let revision = view.position_revision;
                    cx.spawn_in(window, async move |weak, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(250))
                            .await;
                        let _ = weak.update_in(cx, |view, _, _| {
                            if view.position_revision == revision {
                                let _ = view.service.update_settings(|settings| {
                                    settings.bubble_position = Some(SavedPosition {
                                        x: f32::from(position.x).round() as i32,
                                        y: f32::from(position.y).round() as i32,
                                    });
                                    settings.bubble_position_version = 2;
                                });
                            }
                        });
                    })
                    .detach();
                }));
        }
        view
    }
    fn dispatch(&self, change: Change, cx: &mut Context<Self>) {
        let desktop = self.desktop.clone();
        cx.defer(move |cx| desktop.update(cx, |desktop, cx| desktop.change(change, cx)));
    }
    fn open(&self, mode: View, cx: &mut Context<Self>) {
        let desktop = self.desktop.clone();
        let toggle = self.mode == View::Bubble && mode == View::Details;
        cx.defer(move |cx| {
            desktop.update(cx, |desktop, cx| {
                if toggle {
                    desktop.toggle_details(cx)
                } else {
                    desktop.open_panel(mode, cx)
                }
            })
        });
    }
    fn refresh(&self, recheck: bool, cx: &mut Context<Self>) {
        let desktop = self.desktop.clone();
        cx.defer(move |cx| {
            desktop.update(cx, |desktop, cx| {
                desktop.refresh(recheck);
                cx.notify();
            })
        });
    }
    fn reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.peek {
            let origin = window.bounds().origin;
            window.resize(size(px(BUBBLE_SIZE), px(BUBBLE_SIZE)));
            if self.dock_right {
                let _ = platform::move_window(
                    window,
                    point(origin.x - px(BUBBLE_SIZE - PEEK_SIZE), origin.y),
                );
            }
            self.peek = false;
            cx.notify();
        }
    }
    fn fold(&mut self, window: &mut Window, cx: &mut Context<Self>, snap_to_edge: bool) {
        if self.peek || self.dragging {
            return;
        }
        let Some(display) = window.display(cx) else {
            return;
        };
        let work = display.visible_bounds();
        let bounds = window.bounds();
        let near_edge = f32::from(bounds.origin.x - work.origin.x).abs() < 12.
            || f32::from(bounds.right() - work.right()).abs() < 12.;
        if !snap_to_edge && !near_edge {
            return;
        }
        self.dock_right = bounds.center().x >= work.center().x;
        let x = if self.dock_right {
            work.right() - px(PEEK_SIZE)
        } else {
            work.origin.x
        };
        self.peek = true;
        window.resize(size(px(PEEK_SIZE), px(BUBBLE_SIZE)));
        if let Err(error) = platform::move_window(
            window,
            point(x, clamp_origin(bounds.origin, bounds.size, work).y),
        ) {
            self.peek = false;
            window.resize(size(px(BUBBLE_SIZE), px(BUBBLE_SIZE)));
            diagnostics::warn("bubble.snap_failed", error);
        }
        cx.notify();
    }
    fn finish_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dragging {
            return;
        }
        self.dragging = false;
        let snap = self.service.payload().settings.bubble_snap_enabled;
        if self.moved && snap {
            self.fold(window, cx, true);
        }
        if !self.moved {
            self.open(View::Details, cx);
        }
        self.drag_origin = None;
        cx.notify();
    }
    fn bubble(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        payload: &AppPayload,
    ) -> AnyElement {
        let entries = status_bar::entries(&payload.snapshot, &payload.settings);
        let t = I18n::new(payload.settings.ui_language);
        let usage = entries
            .iter()
            .map(status_bar::StatusEntry::headline)
            .collect::<Vec<_>>()
            .join(&t.t("bubble.usageSeparator"));
        let accessible_label = t.format(
            "bubble.ariaLabel",
            &[
                ("usage", usage),
                (
                    "dragHint",
                    t.t(if payload.settings.bubble_snap_enabled {
                        "bubble.snapHint"
                    } else {
                        "bubble.freeHint"
                    }),
                ),
            ],
        );
        let accessible_desktop = self.desktop.clone();
        let mut body = div()
            .id("metra-bubble")
            .role(Role::Button)
            .aria_label(accessible_label)
            .focusable()
            .tab_stop(true)
            .on_a11y_action(AccessibleAction::Click, move |_, _, cx| {
                let desktop = accessible_desktop.clone();
                cx.defer(move |cx| desktop.update(cx, |desktop, cx| desktop.toggle_details(cx)));
            })
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space" | " ") {
                    view.reveal(window, cx);
                    view.open(View::Details, cx);
                    cx.stop_propagation();
                }
            }))
            .size_full()
            .rounded_full()
            .bg(linear_gradient(
                138.,
                linear_color_stop(rgb(0x8d72ff), 0.),
                linear_color_stop(rgb(0x28d1b3), 1.),
            ))
            .p(px(3.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(1.))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, event: &MouseDownEvent, window, cx| {
                    view.reveal(window, cx);
                    view.drag_origin = Some(event.position);
                    view.moved = false;
                    view.dragging = true;
                }),
            )
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, window, cx| {
                if view.dragging
                    && !view.moved
                    && let Some(origin) = view.drag_origin
                {
                    let distance = (f32::from(event.position.x - origin.x)).abs()
                        + (f32::from(event.position.y - origin.y)).abs();
                    if distance > 4. {
                        view.moved = true;
                        window.start_window_move();
                        cx.spawn_in(window, async move |weak, cx| {
                            loop {
                                cx.background_executor()
                                    .timer(Duration::from_millis(32))
                                    .await;
                                let keep = weak
                                    .update_in(cx, |view, window, cx| {
                                        if !view.dragging {
                                            return false;
                                        }
                                        if platform::primary_button_pressed() == Ok(false) {
                                            view.finish_drag(window, cx);
                                            return false;
                                        }
                                        true
                                    })
                                    .unwrap_or(false);
                                if !keep {
                                    break;
                                }
                            }
                        })
                        .detach();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, window, cx| view.finish_drag(window, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|view, _, _, cx| view.open(View::Settings, cx)),
            )
            .on_hover(cx.listener(|view, hovered: &bool, window, cx| {
                view.hovered = *hovered;
                view.idle_revision = view.idle_revision.wrapping_add(1);
                let revision = view.idle_revision;
                if *hovered {
                    view.reveal(window, cx);
                } else if view.service.payload().settings.bubble_snap_enabled {
                    cx.spawn_in(window, async move |weak, cx| {
                        cx.background_executor().timer(Duration::from_secs(3)).await;
                        let _ = weak.update_in(cx, |view, window, cx| {
                            if view.idle_revision == revision
                                && !view.hovered
                                && view.service.payload().settings.bubble_snap_enabled
                            {
                                view.fold(window, cx, false);
                            }
                        });
                    })
                    .detach();
                }
            }));
        let mut center = div()
            .size_full()
            .rounded_full()
            .bg(rgb(0x171b2b))
            .flex()
            .flex_col()
            .justify_center()
            .px(px(5.))
            .py(px(3.));
        let count = entries.len();
        for entry in entries {
            let accent = color(&entry.color);
            let severity = entry
                .value
                .map(|value| match payload.settings.bubble_percent_mode {
                    BubblePercentMode::Used => 100 - value,
                    BubblePercentMode::Remaining => value,
                });
            let value_color = match severity {
                Some(0..=10) => rgb(0xff667d).into(),
                Some(11..=25) => rgb(0xffd15a).into(),
                _ => accent,
            };
            let mut row = div()
                .flex()
                .items_center()
                .justify_between()
                .h(px(if count == 3 { 12. } else { 15. }))
                .gap(px(2.));
            if !self.peek {
                row = row.child(
                    div()
                        .w(px(12.))
                        .h(px(10.))
                        .rounded(px(3.))
                        .bg(accent.opacity(0.18))
                        .text_color(accent)
                        .text_size(px(6.5))
                        .font_weight(FontWeight::BOLD)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(entry.label),
                );
            }
            row = row.child(
                div()
                    .text_size(px(if self.peek { 8. } else { 9. }))
                    .font_weight(FontWeight::BOLD)
                    .text_color(value_color)
                    .child(
                        entry
                            .value
                            .map_or_else(|| "—".into(), |value| format!("{value}%")),
                    ),
            );
            if entry.stale {
                row = row.opacity(0.55);
            }
            center = center.child(row);
        }
        let _ = window;
        body = body.child(center);
        body.into_any_element()
    }
    fn settings(&mut self, cx: &mut Context<Self>, payload: &AppPayload) -> AnyElement {
        let s = &payload.settings;
        let t = I18n::new(s.ui_language);
        let language = div()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.))
            .mb(px(7.))
            .child(
                div()
                    .text_size(px(10.))
                    .text_color(rgb(0xaeb7cf))
                    .child(t.t("menu.language")),
            )
            .child(
                Select::new(&self.language)
                    .small()
                    .h(px(32.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgb(0x353b4e))
                    .bg(rgb(0x171b29))
                    .w(px(116.))
                    .menu_width(px(116.)),
            );
        let mut intervals = div().flex().gap(px(5.));
        for minutes in REFRESH_INTERVALS {
            intervals = intervals.child(
                appearance::segment(
                    ("interval", minutes as usize),
                    if minutes == 60 {
                        "1h".into()
                    } else {
                        format!("{minutes}m")
                    },
                    s.refresh_minutes == minutes,
                )
                .on_click(
                    cx.listener(move |view, _, _, cx| view.dispatch(Change::Refresh(minutes), cx)),
                ),
            );
        }
        let display = div()
            .flex()
            .gap(px(5.))
            .child(
                appearance::segment(
                    "display-bubble",
                    t.t("menu.displayBubble"),
                    s.display_mode == DisplayMode::Bubble,
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.dispatch(Change::Display(DisplayMode::Bubble), cx)
                })),
            )
            .child(
                appearance::segment(
                    "display-status",
                    t.t("menu.displayStatusBar"),
                    s.display_mode == DisplayMode::StatusBar,
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.dispatch(Change::Display(DisplayMode::StatusBar), cx)
                })),
            );
        let percentage = div()
            .flex()
            .gap(px(5.))
            .child(
                appearance::segment(
                    "percent-used",
                    t.t("menu.used"),
                    s.bubble_percent_mode == BubblePercentMode::Used,
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.dispatch(Change::Percent(BubblePercentMode::Used), cx)
                })),
            )
            .child(
                appearance::segment(
                    "percent-remaining",
                    t.t("menu.remaining"),
                    s.bubble_percent_mode == BubblePercentMode::Remaining,
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.dispatch(Change::Percent(BubblePercentMode::Remaining), cx)
                })),
            );
        div()
            .flex()
            .flex_col()
            .gap(px(13.))
            .child(language)
            .child(
                div()
                    .child(appearance::section_title(t.t("menu.refreshInterval")))
                    .child(intervals),
            )
            .child(
                div()
                    .child(appearance::section_title(t.t("menu.displayMode")))
                    .child(display),
            )
            .child(
                div()
                    .child(appearance::section_title(t.t("menu.bubblePercentage")))
                    .child(percentage),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(appearance::setting_row(
                        t.t("menu.autoSnap"),
                        Some(t.t("menu.autoSnapHint")),
                        Switch::new("snap")
                            .small()
                            .accessibility_label(t.t("menu.autoSnap"))
                            .checked(s.bubble_snap_enabled)
                            .on_change(cx.listener(|view, enabled: &bool, _, cx| {
                                view.dispatch(Change::Snap(*enabled), cx)
                            })),
                    ))
                    .child(appearance::setting_row(
                        t.t("menu.autostart"),
                        None,
                        Switch::new("autostart")
                            .small()
                            .accessibility_label(t.t("menu.autostart"))
                            .checked(s.autostart)
                            .on_change(cx.listener(|view, enabled: &bool, _, cx| {
                                view.dispatch(Change::Autostart(*enabled), cx)
                            })),
                    ))
                    .child(appearance::setting_row(
                        t.t("menu.cursorCompat"),
                        Some(t.t("menu.cursorCompatHint")),
                        Switch::new("compat")
                            .small()
                            .accessibility_label(t.t("menu.cursorCompat"))
                            .checked(s.cursor_compat_enabled)
                            .on_change(cx.listener(|view, enabled: &bool, _, cx| {
                                if *enabled {
                                    view.open(View::Consent, cx)
                                } else {
                                    view.dispatch(Change::Compat(false), cx)
                                }
                            })),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .border_t_1()
                    .border_color(rgba(0xffffff0b))
                    .pt(px(7.))
                    .child(
                        Button::new("rescan")
                            .ghost()
                            .small()
                            .w_full()
                            .h(px(34.))
                            .label(t.t("menu.rescanCli"))
                            .on_click(cx.listener(|view, _, _, cx| view.refresh(false, cx))),
                    )
                    .child(
                        Button::new("quit")
                            .ghost()
                            .small()
                            .w_full()
                            .h(px(30.))
                            .text_color(rgb(0xd17a8d))
                            .label(t.t("menu.quit"))
                            .on_click(|_, _, cx| cx.quit()),
                    ),
            )
            .into_any_element()
    }
    fn provider_settings(&mut self, cx: &mut Context<Self>, payload: &AppPayload) -> AnyElement {
        let s = &payload.settings;
        let t = I18n::new(s.ui_language);
        let mut list = div().flex().flex_col().gap(px(6.)).pt(px(5.)).child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .mb(px(3.))
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(t.t("config.title")),
                )
                .child(
                    div()
                        .text_size(px(8.))
                        .text_color(rgb(0x69748f))
                        .child(t.t("config.subtitle")),
                ),
        );
        for (index, provider) in s.bubble_provider_order.iter().copied().enumerate() {
            let entry = status_bar::entries(
                &payload.snapshot,
                &AppSettings {
                    bubble_visible_providers: vec![provider],
                    ..s.clone()
                },
            )
            .remove(0);
            let row = div()
                .id(("provider-settings", index))
                .on_drag(ProviderDrag(provider), |drag, _, _, cx| {
                    cx.new(|_| ProviderDrag(drag.0))
                })
                .on_drop(cx.listener(move |view, drag: &ProviderDrag, _, cx| {
                    view.dispatch(Change::MoveBefore(drag.0, provider), cx)
                }))
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(38.))
                .px(px(7.))
                .rounded(px(10.))
                .bg(rgba(0xffffff04))
                .border_1()
                .border_color(rgba(0xffffff08))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            appearance::icon_button(
                                ("up", index),
                                IconName::ChevronUp,
                                t.t("native.moveUp"),
                            )
                            .size(px(14.))
                            .disabled(index == 0)
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    view.dispatch(Change::Reorder(provider, -1), cx)
                                },
                            )),
                        )
                        .child(
                            appearance::icon_button(
                                ("down", index),
                                IconName::ChevronDown,
                                t.t("native.moveDown"),
                            )
                            .size(px(14.))
                            .disabled(index == 2)
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    view.dispatch(Change::Reorder(provider, 1), cx)
                                },
                            )),
                        ),
                )
                .child(
                    Button::new(("color", index))
                        .ghost()
                        .small()
                        .size(px(20.))
                        .label("●")
                        .text_color(color(&entry.color))
                        .tooltip(t.format(
                            "config.chooseColor",
                            &[("provider", provider_name(provider).into())],
                        ))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.color_provider = if view.color_provider == Some(provider) {
                                None
                            } else {
                                Some(provider)
                            };
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(10.))
                        .child(provider_name(provider)),
                )
                .child(
                    Input::new(&self.labels[provider_index(provider)])
                        .small()
                        .w(px(35.))
                        .h(px(25.)),
                )
                .child(
                    Switch::new(("visible", index))
                        .small()
                        .accessibility_label(t.format(
                            "config.providerVisible",
                            &[("provider", provider_name(provider).into())],
                        ))
                        .checked(s.bubble_visible_providers.contains(&provider))
                        .on_change(cx.listener(move |view, visible: &bool, _, cx| {
                            view.dispatch(Change::Visible(provider, *visible), cx)
                        })),
                );
            list = list.child(row);
            if self.color_provider == Some(provider) {
                let mut palette = div()
                    .flex()
                    .flex_wrap()
                    .gap(px(3.))
                    .p(px(7.))
                    .rounded(px(10.))
                    .bg(rgb(0x1b2030));
                for (index, value) in BUBBLE_COLOR_PALETTE.iter().enumerate() {
                    let value = value.to_string();
                    let change = Change::Color(provider, value.clone());
                    palette = palette.child(
                        Button::new(("swatch", index))
                            .ghost()
                            .small()
                            .size(px(23.))
                            .label("●")
                            .text_color(color(&value))
                            .tooltip(value)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.dispatch(change.clone(), cx);
                                view.color_provider = None;
                                cx.notify();
                            })),
                    );
                }
                list = list.child(palette);
            }
        }
        list.into_any_element()
    }
    fn details(&mut self, cx: &mut Context<Self>, payload: &AppPayload) -> AnyElement {
        let t = I18n::new(payload.settings.ui_language);
        let mut body = div().flex().flex_col().gap(px(11.));
        if payload.snapshot.refreshing {
            body = body.child(
                Progress::new("refreshing")
                    .loading(true)
                    .accessibility_label(t.t("native.refreshing")),
            );
        }
        for provider in [Provider::Cursor, Provider::Codex, Provider::Claude] {
            let data = provider_data(payload, provider);
            let entry = status_bar::entries(
                &payload.snapshot,
                &AppSettings {
                    bubble_visible_providers: vec![provider],
                    ..payload.settings.clone()
                },
            )
            .remove(0);
            let accent = color(&entry.color);
            let key = if data.stale {
                "status.stale"
            } else {
                match data.status {
                    ProviderStatus::Available => "status.available",
                    ProviderStatus::DesktopInstalled => "status.desktopInstalled",
                    ProviderStatus::NotInstalled => "status.notInstalled",
                    ProviderStatus::NotLoggedIn => "status.notLoggedIn",
                    ProviderStatus::Unsupported => "status.unsupported",
                    ProviderStatus::NetworkError => "status.networkError",
                    ProviderStatus::ProtocolError => "status.protocolError",
                }
            };
            let mut card = div()
                .flex()
                .flex_col()
                .gap(px(9.))
                .p(px(12.))
                .rounded(px(14.))
                .bg(linear_gradient(
                    150.,
                    linear_color_stop(rgb(0x202536), 0.),
                    linear_color_stop(rgb(0x171c2a), 1.),
                ))
                .border_1()
                .border_color(rgba(0xffffff0c))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(accent)
                                .child(provider_name(provider)),
                        )
                        .child(div().text_xs().text_color(rgb(0x9aa5bd)).child(t.t(key))),
                )
                .child(div().text_xs().text_color(rgb(0x9aa5bd)).child(format!(
                        "{} · {}",
                        data.plan.as_deref().unwrap_or("—"),
                        data.captured_at
                            .with_timezone(&chrono::Local)
                            .format("%m/%d %H:%M")
                    )));
            for (index, quota) in data.quotas.iter().enumerate() {
                let reset = quota
                    .resets_at
                    .map(|at| {
                        let minutes = (at - chrono::Utc::now()).num_minutes().max(0);
                        t.format(
                            "quota.resetsAt",
                            &[(
                                "date",
                                format!(
                                    "{} · {}h {}m",
                                    at.with_timezone(&chrono::Local).format("%m/%d %H:%M"),
                                    minutes / 60,
                                    minutes % 60
                                ),
                            )],
                        )
                    })
                    .unwrap_or_else(|| "—".into());
                card = card.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(quota.label.clone())
                                .child(t.format(
                                    "quota.remaining",
                                    &[("value", format!("{:.0}", quota.remaining_percent))],
                                )),
                        )
                        .child(
                            Progress::new(("quota", provider_index(provider) * 10 + index))
                                .value(quota.remaining_percent as f32)
                                .color(accent)
                                .small()
                                .accessibility_label(quota.label.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .text_xs()
                                .text_color(rgb(0x9aa5bd))
                                .child(t.format(
                                    "quota.used",
                                    &[("value", format!("{:.0}", quota.used_percent))],
                                ))
                                .child(reset),
                        ),
                );
            }
            if data.quotas.is_empty() {
                card = card.child(
                    div().text_sm().text_color(rgb(0x9aa5bd)).child(
                        data.message
                            .as_deref()
                            .map(|message| t.message(message))
                            .unwrap_or_else(|| t.t("quota.noData")),
                    ),
                );
            }
            if let Some(tokens) = &data.tokens {
                card = card.child(
                    div()
                        .flex()
                        .justify_between()
                        .text_sm()
                        .child(format!(
                            "{} {}",
                            t.t(
                                if provider == Provider::Claude
                                    && data
                                        .cost
                                        .as_ref()
                                        .is_some_and(|cost| cost.today_used_cents.is_some())
                                {
                                    "provider.apiTodayTokensUtc"
                                } else {
                                    "provider.localTodayTokens"
                                }
                            ),
                            compact(tokens.today)
                        ))
                        .child(format!(
                            "{} {}",
                            t.t(if provider == Provider::Codex {
                                "provider.officialLifetimeTokens"
                            } else {
                                "provider.localLifetimeTokens"
                            }),
                            compact(tokens.lifetime)
                        )),
                );
            }
            if let Some(cost) = &data.cost {
                if cost.included_used_cents.is_some() || cost.included_limit_cents.is_some() {
                    card = card.child(div().text_sm().child(format!(
                        "{} {} / {}",
                        t.t("native.included"),
                        money(cost.included_used_cents),
                        money(cost.included_limit_cents)
                    )));
                }
                if cost.on_demand_enabled == Some(false) {
                    card = card.child(div().text_sm().child(format!(
                        "{} · {}",
                        t.t("native.ondemand"),
                        t.t("common.disabled")
                    )));
                } else if cost.on_demand_used_cents.is_some() {
                    card = card.child(div().text_sm().child(format!(
                        "{} {} / {}",
                        t.t("native.ondemand"),
                        money(cost.on_demand_used_cents),
                        money(cost.on_demand_limit_cents)
                    )));
                }
                if let Some(today) = cost.today_used_cents {
                    card = card.child(div().text_sm().child(format!(
                        "{} {} {:.2}",
                        t.t("provider.apiEstimatedCostUtc"),
                        cost.currency,
                        today as f64 / 100.
                    )));
                }
            }
            if provider == Provider::Cursor {
                if matches!(
                    data.status,
                    ProviderStatus::NotLoggedIn | ProviderStatus::DesktopInstalled
                ) {
                    card = card.child(
                        Button::new("login")
                            .primary()
                            .label(t.t("cursor.login"))
                            .on_click(cx.listener(|view, _, _, cx| {
                                let desktop = view.desktop.clone();
                                cx.defer(move |cx| {
                                    desktop.update(cx, |desktop, _| desktop.login())
                                });
                            })),
                    );
                } else if !payload.settings.cursor_compat_enabled {
                    card = card.child(
                        Button::new("enable-usage")
                            .label(t.t("cursor.readExactUsage"))
                            .on_click(cx.listener(|view, _, _, cx| view.open(View::Consent, cx))),
                    );
                }
            }
            if data.stale
                && let Some(message) = &data.message
            {
                card = card.child(
                    div()
                        .text_xs()
                        .text_color(rgb(0xe9b869))
                        .child(t.message(message)),
                );
            }
            body = body.child(card);
        }
        body = body.child(self.provider_settings(cx, payload));
        body = body.child(
            div()
                .flex()
                .gap(px(6.))
                .child(
                    Button::new("recheck")
                        .ghost()
                        .small()
                        .label(t.t("native.recheck"))
                        .on_click(cx.listener(|view, _, _, cx| view.refresh(true, cx))),
                )
                .child(
                    Button::new("check-update")
                        .ghost()
                        .small()
                        .label(t.t("native.checkUpdate"))
                        .on_click(
                            cx.listener(|view, _, _, cx| view.desktop.read(cx).check_update()),
                        ),
                ),
        );
        let update = self.updater.status();
        if let Some(version) = update.version {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .bg(rgb(0x25233e))
                    .rounded_lg()
                    .child(format!("Metra {version}"))
                    .child(
                        Button::new("update")
                            .primary()
                            .label(t.t(if self.updater.supports_install() {
                                "native.installUpdate"
                            } else {
                                "native.openRelease"
                            }))
                            .disabled(update.downloading)
                            .on_click(cx.listener(move |_view, _, _, cx| {
                                #[cfg(windows)]
                                if _view.desktop.read(cx).updater.supports_install() {
                                    _view.desktop.read(cx).install_update();
                                } else if let Some(url) = crate::updater::release_page(&version) {
                                    cx.open_url(&url);
                                }
                                #[cfg(not(windows))]
                                if let Some(url) = crate::updater::release_page(&version) {
                                    cx.open_url(&url);
                                }
                            })),
                    ),
            );
        }
        body.into_any_element()
    }
}
impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let payload = self.service.payload();
        let t = I18n::new(payload.settings.ui_language);
        if self.mode == View::Bubble {
            return self.bubble(window, cx, &payload);
        }
        let subtitle = format!(
            "{} · v{}",
            t.t("app.usageSubtitle"),
            env!("CARGO_PKG_VERSION")
        );
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .pb(px(12.))
            .child(appearance::brand(subtitle))
            .child(
                div()
                    .flex()
                    .gap(px(1.))
                    .child(
                        appearance::icon_button(
                            "header-refresh",
                            IconName::RefreshCw,
                            t.t("native.refresh"),
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.refresh(false, cx))),
                    )
                    .child(
                        appearance::icon_button(
                            "header-settings",
                            if self.mode == View::Settings {
                                IconName::ChevronLeft
                            } else {
                                IconName::Settings
                            },
                            t.t("native.settings"),
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.open(
                                if view.mode == View::Settings {
                                    View::Details
                                } else {
                                    View::Settings
                                },
                                cx,
                            )
                        })),
                    )
                    .child(
                        appearance::icon_button(
                            "header-close",
                            IconName::Close,
                            t.t("native.close"),
                        )
                        .on_click(cx.listener(|view, _, window, cx| {
                            window.remove_window();
                            let desktop = view.desktop.clone();
                            cx.defer(move |cx| {
                                desktop.update(cx, |desktop, cx| {
                                    desktop.panel = None;
                                    cx.notify();
                                })
                            });
                        })),
                    ),
            );
        let content = match self.mode {
            View::Settings => self.settings(cx, &payload),
            View::Details => self.details(cx, &payload),
            View::Consent => div()
                .flex()
                .flex_col()
                .gap_4()
                .child(t.t("native.confirmCompat"))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x9aa5bd))
                        .child(t.t("menu.cursorCompatHint")),
                )
                .child(
                    Button::new("confirm-compat")
                        .primary()
                        .label(t.t("common.confirm"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.dispatch(Change::Compat(true), cx);
                            view.open(View::Details, cx);
                        })),
                )
                .child(
                    Button::new("cancel-compat")
                        .label(t.t("common.cancel"))
                        .on_click(cx.listener(|view, _, _, cx| view.open(View::Details, cx))),
                )
                .into_any_element(),
            View::Bubble => unreachable!(),
        };
        let message = self.message.clone();
        let mut panel = div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(14.))
            .rounded(px(22.))
            .bg(linear_gradient(
                155.,
                linear_color_stop(rgb(0x191d2c), 0.),
                linear_color_stop(rgb(0x0f121d), 1.),
            ))
            .text_color(rgb(0xe4e9f5))
            .text_size(px(11.))
            .border_1()
            .border_color(rgba(0xffffff1d))
            .child(header)
            .child(
                div()
                    .id("panel-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(content),
            );
        if let Some(message) = message {
            panel = panel.child(
                div()
                    .pt_2()
                    .text_xs()
                    .text_color(rgb(0xc6b7ff))
                    .child(t.message(&message)),
            );
        }
        panel.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{clamp_origin, compact};
    use gpui_kit::{Bounds, point, px, size};
    #[test]
    fn popup_geometry_stays_in_the_selected_monitor() {
        let work = Bounds::new(point(px(-1280.), px(30.)), size(px(1280.), px(770.)));
        assert_eq!(
            clamp_origin(point(px(100.), px(900.)), size(px(370.), px(620.)), work),
            point(px(-370.), px(180.))
        );
        assert_eq!(compact(Some(1_500_000)), "1.5M");
    }
}

#[cfg(all(test, feature = "ui-tests"))]
mod ui_tests {
    use super::{Desktop, View};
    use crate::{
        service::RefreshService,
        settings::{BubblePercentMode, SettingsStore},
        updater::Updater,
    };
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext};
    use std::{
        fs::File,
        sync::{Arc, atomic::AtomicBool, mpsc},
        time::Instant,
    };

    #[gpui_kit::test]
    fn native_settings_render_and_persist_control_changes(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let service = Arc::new(RefreshService::new(SettingsStore::new(
            directory.path().join("settings.db"),
        )));
        let shared = service.clone();
        let (desktop, panel) = cx.update(|cx| {
            gpui_kit::init(cx);
            crate::appearance::init(cx);
            let (sender, receiver) = mpsc::channel();
            let desktop = cx.new(|_| Desktop {
                service,
                updater: Arc::new(Updater::default()),
                sender,
                receiver,
                tray: None,
                bubble: None,
                panel: None,
                panel_mode: View::Details,
                cycle: 0,
                last_refresh: Instant::now(),
                last_cycle: Instant::now(),
                last_second: Instant::now(),
                started: Instant::now(),
                update_checked: false,
                message: None,
                login_running: Arc::new(AtomicBool::new(false)),
                activation_file: directory.path().join("activate"),
                _instance_lock: File::create(directory.path().join("lock")).unwrap(),
            });
            // Opening while Desktop is leased must not make the renderer read that entity.
            desktop.update(cx, |desktop, cx| desktop.open_panel(View::Settings, cx));
            let panel = desktop.read(cx).panel.unwrap();
            (desktop, panel)
        });
        cx.update_window(panel, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("display-bubble").visible());
            assert!(window.find("display-status").bounds().size.width < gpui_kit::px(130.));
            window.click("percent-used", cx);
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(
                shared.payload().settings.bubble_percent_mode,
                BubblePercentMode::Used
            );
            assert_eq!(
                shared.store.load().bubble_percent_mode,
                BubblePercentMode::Used
            );
            desktop.update(cx, |desktop, cx| desktop.open_bubble(cx));
        });
    }
}
