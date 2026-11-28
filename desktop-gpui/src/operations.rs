use super::*;
use std::time::Duration;

const UPDATE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const UPDATE_RETRY: Duration = Duration::from_secs(5 * 60);

impl Dashboard {
    fn schedule_update(&mut self, delay: Duration, cx: &mut Context<Self>) {
        self.update_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = this.update(cx, |this, cx| {
                if this.update_task.is_some() {
                    this.schedule_update(UPDATE_RETRY, cx);
                } else {
                    this.check_update(false, cx);
                }
            });
        }));
    }
    pub(super) fn check_update(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.update_task.is_some() {
            return;
        }
        self.update_feedback = None;
        let updater = self.updater.clone();
        let work = cx.background_executor().spawn(async move {
            updater.check();
            updater.status()
        });
        self.update_task = Some(cx.spawn(async move |this, cx| {
            let status = work.await;
            let _ = this.update(cx, |this, cx| {
                this.update_task = None;
                this.schedule_update(
                    if status.error.is_some() {
                        UPDATE_RETRY
                    } else {
                        UPDATE_INTERVAL
                    },
                    cx,
                );
                if manual {
                    this.update_feedback = status.error.or_else(|| {
                        status.version.is_none().then(|| {
                            I18n::new(this.service.payload().settings.ui_language)
                                .t("native.upToDate")
                        })
                    });
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn install_update(&mut self, cx: &mut Context<Self>) {
        if self.update_task.is_some() {
            return;
        }
        let updater = self.updater.clone();
        let work = cx.background_executor().spawn(async move {
            (|| -> Result<bool, String> {
                if updater.supports_install() {
                    updater.install_windows()?;
                    Ok(true)
                } else {
                    let version = updater.available_version().ok_or("No update available")?;
                    metra_lib::desktop_actions::open_url(
                        &metra_lib::updater::release_page(&version).ok_or("Invalid version")?,
                    )?;
                    Ok(false)
                }
            })()
        });
        self.update_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.update_task = None;
                match result {
                    Ok(true) => cx.quit(),
                    Ok(false) => {}
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn login(&mut self, cx: &mut Context<Self>) {
        if self.login_task.is_some() {
            return;
        }
        let compat = self.service.payload().settings.cursor_compat_enabled;
        let work = cx
            .background_executor()
            .spawn(async move { metra_lib::desktop_actions::login_cursor(compat) });
        self.login_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.login_task = None;
                match result {
                    Ok(true) => this.refresh_mode(true, cx),
                    Ok(false) => {}
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
    pub(super) fn update_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let status = self.updater.status();
        let version = status.version?;
        let t = I18n::new(self.service.payload().settings.ui_language);
        let mut view = div()
            .id("update-banner")
            .v_flex()
            .flex_shrink_0()
            .max_h(relative(0.4))
            .overflow_y_scroll()
            .mx(px(14.))
            .mt(px(12.))
            .p(px(12.))
            .rounded(px(12.))
            .border_1()
            .border_color(rgba(0xb6a0ff42))
            .bg(rgba(0xb6a0ff0b))
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(19.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xd3c7ff))
                    .child(t.format("update.availableTitle", &[("version", version.clone())])),
            )
            .child(
                div()
                    .mt(px(4.))
                    .mb(px(8.))
                    .text_size(px(10.))
                    .line_height(px(16.))
                    .text_color(rgb(0xa0adc2))
                    .child(t.format(
                        "update.versionPath",
                        &[
                            ("current", env!("CARGO_PKG_VERSION").into()),
                            ("next", version),
                        ],
                    )),
            );
        if let Some(notes) = status.notes {
            let label = t.t("update.releaseNotes");
            view = view.child(
                Button::new("update-notes")
                    .ghost()
                    .h(px(28.))
                    .px(px(6.))
                    .accessibility_label(label.clone())
                    .aria_expanded(self.update_notes_expanded)
                    .child(
                        div()
                            .h_flex()
                            .w_full()
                            .justify_between()
                            .text_size(px(10.))
                            .child(label)
                            .child(
                                Icon::new(if self.update_notes_expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .with_size(px(12.))
                                .text_color(rgb(0xa0adc2)),
                            ),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_notes_expanded = !this.update_notes_expanded;
                        cx.notify();
                    })),
            );
            if self.update_notes_expanded {
                view = view.child(
                    div()
                        .id("update-notes-body")
                        .max_h(px(96.))
                        .overflow_y_scroll()
                        .text_size(px(10.))
                        .line_height(px(16.))
                        .text_color(rgb(0xa0adc2))
                        .child(notes),
                );
            }
        }
        Some(
            view.child(
                Button::new("install-update")
                    .small()
                    .outline()
                    .self_start()
                    .max_w_full()
                    .h(px(30.))
                    .px(px(11.))
                    .rounded(px(7.))
                    .border_color(rgb(0x545868))
                    .text_color(rgb(0xa0adc2))
                    .font_weight(FontWeight::MEDIUM)
                    .label(t.t(if self.updater.supports_install() {
                        "native.installUpdate"
                    } else {
                        "native.openRelease"
                    }))
                    .disabled(self.update_task.is_some())
                    .on_click(cx.listener(|this, _, _, cx| this.install_update(cx))),
            )
            .into_any_element(),
        )
    }
}
