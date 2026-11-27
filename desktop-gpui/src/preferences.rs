use super::*;
use gpui_kit::base::Switch;
use gpui_kit::component::{
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    popover::Popover,
    searchable_list::{SearchableListItem, SearchableVec},
    select::{Select, SelectEvent, SelectState},
};
use metra_lib::{
    model::Provider,
    settings::{BUBBLE_COLOR_PALETTE, BubblePercentMode, UiLanguage},
    settings_change::Change,
};
use std::{cell::Cell, rc::Rc};

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
        if matches!(
            change,
            Change::Display(metra_lib::settings::DisplayMode::StatusBar)
        ) && !runtime::tray_available(cx)
        {
            self.error = Some("Status bar is unavailable on this desktop".into());
            cx.notify();
            return;
        }
        let interval_changed = matches!(change, Change::Refresh(_));
        let label_submission = match &change {
            Change::Label { provider, label } => Some((*provider, label.clone())),
            _ => None,
        };
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
                    if let Some((provider, submitted)) = label_submission {
                        let saved = this.service.payload().settings;
                        let normalized = match provider {
                            Provider::Cursor => saved.cursor_bubble_label,
                            Provider::Codex => saved.codex_bubble_label,
                            Provider::Claude => saved.claude_bubble_label,
                        };
                        this.saved_labels.push((provider, submitted, normalized));
                    }
                    if interval_changed {
                        this.schedule_refresh(cx);
                    }
                    if refresh_after {
                        if this.page == Page::Consent {
                            this.page = Page::Details;
                            if let Some(panel) = cx.global::<runtime::Runtime>().panel {
                                let _ = panel.update(cx, |_, window, cx| {
                                    runtime::position_panel(Page::Details, window, cx);
                                    window.focus(&this.focus, cx);
                                });
                            }
                        }
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

    pub(super) fn preferences(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let s = self.service.payload().settings;
        let t = I18n::new(s.ui_language);
        let busy = self.settings_task.is_some();
        use metra_lib::settings::DisplayMode;
        let mut view = div()
            .v_flex()
            .child(preference_select(
                "language",
                t.t("menu.language"),
                s.ui_language,
                [
                    (UiLanguage::System, t.t("menu.languageSystem")),
                    (UiLanguage::ZhCn, "简体中文".into()),
                    (UiLanguage::En, "English".into()),
                    (UiLanguage::Ja, "日本語".into()),
                    (UiLanguage::Ko, "한국어".into()),
                    (UiLanguage::Fr, "Français".into()),
                    (UiLanguage::De, "Deutsch".into()),
                    (UiLanguage::Ru, "Русский".into()),
                ],
                Change::Language,
                busy,
                window,
                cx,
            ))
            .child(preference_select(
                "display",
                t.t("menu.displayMode"),
                s.display_mode,
                [
                    (DisplayMode::Bubble, t.t("menu.displayBubble")),
                    (DisplayMode::StatusBar, t.t("menu.displayStatusBar")),
                ],
                Change::Display,
                busy,
                window,
                cx,
            ))
            .child(preference_select(
                "percentage",
                t.t("menu.bubblePercentage"),
                s.bubble_percent_mode,
                [
                    (BubblePercentMode::Remaining, t.t("menu.remaining")),
                    (BubblePercentMode::Used, t.t("menu.used")),
                ],
                Change::Percent,
                busy,
                window,
                cx,
            ))
            .child(preference_select(
                "interval",
                t.t("menu.refreshInterval"),
                s.refresh_minutes,
                [1, 5, 15, 30, 60].map(|minutes| (minutes, format!("{minutes} min"))),
                Change::Refresh,
                busy,
                window,
                cx,
            ))
            .child(setting_switch(
                "snap",
                t.t("menu.autoSnap"),
                Some(t.t("menu.autoSnapHint")),
                s.bubble_snap_enabled,
                busy,
                cx.listener(|this, enabled: &bool, _, cx| this.save(Change::Snap(*enabled), cx)),
            ))
            .child(setting_switch(
                "autostart",
                t.t("menu.autostart"),
                None,
                s.autostart,
                busy,
                cx.listener(|this, enabled: &bool, _, cx| {
                    this.save(Change::Autostart(*enabled), cx)
                }),
            ))
            .child(setting_switch(
                "compatibility",
                t.t("menu.cursorCompat"),
                Some(t.t("menu.cursorCompatHint")),
                s.cursor_compat_enabled,
                busy,
                cx.listener(|this, enabled: &bool, window, cx| {
                    if *enabled {
                        this.navigate(Page::Consent, window, cx);
                    } else {
                        this.save(Change::Compat(false), cx);
                    }
                }),
            ))
            .child(
                div()
                    .v_flex()
                    .pt(px(19.))
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t.t("config.title")),
                    )
                    .child(setting_hint(t.t("config.reorderHint"))),
            );
        for provider in s.bubble_provider_order.iter().copied() {
            let (name, color) = match provider {
                Provider::Cursor => ("Cursor", &s.cursor_bubble_color),
                Provider::Codex => ("Codex", &s.codex_bubble_color),
                Provider::Claude => ("Claude Code", &s.claude_bubble_color),
            };
            let accent = provider_accent(color);
            let Some((_, input, _, focus)) = self.labels.iter().find(|(p, ..)| *p == provider)
            else {
                continue;
            };
            let drag_focus = focus.clone();
            let handle = div()
                .id("order-handle")
                .track_focus(focus)
                .role(Role::Button)
                .aria_label(t.format("config.dragProviderOrder", &[("provider", name.into())]))
                .tab_stop(!busy)
                .size(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_move()
                .text_color(rgb(0xa0adc2))
                .hover(|style| style.text_color(accent).bg(rgba(0xb8a6ff15)))
                .rounded_md()
                .child("⠿")
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if this.settings_task.is_some() || cx.has_active_drag() {
                        return;
                    }
                    let shift = match event.keystroke.key.as_str() {
                        "up" => -1,
                        "down" => 1,
                        _ => return,
                    };
                    cx.stop_propagation();
                    this.move_provider(provider, shift, cx);
                }))
                .when(!busy, |handle| {
                    handle.on_drag(DraggedProvider(provider), move |value, _, window, cx| {
                        window.focus(&drag_focus, cx);
                        cx.new(|_| *value)
                    })
                });
            let color_picker = Self::provider_color(provider, name, color, busy, t, cx);
            let row =
                div()
                    .id(name)
                    .relative()
                    .v_flex()
                    .mt(px(7.))
                    .gap(px(6.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(rgba(0xffffff10))
                    .bg(rgba(0xffffff03))
                    .py(px(8.))
                    .px(px(10.))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(5.))
                            .child(handle)
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(accent)
                                    .child(name),
                            )
                            .child(
                                Checkbox::new("visible")
                                    .accessibility_label(t.format(
                                        "config.providerVisible",
                                        &[("provider", name.into())],
                                    ))
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
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(7.))
                            .child(
                                div().w(px(49.)).flex_none().child(
                                    Input::new(input)
                                        .small()
                                        .min_h(px(30.))
                                        .rounded(px(7.))
                                        .px(px(5.))
                                        .text_align(TextAlign::Center)
                                        .text_color(accent)
                                        .bg(rgb(0x1c2538))
                                        .border_color(rgb(0x545868))
                                        .aria_label(t.format(
                                            "config.bubbleLabel",
                                            &[("provider", name.into())],
                                        ))
                                        .disabled(busy),
                                ),
                            )
                            .child(div().min_w_0().flex_1().child(color_picker))
                            .map(|fields| {
                                #[cfg(feature = "native-smoke")]
                                return fields.on_children_prepainted(move |bounds, _, _| {
                                    if provider == Provider::Codex {
                                        smoke::record_color_trigger(bounds[1].center());
                                    }
                                });
                                #[cfg(not(feature = "native-smoke"))]
                                fields
                            }),
                    );
            let row_bounds = Rc::new(Cell::new(None));
            let measured_bounds = row_bounds.clone();
            let row = row.child(
                canvas(
                    move |bounds, _, _| {
                        measured_bounds.set(Some(bounds));
                        #[cfg(feature = "native-smoke")]
                        smoke::record_row(provider, bounds);
                    },
                    |bounds, (), window, cx| {
                        let pointer = window.mouse_position();
                        if cx.has_active_drag() && bounds.contains(&pointer) {
                            let y = if pointer.y < bounds.center().y {
                                bounds.top()
                            } else {
                                bounds.bottom() - px(2.)
                            };
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(bounds.left(), y),
                                    size(bounds.size.width, px(2.)),
                                ),
                                rgb(0x9c83ff),
                            ));
                        }
                    },
                )
                .absolute()
                .inset_0(),
            );
            view = view.child(row.on_drop(cx.listener(
                move |this, value: &DraggedProvider, window, cx| {
                    if this.settings_task.is_none()
                        && let Some(bounds) = row_bounds.get()
                        && let Some(order) = provider_drop_order(
                            this.service.payload().settings.bubble_provider_order,
                            value.0,
                            provider,
                            window.mouse_position().y >= bounds.center().y,
                        )
                    {
                        this.save(Change::Order(order), cx);
                    }
                },
            )));
        }
        view.child(
            div()
                .h_flex()
                .flex_wrap()
                .mt_3()
                .gap(px(6.))
                .child(
                    Button::new("rescan")
                        .small()
                        .outline()
                        .h(px(30.))
                        .border_color(rgb(0x545868))
                        .text_color(rgb(0xa0adc2))
                        .label(t.t("menu.rescanCli"))
                        .loading(self.refresh_task.is_some())
                        .disabled(self.refresh_task.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                )
                .child(
                    Button::new("quit")
                        .small()
                        .danger()
                        .outline()
                        .h(px(30.))
                        .label(t.t("menu.quit"))
                        .on_click(|_, _, cx| cx.quit()),
                ),
        )
        .on_drag_move(
            cx.listener(|this, event: &DragMoveEvent<DraggedProvider>, window, _| {
                let scroll = &this.settings_scroll;
                let bounds = scroll.bounds();
                let pointer = event.event.position;
                if !bounds.contains(&pointer) {
                    return;
                }
                let step = if pointer.y < bounds.top() + px(24.) {
                    px(12.)
                } else if pointer.y > bounds.bottom() - px(24.) {
                    px(-12.)
                } else {
                    return;
                };
                let old = scroll.offset();
                let y = (old.y + step).clamp(-scroll.max_offset().y, px(0.));
                if old.y != y {
                    scroll.set_offset(point(old.x, y));
                    window.refresh();
                }
            }),
        )
        .into_any_element()
    }

    fn provider_color(
        provider: Provider,
        name: &'static str,
        color: &str,
        busy: bool,
        t: I18n,
        cx: &mut Context<Self>,
    ) -> Popover {
        let dashboard = cx.entity().downgrade();
        let color = color.to_owned();
        let accent = provider_accent(&color);
        let mut trigger = StyleRefinement::default();
        trigger.size.width = Some(relative(1.).into());
        Popover::new("color-picker")
            .anchor(Anchor::BottomRight)
            .arrow(false)
            .trigger_style(trigger)
            .w(px(240.))
            .p(px(10.))
            .bg(rgb(0x263147))
            .border_color(rgba(0xffffff1a))
            .rounded(px(7.))
            .trigger(
                Button::new("color")
                    .small()
                    .outline()
                    .w_full()
                    .h(px(30.))
                    .bg(rgb(0x1c2538))
                    .border_color(rgb(0x545868))
                    .child(
                        div()
                            .w_full()
                            .h_flex()
                            .gap(px(6.))
                            .items_center()
                            .child(div().size(px(12.)).rounded(px(4.)).bg(accent))
                            .child(div().text_color(rgb(0xa0adc2)).child(color.clone())),
                    )
                    .accessibility_label(
                        t.format("config.chooseColor", &[("provider", name.into())]),
                    )
                    .disabled(busy),
            )
            .content(move |_, _, cx| {
                let mut palette = div().v_flex().gap(px(4.));
                for (row_index, colors) in BUBBLE_COLOR_PALETTE.chunks(11).enumerate() {
                    let mut row = div().h_flex().gap(px(4.));
                    for (column, &value) in colors.iter().enumerate() {
                        let dashboard = dashboard.clone();
                        let selected = value == color;
                        row = row.child(
                            Button::new(("swatch", row_index * 11 + column))
                                .accessibility_label(t.format(
                                    "config.chooseCurrentColor",
                                    &[("provider", name.into()), ("color", value.into())],
                                ))
                                .tooltip(value)
                                .size(px(16.))
                                .min_w(px(16.))
                                .p_0()
                                .rounded(px(3.))
                                .bg(provider_accent(value))
                                .border_1()
                                .border_color(rgba(0xffffff30))
                                .selected(selected)
                                .when(selected, |swatch| {
                                    swatch.border_2().border_color(rgb(0xffffff))
                                })
                                .on_click(cx.listener(move |popover, _, window, cx| {
                                    popover.dismiss(window, cx);
                                    let _ = dashboard.update(cx, |this, cx| {
                                        this.save(
                                            Change::Color {
                                                provider,
                                                color: value.into(),
                                            },
                                            cx,
                                        )
                                    });
                                })),
                        );
                    }
                    #[cfg(feature = "native-smoke")]
                    let row = row.on_children_prepainted(move |bounds, _, _| {
                        if provider == Provider::Codex && row_index == 0 {
                            smoke::record_swatch(bounds[0].center());
                        }
                    });
                    palette = palette.child(row);
                }
                palette
            })
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
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.navigate(Page::Details, window, cx);
                    })),
            )
            .child(
                Button::new("enable-compatibility")
                    .label(t.t("consent.enable"))
                    .disabled(self.settings_task.is_some())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save(Change::Compat(true), cx);
                    })),
            )
            .into_any_element()
    }
}

#[derive(Clone, PartialEq)]
struct PreferenceOption<T> {
    value: T,
    title: SharedString,
}

impl<T: Copy + PartialEq> SearchableListItem for PreferenceOption<T> {
    type Value = T;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &T {
        &self.value
    }
}

struct PreferenceSelect<T: Copy + PartialEq + 'static> {
    control: Entity<SelectState<SearchableVec<PreferenceOption<T>>>>,
    options: Vec<PreferenceOption<T>>,
    saved: Rc<Cell<T>>,
    _subscription: Subscription,
}

fn preference_select<T: Copy + PartialEq + 'static, const N: usize>(
    id: &'static str,
    title: String,
    current: T,
    options: [(T, String); N],
    change: fn(T) -> Change,
    busy: bool,
    window: &mut Window,
    cx: &mut Context<Dashboard>,
) -> Div {
    let options = options
        .into_iter()
        .map(|(value, title)| PreferenceOption {
            value,
            title: title.into(),
        })
        .collect::<Vec<_>>();
    let dashboard = cx.entity().downgrade();
    let state = window.use_keyed_state(format!("preference-select-{id}"), cx, |window, cx| {
        let selected = options
            .iter()
            .position(|option| option.value == current)
            .map(|index| IndexPath::new(0).row(index));
        let control = cx
            .new(|cx| SelectState::new(SearchableVec::new(options.clone()), selected, window, cx));
        let saved = Rc::new(Cell::new(current));
        let current = saved.clone();
        let subscription = window.subscribe(&control, cx, move |_, event, _, cx| {
            if let SelectEvent::Confirm(Some(value)) = event
                && *value != current.get()
            {
                let _ = dashboard.update(cx, |this, cx| this.save(change(*value), cx));
            }
        });
        PreferenceSelect {
            control,
            options: options.clone(),
            saved,
            _subscription: subscription,
        }
    });
    state.read(cx).saved.set(current);
    let control = state.read(cx).control.clone();
    if state.read(cx).options != options {
        control.update(cx, |view, cx| {
            view.set_items(SearchableVec::new(options.clone()), window, cx)
        });
        state.update(cx, |state, _| state.options = options);
    }
    if !busy && control.read(cx).selected_value() != Some(&current) {
        control.update(cx, |view, cx| view.set_selected_value(&current, window, cx));
    }
    let menu = Select::new(&control)
        .id(id)
        .accessibility_label(title.clone())
        .small()
        .h(px(30.))
        .rounded(px(7.))
        .bg(rgb(0x1c2538))
        .border_color(rgb(0x545868))
        .disabled(busy);
    let row = setting_row(title, div().w(px(145.)).h(px(30.)).child(menu));
    #[cfg(feature = "native-smoke")]
    let row = row.on_children_prepainted(move |bounds, _, _| {
        if id == "language" {
            smoke::record_language_trigger(bounds[1].center());
        }
    });
    row
}

fn setting_row(title: String, control: impl IntoElement) -> Div {
    div()
        .h_flex()
        .min_h(px(49.))
        .items_center()
        .justify_between()
        .gap_3()
        .py(px(10.))
        .border_b_1()
        .border_color(rgba(0xffffff09))
        .child(setting_label(title, None))
        .child(div().flex_none().child(control))
}

fn setting_switch(
    id: &'static str,
    title: String,
    hint: Option<String>,
    checked: bool,
    busy: bool,
    change: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> Switch {
    Switch::new(id)
        .accessibility_label(title.clone())
        .checked(checked)
        .disabled(busy)
        .on_change(move |enabled, _, window, cx| change(&enabled, window, cx))
        .h_flex()
        .min_h(px(49.))
        .items_center()
        .justify_between()
        .gap_3()
        .py(px(10.))
        .border_b_1()
        .border_color(rgba(0xffffff09))
        .focus_visible(|style| style.border_color(rgb(0xc4b4ff)))
        .when(busy, |row| row.opacity(0.5))
        .child(setting_label(title, hint))
        .child(
            div()
                .h_flex()
                .items_center()
                .flex_none()
                .w(px(24.))
                .h(px(14.))
                .p(px(2.))
                .rounded_full()
                .border_1()
                .when(checked, |track| {
                    track
                        .justify_end()
                        .bg(rgb(0xb6a0ff))
                        .border_color(rgb(0xc4b4ff))
                })
                .when(!checked, |track| {
                    track.bg(rgb(0x1c2538)).border_color(rgb(0x545868))
                })
                .child(div().size(px(8.)).rounded_full().bg(rgb(if checked {
                    0x201832
                } else {
                    0x545868
                }))),
        )
}

fn setting_label(title: String, hint: Option<String>) -> Div {
    div()
        .v_flex()
        .min_w_0()
        .gap(px(4.))
        .text_size(px(11.))
        .child(title)
        .when_some(hint, |label, hint| label.child(setting_hint(hint)))
}

fn setting_hint(text: String) -> Div {
    div()
        .text_size(px(9.))
        .line_height(px(14.))
        .text_color(rgb(0xa0adc2))
        .child(text)
}

fn provider_accent(color: &str) -> Rgba {
    rgb(u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0x9c83ff))
}

fn provider_drop_order(
    mut order: Vec<Provider>,
    source: Provider,
    target: Provider,
    after: bool,
) -> Option<Vec<Provider>> {
    if source == target {
        return None;
    }
    let from = order.iter().position(|provider| *provider == source)?;
    order.remove(from);
    let to = order.iter().position(|provider| *provider == target)? + usize::from(after);
    if from == to {
        return None;
    }
    order.insert(to, source);
    Some(order)
}

#[cfg(feature = "native-smoke")]
pub(super) mod smoke {
    use super::*;

    thread_local! {
        static ROWS: Cell<[Option<Bounds<Pixels>>; 3]> = const { Cell::new([None; 3]) };
        static LANGUAGE_TRIGGER: Cell<Option<Point<Pixels>>> = const { Cell::new(None) };
        static COLOR_TRIGGER: Cell<Option<Point<Pixels>>> = const { Cell::new(None) };
        static COLOR_SWATCH: Cell<Option<Point<Pixels>>> = const { Cell::new(None) };
    }

    pub(super) fn record_language_trigger(point: Point<Pixels>) {
        LANGUAGE_TRIGGER.set(Some(point));
    }

    pub(crate) fn language_trigger() -> Point<Pixels> {
        LANGUAGE_TRIGGER
            .get()
            .expect("Language dropdown trigger was laid out")
    }

    pub(super) fn record_color_trigger(point: Point<Pixels>) {
        COLOR_TRIGGER.set(Some(point));
    }

    pub(super) fn record_swatch(point: Point<Pixels>) {
        COLOR_SWATCH.set(Some(point));
    }

    pub(crate) fn color_trigger() -> Point<Pixels> {
        COLOR_SWATCH.set(None);
        COLOR_TRIGGER
            .get()
            .expect("Codex color trigger was laid out")
    }

    pub(crate) fn color_swatch() -> Point<Pixels> {
        COLOR_SWATCH
            .get()
            .expect("Codex color popover did not open")
    }

    pub(crate) fn is_palette_visible() -> bool {
        COLOR_SWATCH.get().is_some()
    }

    pub(super) fn record_row(provider: Provider, bounds: Bounds<Pixels>) {
        ROWS.with(|rows| {
            let mut values = rows.get();
            values[provider as usize] = Some(bounds);
            rows.set(values);
        });
    }

    pub(crate) fn drag(
        window: &mut Window,
        cx: &mut App,
        source: Provider,
        target: Provider,
        after: bool,
    ) -> Point<Pixels> {
        let rows = ROWS.with(Cell::get);
        let source_bounds = rows[source as usize].expect("Drag source was laid out");
        let target_bounds = rows[target as usize].expect("Drop target was laid out");
        // Start on the centered grip and drop well inside the chosen half.
        let start = source_bounds.origin + point(px(20.), px(20.));
        let end = point(
            target_bounds.center().x,
            if after {
                target_bounds.bottom() - px(8.)
            } else {
                target_bounds.top() + px(8.)
            },
        );
        for position in [start, end] {
            assert!(
                position.y > px(65.) && position.y < window.bounds().size.height - px(25.),
                "Provider row is outside the panel: {position:?}"
            );
        }
        let move_pointer = |position, pressed_button, window: &mut Window, cx: &mut App| {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    modifiers: Default::default(),
                    pressed_button,
                }),
                cx,
            );
        };
        move_pointer(start, None, window, cx);
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                position: start,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        move_pointer(
            start + point(px(6.), px(0.)),
            Some(MouseButton::Left),
            window,
            cx,
        );
        assert!(cx.has_active_drag(), "Provider grip did not start a drag");
        move_pointer(end, Some(MouseButton::Left), window, cx);
        end
    }

    pub(crate) fn release(window: &mut Window, cx: &mut App, position: Point<Pixels>) {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                position,
                button: MouseButton::Left,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
        assert!(!cx.has_active_drag(), "Provider drag was not released");
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
        for (provider, submitted, normalized) in self.saved_labels.drain(..) {
            if let Some((_, input, _, _)) = self.labels.iter().find(|(p, ..)| *p == provider) {
                // A later keystroke is an unfinished edit, not the saved submission.
                if input.read(cx).value().as_ref() == submitted {
                    input.update(cx, |input, cx| input.set_value(normalized, window, cx));
                }
            }
        }
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
