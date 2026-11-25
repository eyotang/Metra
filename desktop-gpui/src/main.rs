//! Independent migration entry point; the released shell remains available during parity work.
use gpui_kit::prelude::*;
use gpui_kit::{
    component::{button::*, *},
    *,
};
use metra_lib::{
    i18n::I18n,
    service::RefreshService,
    settings::{SettingsStore, config_dir},
};
use std::sync::Arc;

mod bubble;
mod native_window;
#[path = "../../src/platform.rs"]
mod platform;
mod preferences;
mod runtime;
mod usage;

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Details,
    Settings,
    Consent,
}

struct Dashboard {
    page: Page,
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
    service: Arc<RefreshService>,
    refresh_task: Option<Task<()>>,
    refreshing: Vec<metra_lib::model::Provider>,
}
impl Dashboard {
    fn schedule_refresh(&mut self, cx: &mut Context<Self>) {
        let delay = std::time::Duration::from_secs(
            self.service.payload().settings.refresh_minutes.max(1) * 60,
        );
        let timer = cx.background_executor().timer(delay);
        self.interval_task = Some(cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |this, cx| {
                this.refresh(cx);
                this.schedule_refresh(cx);
            });
        }));
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refresh_task.is_some() {
            return;
        }
        let service = self.service.clone();
        let (sender, receiver) = async_channel::unbounded();
        let work = cx.background_executor().spawn(async move {
            service.refresh_with_progress(true, |provider, running| {
                let _ = sender.send_blocking((provider, running));
            });
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
        let mut header = div()
            .id("header")
            .v_flex()
            .gap_3()
            .p_4()
            .flex_shrink_0()
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .child("Metra")
                    .child(env!("CARGO_PKG_VERSION")),
            )
            .child(
                Button::new("refresh")
                    .label(t.t("native.refresh"))
                    .disabled(self.refresh_task.is_some())
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            );
        header = header.child(
            div()
                .h_flex()
                .gap_2()
                .child(
                    Button::new("details")
                        .label(t.t("native.details"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.page = Page::Details;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("settings")
                        .label(t.t("native.settings"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.page = Page::Settings;
                            cx.notify();
                        })),
                ),
        );
        let mut content = div()
            .id("body")
            .v_flex()
            .gap_3()
            .p_4()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll();
        if let Some(error) = &self.error {
            header = header.child(error.clone());
        }
        if self.page == Page::Settings {
            return div()
                .v_flex()
                .size_full()
                .child(header)
                .child(content.child(self.preferences(cx)));
        }
        if self.page == Page::Consent {
            return div()
                .v_flex()
                .size_full()
                .child(header)
                .child(content.child(self.consent(cx)));
        }
        for provider in payload.settings.bubble_provider_order {
            let (_, snapshot) = match provider {
                metra_lib::model::Provider::Cursor => ("Cursor", &payload.snapshot.cursor),
                metra_lib::model::Provider::Codex => ("Codex", &payload.snapshot.codex),
                metra_lib::model::Provider::Claude => ("Claude Code", &payload.snapshot.claude),
            };
            content = content.child(self.usage_card(provider, snapshot, t, cx));
        }
        div().v_flex().size_full().child(header).child(content)
    }
}
fn main() {
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
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            if let Err(error) = runtime::start(service, config, lock, cx) {
                eprintln!("{error}");
                cx.quit();
            }
        });
}
