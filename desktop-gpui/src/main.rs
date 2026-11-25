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

struct Dashboard {
    service: Arc<RefreshService>,
    refresh_task: Option<Task<()>>,
    refreshing: Vec<metra_lib::model::Provider>,
}
impl Dashboard {
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let payload = self.service.payload();
        let t = I18n::new(payload.settings.ui_language);
        let mut content = div()
            .id("usage")
            .v_flex()
            .gap_3()
            .p_4()
            .size_full()
            .overflow_y_scroll()
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
        for provider in payload.settings.bubble_provider_order {
            let (name, snapshot) = match provider {
                metra_lib::model::Provider::Cursor => ("Cursor", &payload.snapshot.cursor),
                metra_lib::model::Provider::Codex => ("Codex", &payload.snapshot.codex),
                metra_lib::model::Provider::Claude => ("Claude Code", &payload.snapshot.claude),
            };
            let mut card = div()
                .v_flex()
                .gap_2()
                .p_3()
                .border_1()
                .rounded_lg()
                .child(name)
                .when(self.refreshing.contains(&provider), |card| {
                    card.child(t.t("refresh.refreshing"))
                })
                .child(snapshot.plan.clone().unwrap_or_else(|| t.t("plan.unknown")));
            for quota in &snapshot.quotas {
                card = card.child(format!(
                    "{} · {}",
                    quota.label,
                    t.format(
                        "quota.remaining",
                        &[("value", format!("{:.0}", quota.remaining_percent))]
                    )
                ));
            }
            if let Some(message) = &snapshot.message {
                card = card.child(t.message(message));
            }
            content = content.child(card);
        }
        content
    }
}
fn main() {
    let config = config_dir().expect("configuration directory");
    std::fs::create_dir_all(&config).expect("create configuration directory");
    let service = Arc::new(RefreshService::new(SettingsStore::new(
        config.join("settings.db"),
    )));
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let bounds = Bounds::centered(None, size(px(420.), px(680.)), cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    cx.new(|cx| {
                        let mut dashboard = Dashboard {
                            service,
                            refresh_task: None,
                            refreshing: Vec::new(),
                        };
                        dashboard.refresh(cx);
                        dashboard
                    })
                },
            )
            .expect("open Metra window");
            cx.activate(true);
        });
}
