use super::*;
use gpui_kit::component::{
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
};
use metra_lib::{
    model::Provider,
    settings::{BUBBLE_COLOR_PALETTE, BubblePercentMode, UiLanguage},
    settings_change::Change,
};

impl Dashboard {
    pub(super) fn save(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.settings_task.is_some() {
            self.pending_settings.push_back(change);
            return;
        }
        let settings = self.service.payload().settings;
        if let Err(error) = change.validate(&settings) {
            self.error = Some(error);
            cx.notify();
            return;
        }
        let refresh_after = matches!(change, Change::Compat(true));
        let service = self.service.clone();
        let work = cx.background_executor().spawn(async move {
            if let Change::Autostart(enabled) = change {
                platform::set_autostart(enabled)?;
            }
            let result = service.update_settings(|settings| change.apply(settings));
            if result.is_err() && matches!(change, Change::Autostart(_)) {
                let _ = platform::set_autostart(settings.autostart);
            }
            result
        });
        self.settings_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.settings_task = None;
                this.error = result.err();
                if this.error.is_none() {
                    this.schedule_refresh(cx);
                    if refresh_after {
                        this.refresh(cx);
                    }
                }
                if let Some(next) = this.pending_settings.pop_front() {
                    this.save(next, cx);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub(super) fn preferences(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = self.service.payload().settings;
        let t = I18n::new(s.ui_language);
        let busy = self.settings_task.is_some();
        let mut view = div().v_flex().gap_3();
        let mut languages = div().h_flex().flex_wrap().gap_2();
        for (i, (language, title)) in [
            (UiLanguage::System, t.t("menu.languageSystem")),
            (UiLanguage::En, "English".into()),
            (UiLanguage::ZhCn, "简体中文".into()),
            (UiLanguage::Ja, "日本語".into()),
            (UiLanguage::Ko, "한국어".into()),
            (UiLanguage::Fr, "Français".into()),
            (UiLanguage::De, "Deutsch".into()),
            (UiLanguage::Ru, "Русский".into()),
        ]
        .into_iter()
        .enumerate()
        {
            languages =
                languages.child(
                    Button::new(("language", i))
                        .label(title)
                        .selected(s.ui_language == language)
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.save(Change::Language(language), cx)
                        })),
                );
        }
        view =
            view.child(
                Checkbox::new("snap")
                    .label(t.t("menu.autoSnap"))
                    .checked(s.bubble_snap_enabled)
                    .disabled(busy)
                    .on_change(cx.listener(|this, enabled: &bool, _, cx| {
                        this.save(Change::Snap(*enabled), cx)
                    })),
            );
        let mode = s.display_mode;
        let modes = div()
            .h_flex()
            .gap_2()
            .child(
                Button::new("display-bubble")
                    .label(t.t("menu.displayBubble"))
                    .selected(mode == metra_lib::settings::DisplayMode::Bubble)
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save(
                            Change::Display(metra_lib::settings::DisplayMode::Bubble),
                            cx,
                        )
                    })),
            )
            .child(
                Button::new("display-tray")
                    .label(t.t("menu.displayStatusBar"))
                    .selected(mode == metra_lib::settings::DisplayMode::StatusBar)
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save(
                            Change::Display(metra_lib::settings::DisplayMode::StatusBar),
                            cx,
                        )
                    })),
            );
        view = view.child(t.t("menu.displayMode")).child(modes).child(
            Checkbox::new("autostart")
                .label(t.t("menu.autostart"))
                .checked(s.autostart)
                .disabled(busy)
                .on_change(cx.listener(|this, enabled: &bool, _, cx| {
                    this.save(Change::Autostart(*enabled), cx)
                })),
        );
        view = view
            .child(t.t("menu.language"))
            .child(languages)
            .child(t.t("menu.bubblePercentage"));
        for (i, (mode, key)) in [
            (BubblePercentMode::Remaining, "menu.remaining"),
            (BubblePercentMode::Used, "menu.used"),
        ]
        .into_iter()
        .enumerate()
        {
            view = view.child(
                Button::new(("percentage", i))
                    .label(t.t(key))
                    .selected(s.bubble_percent_mode == mode)
                    .disabled(busy)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.save(Change::Percent(mode), cx)),
                    ),
            );
        }
        let mut intervals = div().h_flex().flex_wrap().gap_2();
        for (i, minutes) in [1, 5, 15, 30, 60].into_iter().enumerate() {
            intervals = intervals.child(
                Button::new(("interval", i))
                    .label(format!("{minutes} min"))
                    .selected(s.refresh_minutes == minutes)
                    .disabled(busy)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.save(Change::Refresh(minutes), cx)),
                    ),
            );
        }
        view = view
            .child(t.t("menu.refreshInterval"))
            .child(intervals)
            .child(
                Checkbox::new("compatibility")
                    .label(t.t("menu.cursorCompat"))
                    .checked(s.cursor_compat_enabled)
                    .disabled(busy)
                    .on_change(cx.listener(|this, enabled: &bool, _, cx| {
                        if *enabled {
                            this.page = Page::Consent;
                            cx.notify();
                        } else {
                            this.save(Change::Compat(false), cx);
                        }
                    })),
            );
        for (i, provider) in s.bubble_provider_order.iter().copied().enumerate() {
            let (name, color) = match provider {
                Provider::Cursor => ("Cursor", &s.cursor_bubble_color),
                Provider::Codex => ("Codex", &s.codex_bubble_color),
                Provider::Claude => ("Claude Code", &s.claude_bubble_color),
            };
            if let Some((_, input, _, focus)) =
                self.labels.iter().find(|(p, _, _, _)| *p == provider)
            {
                let handle = div()
                    .id(("order", i))
                    .track_focus(focus)
                    .tab_stop(true)
                    .cursor_move()
                    .child("≡")
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            cx.stop_active_drag(window);
                            return;
                        }
                        let shift = match event.keystroke.key.as_str() {
                            "up" => -1,
                            "down" => 1,
                            _ => return,
                        };
                        this.move_provider(provider, shift, cx);
                    }))
                    .when(!busy, |handle| {
                        handle.on_drag(DraggedProvider(provider), |value, _, _, cx| {
                            cx.new(|_| *value)
                        })
                    });
                view = view
                    .child(
                        div()
                            .id(("provider-order", i))
                            .h_flex()
                            .gap_2()
                            .child(handle)
                            .child(name)
                            .on_drop(cx.listener(move |this, value: &DraggedProvider, _, cx| {
                                let mut order =
                                    this.service.payload().settings.bubble_provider_order;
                                let Some(from) = order.iter().position(|p| *p == value.0) else {
                                    return;
                                };
                                let Some(to) = order.iter().position(|p| *p == provider) else {
                                    return;
                                };
                                if from != to {
                                    let moved = order.remove(from);
                                    order.insert(to, moved);
                                    this.save(Change::Order(order), cx);
                                }
                            })),
                    )
                    .child(
                        Input::new(input)
                            .aria_label(
                                t.format("config.bubbleLabel", &[("provider", name.into())]),
                            )
                            .disabled(busy),
                    );
            }
            let mut palette = div().h_flex().flex_wrap().gap_1();
            for (j, value) in BUBBLE_COLOR_PALETTE.iter().copied().enumerate() {
                palette = palette.child(
                    Button::new((SharedString::from(format!("palette-{i}")), j))
                        .label(value)
                        .selected(value == color)
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.save(
                                Change::Color {
                                    provider,
                                    color: value.into(),
                                },
                                cx,
                            )
                        })),
                );
            }
            view = view
                .child(
                    Checkbox::new(("visible", i))
                        .label(t.format("config.providerVisible", &[("provider", name.into())]))
                        .checked(s.bubble_visible_providers.contains(&provider))
                        .disabled(busy)
                        .on_change(cx.listener(move |this, visible: &bool, _, cx| {
                            this.save(
                                Change::Visible {
                                    provider,
                                    visible: *visible,
                                },
                                cx,
                            )
                        })),
                )
                .child(palette);
        }
        view.child(
            Button::new("rescan")
                .label(t.t("menu.rescanCli"))
                .disabled(self.refresh_task.is_some())
                .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
        )
        .child(
            Button::new("quit")
                .label(t.t("menu.quit"))
                .on_click(|_, _, cx| cx.quit()),
        )
        .into_any_element()
    }

    pub(super) fn consent(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = I18n::new(self.service.payload().settings.ui_language);
        div()
            .v_flex()
            .gap_3()
            .child(t.t("consent.cursorTitle"))
            .child(t.t("consent.cursorReadOnly"))
            .child(t.t("consent.cursorMemoryOnly"))
            .child(
                Button::new("cancel-consent")
                    .label(t.t("common.cancel"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = Page::Details;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("enable-compatibility")
                    .label(t.t("consent.enable"))
                    .disabled(self.settings_task.is_some())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save(Change::Compat(true), cx);
                        this.page = Page::Details;
                    })),
            )
            .into_any_element()
    }
}

#[derive(Clone, Copy)]
struct DraggedProvider(Provider);
impl Render for DraggedProvider {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
            .rounded_md()
            .bg(rgb(0x232938))
            .text_color(rgb(0xffffff))
            .child(match self.0 {
                Provider::Cursor => "Cursor",
                Provider::Codex => "Codex",
                Provider::Claude => "Claude Code",
            })
    }
}
impl Dashboard {
    pub(super) fn prepare_labels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.labels.is_empty() {
            return;
        }
        let s = self.service.payload().settings;
        for (provider, label) in [
            (Provider::Cursor, s.cursor_bubble_label),
            (Provider::Codex, s.codex_bubble_label),
            (Provider::Claude, s.claude_bubble_label),
        ] {
            let input = cx.new(|cx| {
                let mut input = InputState::new(window, cx);
                input.set_value(label, window, cx);
                input
            });
            let subscription = cx.subscribe(&input, move |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    let value = input.read(cx).value().to_string();
                    let settings = this.service.payload().settings;
                    let previous = match provider {
                        Provider::Cursor => settings.cursor_bubble_label,
                        Provider::Codex => settings.codex_bubble_label,
                        Provider::Claude => settings.claude_bubble_label,
                    };
                    if value != previous {
                        this.save(
                            Change::Label {
                                provider,
                                label: value,
                            },
                            cx,
                        );
                    }
                }
            });
            self.labels
                .push((provider, input, subscription, cx.focus_handle()));
        }
    }
    fn move_provider(&mut self, provider: Provider, shift: isize, cx: &mut Context<Self>) {
        let mut order = self.service.payload().settings.bubble_provider_order;
        if let Some(from) = order.iter().position(|p| *p == provider) {
            let to = from as isize + shift;
            if to >= 0 && (to as usize) < order.len() {
                order.swap(from, to as usize);
                self.save(Change::Order(order), cx);
            }
        }
    }
}
