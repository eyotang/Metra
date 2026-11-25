use super::*;
impl Dashboard {
    pub(super) fn schedule_update(&mut self, delay: std::time::Duration, cx: &mut Context<Self>) {
        self.update_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = this.update(cx, |this, cx| {
                this.check_update(false, cx);
                this.schedule_update(std::time::Duration::from_secs(86400), cx);
            });
        }));
    }
    pub(super) fn check_update(&mut self, manual: bool, cx: &mut Context<Self>) {
        if self.update_task.is_some() {
            return;
        }
        let updater = self.updater.clone();
        let work = cx.background_executor().spawn(async move {
            updater.check();
            updater.status()
        });
        self.update_task = Some(cx.spawn(async move |this, cx| {
            let status = work.await;
            let _ = this.update(cx, |this, cx| {
                this.update_task = None;
                this.error = status.error.or_else(|| {
                    (manual && status.version.is_none()).then(|| {
                        I18n::new(this.service.payload().settings.ui_language).t("native.upToDate")
                    })
                });
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
                    let version = updater.status().version.ok_or("No update available")?;
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
    pub(super) fn update_banner(&self, cx: &mut Context<Self>) -> AnyElement {
        let status = self.updater.status();
        let t = I18n::new(self.service.payload().settings.ui_language);
        let mut view = div().v_flex().gap_1();
        if let Some(version) = status.version {
            view = view
                .child(t.format("update.availableTitle", &[("version", version.clone())]))
                .child(t.format(
                    "update.versionPath",
                    &[
                        ("current", env!("CARGO_PKG_VERSION").into()),
                        ("next", version),
                    ],
                ));
            if let Some(notes) = status.notes {
                view = view.child(
                    Button::new("update-notes")
                        .label(t.t("common.operation"))
                        .selected(self.update_notes_expanded)
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
                            .child(notes),
                    );
                }
            }
            view = view.child(
                Button::new("install-update")
                    .label(t.t(if self.updater.supports_install() {
                        "native.installUpdate"
                    } else {
                        "native.openRelease"
                    }))
                    .disabled(self.update_task.is_some())
                    .on_click(cx.listener(|this, _, _, cx| this.install_update(cx))),
            );
            if let Some(error) = status.error {
                view = view.child(error);
            }
        }
        view.into_any_element()
    }
}
