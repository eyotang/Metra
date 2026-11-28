#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Independent migration entry point; the released shell remains available during parity work.
use gpui_kit::prelude::*;
use gpui_kit::{
    component::{button::*, spinner::Spinner, *},
    *,
};
use metra_lib::{
    i18n::I18n,
    service::RefreshService,
    settings::{SettingsStore, config_dir},
};
use std::sync::Arc;

mod bubble;
mod formatting;
mod native_window;
mod operations;
#[path = "../../src/platform.rs"]
mod platform;
mod preferences;
mod runtime;
mod typography;
mod usage;

use typography::UI_FONT_FAMILY;

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Details,
    Settings,
    Consent,
}

/// Retain GPUI Kit's focus/input/tooltip plugins while overriding its opaque
/// root surface. Native alpha alone does not remove the theme background.
fn open_overlay<V: Render>(
    options: WindowOptions,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> Result<(AnyWindowHandle, Entity<V>), String> {
    let (handle, view) = gpui_kit::open_window(options, cx, build).map_err(|e| e.to_string())?;
    handle
        .update(cx, |_, window, cx| {
            native_window::configure_overlay(window)?;
            gpui_kit::base::Root::update(window, cx, |root, _, cx| {
                root.style().background = Some(transparent_black().into());
                root.style().text.font_weight = Some(typography::BODY_WEIGHT);
                cx.notify();
            });
            Ok::<_, String>(())
        })
        .map_err(|e| e.to_string())??;
    Ok((handle, view))
}

struct Dashboard {
    focus: FocusHandle,
    panel_activation: Option<Subscription>,
    settings_scroll: ScrollHandle,
    saved_labels: Vec<(metra_lib::model::Provider, String, String)>,
    update_notes_expanded: bool,
    page: Page,
    updater: Arc<metra_lib::updater::Updater>,
    update_task: Option<Task<()>>,
    update_timer: Option<Task<()>>,
    update_feedback: Option<String>,
    login_task: Option<Task<()>>,
    labels: Vec<(
        metra_lib::model::Provider,
        Entity<component::input::InputState>,
        Subscription,
        FocusHandle,
    )>,
    pending_settings: std::collections::VecDeque<metra_lib::settings_change::Change>,
    error: Option<String>,
    settings_task: Option<Task<()>>,
    interval_task: Option<Task<()>>,
    last_refresh: std::time::Instant,
    service: Arc<RefreshService>,
    refresh_task: Option<Task<()>>,
    refreshing: Vec<metra_lib::model::Provider>,
}
impl Dashboard {
    fn navigate(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        runtime::position_panel(page, window, cx);
        self.page = page;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn schedule_refresh(&mut self, cx: &mut Context<Self>) {
        let delay = std::time::Duration::from_secs(
            self.service.payload().settings.refresh_minutes.max(1) * 60,
        );
        let delay = delay
            .saturating_sub(self.last_refresh.elapsed())
            .max(std::time::Duration::from_secs(1));
        let timer = cx.background_executor().timer(delay);
        self.interval_task = Some(cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |this, cx| {
                if this.refresh_task.is_some() {
                    this.schedule_refresh(cx);
                } else {
                    this.refresh(cx);
                }
            });
        }));
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.refresh_mode(false, cx);
    }
    fn refresh_mode(&mut self, recheck: bool, cx: &mut Context<Self>) {
        if self.refresh_task.is_some() {
            return;
        }
        self.last_refresh = std::time::Instant::now();
        self.schedule_refresh(cx);
        self.refreshing = if recheck {
            vec![metra_lib::model::Provider::Cursor]
        } else {
            self.service.payload().settings.bubble_provider_order
        };
        let service = self.service.clone();
        let (sender, receiver) = async_channel::unbounded();
        let work = cx.background_executor().spawn(async move {
            metra_lib::providers::discovery::invalidate_shell_cache();
            let progress = |provider, running| {
                let _ = sender.send_blocking((provider, running));
            };
            if recheck {
                service.refresh_cursor_login_status_with_progress(progress);
            } else {
                service.refresh_with_progress(true, progress);
            }
        });
        self.refresh_task = Some(cx.spawn(async move |this, cx| {
            while let Ok((provider, running)) = receiver.recv().await {
                if this
                    .update(cx, |this, cx| {
                        this.refreshing.retain(|p| *p != provider);
                        if running {
                            this.refreshing.push(provider);
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
            work.await;
            let _ = this.update(cx, |this, cx| {
                this.refresh_task = None;
                this.refreshing.clear();
                cx.notify();
            });
        }));
        cx.notify();
    }
}
impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare_labels(window, cx);
        let payload = self.service.payload();
        let t = I18n::new(payload.settings.ui_language);
        let details = self.page == Page::Details;
        let mut actions = div().h_flex().items_center().gap(px(2.));
        if details {
            actions = actions.child(
                Button::new("refresh")
                    .ghost()
                    .size(px(30.))
                    .icon(IconName::RefreshCw)
                    .accessibility_label(t.t("refresh.nowUsage"))
                    .tooltip(t.t("refresh.nowUsage"))
                    .disabled(self.refresh_task.is_some())
                    .loading(self.refresh_task.is_some())
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            );
        } else {
            actions = actions.child(
                Button::new("details")
                    .ghost()
                    .size(px(30.))
                    .icon(IconName::ChevronLeft)
                    .accessibility_label(t.t("native.details"))
                    .tooltip(t.t("native.details"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate(Page::Details, window, cx);
                    })),
            );
        }
        if self.page != Page::Settings {
            actions = actions.child(
                Button::new("settings")
                    .ghost()
                    .size(px(30.))
                    .icon(IconName::Settings)
                    .accessibility_label(t.t("native.settings"))
                    .tooltip(t.t("native.settings"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate(Page::Settings, window, cx);
                    })),
            );
        }
        actions = actions.child(
            Button::new("close-panel")
                .ghost()
                .size(px(30.))
                .icon(IconName::Close)
                .accessibility_label(t.t("native.close"))
                .tooltip(t.t("native.close"))
                .on_click(|_, window, _| window.remove_window()),
        );
        static APP_ICON: std::sync::LazyLock<Arc<Image>> = std::sync::LazyLock::new(|| {
            Arc::new(Image::from_bytes(
                ImageFormat::Png,
                include_bytes!("../../assets/icons/64x64.png").to_vec(),
            ))
        });
        let mut header = div().id("header").v_flex().flex_shrink_0().child(
            div()
                .h_flex()
                .h(px(72.))
                .items_center()
                .justify_between()
                .gap_2()
                .px(px(17.))
                .border_b_1()
                .border_color(rgba(0xffffff09))
                .child(
                    div()
                        .h_flex()
                        .items_center()
                        .gap(px(9.))
                        .child(
                            img(APP_ICON.clone())
                                .size(px(30.))
                                .flex_none()
                                .rounded(px(8.)),
                        )
                        .child(
                            div()
                                .v_flex()
                                .gap(px(3.))
                                .child(
                                    div()
                                        .h_flex()
                                        .items_baseline()
                                        .gap(px(7.))
                                        .child(
                                            div()
                                                .text_size(px(17.))
                                                .font_weight(FontWeight::BOLD)
                                                .child("Metra"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(9.))
                                                .text_color(rgb(0xa0adc2))
                                                .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
                                        ),
                                )
                                .child(div().text_size(px(10.)).text_color(rgb(0xa0adc2)).child(
                                    t.t(if self.page == Page::Settings {
                                        "native.settings"
                                    } else {
                                        "app.usageSubtitle"
                                    }),
                                )),
                        ),
                )
                .child(actions),
        );
        let mut content = div()
            .id("body")
            .v_flex()
            .gap_3()
            .p(px(14.))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        if self.page != Page::Consent {
            if let Some(feedback) = &self.update_feedback {
                header = header.child(feedback.clone());
            }
        }
        let update_banner = (self.page != Page::Consent)
            .then(|| self.update_banner(cx))
            .flatten();
        if let Some(error) = &self.error {
            header = header.child(error.clone());
        }
        let active = self.refresh_task.is_some() || self.settings_task.is_some();
        let status = div()
            .h_flex()
            .items_center()
            .gap(px(6.))
            .px(px(17.))
            .h(px(30.))
            .flex_shrink_0()
            .border_t_1()
            .border_color(rgba(0xffffff09))
            .text_size(px(10.))
            .text_color(rgb(0xa0adc2))
            .when(active, |status| {
                status.child(Spinner::new().with_size(px(12.)))
            })
            .when(!active, |status| {
                status.child(div().size(px(5.)).rounded_full().bg(rgb(0x64ccb0)))
            })
            .child(if self.refresh_task.is_some() {
                t.t("native.refreshing")
            } else {
                t.format(
                    "refresh.everyMinutes",
                    &[("minutes", payload.settings.refresh_minutes.to_string())],
                )
            });
        if self.page == Page::Settings {
            return div()
                .v_flex()
                .size_full()
                .id("panel")
                .track_focus(&self.focus)
                .on_key_down(runtime::panel_key_down)
                .rounded(px(22.))
                .border_1()
                .border_color(rgba(0xffffff20))
                .overflow_hidden()
                .text_size(px(12.))
                .bg(rgb(0x171c27))
                .text_color(rgb(0xedf0f8))
                .font(typography::ui_font(t.locale))
                .child(header)
                .children(update_banner)
                .child(
                    content
                        .track_scroll(&self.settings_scroll)
                        .child(self.preferences(window, cx)),
                )
                .child(status);
        }
        if self.page == Page::Consent {
            return div()
                .v_flex()
                .size_full()
                .id("panel")
                .track_focus(&self.focus)
                .on_key_down(runtime::panel_key_down)
                .rounded(px(22.))
                .overflow_hidden()
                .text_size(px(12.))
                .bg(rgb(0x121620))
                .text_color(rgb(0xe8ecf3))
                .font(typography::ui_font(t.locale))
                .child(header)
                .child(content.child(self.consent(cx)))
                .child(status);
        }
        for provider in payload.settings.bubble_provider_order {
            let (_, snapshot) = match provider {
                metra_lib::model::Provider::Cursor => ("Cursor", &payload.snapshot.cursor),
                metra_lib::model::Provider::Codex => ("Codex", &payload.snapshot.codex),
                metra_lib::model::Provider::Claude => ("Claude Code", &payload.snapshot.claude),
            };
            content = content.child(self.usage_card(provider, snapshot, t, cx));
        }
        content = content.child(
            div()
                .h_flex()
                .gap_2()
                .child(
                    Button::new("recheck")
                        .font_weight(typography::CONTROL_WEIGHT)
                        .label(t.t("native.recheck"))
                        .disabled(self.refresh_task.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_mode(true, cx))),
                )
                .child(
                    Button::new("check-update")
                        .font_weight(typography::CONTROL_WEIGHT)
                        .label(t.t("native.checkUpdate"))
                        .loading(self.update_task.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.check_update(true, cx))),
                ),
        );
        div()
            .v_flex()
            .size_full()
            .id("panel")
            .track_focus(&self.focus)
            .on_key_down(runtime::panel_key_down)
            .rounded(px(22.))
            .overflow_hidden()
            .text_size(px(12.))
            .bg(rgb(0x121620))
            .text_color(rgb(0xe8ecf3))
            .font(typography::ui_font(t.locale))
            .child(header)
            .children(update_banner)
            .child(content)
            .child(status)
    }
}
fn main() {
    let mode = std::env::args().nth(1);
    if matches!(mode.as_deref(), Some("--usage-json" | "--watch-json")) {
        if let Err(error) = metra_lib::headless::run(mode.as_deref() == Some("--watch-json")) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_some() {
        // The positioned overlay contract uses X11/XWayland, as the existing shell does.
        // Set before GPUI or any collector threads are started.
        unsafe {
            std::env::remove_var("WAYLAND_DISPLAY");
            std::env::remove_var("WAYLAND_SOCKET");
        }
    }

    let config = config_dir().expect("configuration directory");
    std::fs::create_dir_all(&config).expect("create configuration directory");
    let lock = match runtime::instance(&config) {
        Ok(Some(lock)) => lock,
        Ok(None) => return,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let service = Arc::new(RefreshService::new(SettingsStore::new(
        config.join("settings.db"),
    )));
    let application = gpui_kit::application()
        .with_quit_mode(QuitMode::Explicit)
        .with_assets(gpui_kit::assets::Assets);
    #[cfg(target_os = "macos")]
    let application = application.with_windowing(WindowingRequest::Headless);
    application.on_reopen(|cx| {
        if cx.has_global::<runtime::Runtime>() {
            cx.defer(|cx| runtime::open_panel(Page::Details, cx));
        }
    });
    application.run(move |cx| {
        gpui_kit::init(cx);
        component::Theme::change(component::ThemeMode::Dark, None, cx);
        let theme = component::Theme::global_mut(cx);
        theme.font_size = px(12.);
        theme.font_family = UI_FONT_FAMILY.into();
        theme.colors.popover = rgb(0x263147).into();
        theme.colors.popover_foreground = rgb(0xedf0f8).into();
        theme.colors.primary = rgb(0xb6a0ff).into();
        theme.colors.primary_foreground = rgb(0x201832).into();
        theme.colors.accent = theme.colors.primary;
        theme.colors.accent_foreground = theme.colors.primary_foreground;
        theme.colors.ring = rgb(0xc4b4ff).into();
        theme.tokens.primary = theme.colors.primary.into();
        theme.tokens.accent = theme.colors.accent.into();
        if let Err(error) = typography::load_fonts(cx) {
            eprintln!("Unable to load bundled font: {error}");
        }

        if let Err(error) = runtime::start(service, config, lock, cx) {
            eprintln!("{error}");
            cx.quit();
        }
    });
}
