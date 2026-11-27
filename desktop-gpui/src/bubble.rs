use super::*;
use metra_lib::status_bar::{StatusEntry, entries};

#[cfg(feature = "native-smoke")]
pub(super) static LAST_RENDERED_USAGE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(255);

enum BubblePress {
    At(Point<Pixels>),
    Resized,
}

pub(super) struct Bubble {
    entries: Vec<StatusEntry>,
    _subscription: Subscription,
    pointer: Option<BubblePress>,
    dragging: bool,
    service: Arc<RefreshService>,
    peek: bool,
    focus: FocusHandle,
    snap: bool,
    percent_mode: metra_lib::settings::BubblePercentMode,
    locale: &'static str,
    drag_finished: Option<std::time::Instant>,
    pending: Option<Task<()>>,
}
impl Bubble {
    fn new(dashboard: Entity<Dashboard>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let data = dashboard.read(cx).service.payload();
        let subscription = cx.observe(&dashboard, |this, view, cx| {
            let data = view.read(cx).service.payload();
            let next = entries(&data.snapshot, &data.settings);
            let locale = I18n::new(data.settings.ui_language).locale;
            if next != this.entries
                || this.snap != data.settings.bubble_snap_enabled
                || this.percent_mode != data.settings.bubble_percent_mode
                || this.locale != locale
            {
                this.percent_mode = data.settings.bubble_percent_mode;
                this.locale = locale;
                this.entries = next;
                cx.notify();
            }
        });
        let mut view = Self {
            entries: entries(&data.snapshot, &data.settings),
            _subscription: subscription,
            pointer: None,
            dragging: false,
            service: dashboard.read(cx).service.clone(),
            peek: false,
            focus: cx.focus_handle(),
            snap: data.settings.bubble_snap_enabled,
            percent_mode: data.settings.bubble_percent_mode,
            locale: I18n::new(data.settings.ui_language).locale,
            drag_finished: None,
            pending: None,
        };
        let result = (|| {
            let (work, scale) = native_window::initial_work_area(window, cx, &data.settings)?;
            let (x, y) = data
                .settings
                .bubble_position
                .map(|p| {
                    metra_lib::geometry::saved_physical(
                        p,
                        data.settings.bubble_position_version,
                        scale,
                    )
                })
                .unwrap_or((
                    work.x + work.width - 64. * scale,
                    work.y + work.height - 64. * scale,
                ));
            let (x, y) = work.clamp(x, y, 56. * scale, 56. * scale);
            native_window::move_to(window, x, y)?;
            view.dock(view.snap, true, window, cx)?;
            if !view.snap {
                view.persist(window, cx);
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            eprintln!("{error}");
        }
        view
    }
    fn dock(
        &mut self,
        peek: bool,
        force: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.dragging {
            return Ok(());
        }
        let (work, scale) = native_window::work_area(window, cx)?;
        let (x, y) = native_window::position(window)?;
        if peek {
            if !self.service.payload().settings.bubble_snap_enabled {
                return Ok(());
            }
            let near = (x as f64 - work.x).abs() < 12. * scale
                || (x as f64 + 56. * scale - work.x - work.width).abs() < 12. * scale;
            if !force && !near {
                return Ok(());
            }
        } else if !self.peek {
            return Ok(());
        }
        let (x, y, _) = work.dock(x as f64, y as f64, scale, peek);
        window.resize(size(px(if peek { 32. } else { 56. }), px(56.)));
        native_window::move_to(window, x, y)?;
        self.peek = peek;
        if self.pointer.is_some() {
            // The press used coordinates relative to the previous window origin.
            self.pointer = Some(BubblePress::Resized);
        }
        self.persist(window, cx);
        cx.notify();
        Ok(())
    }
    fn persist(&self, window: &Window, cx: &mut Context<Self>) {
        let result = (|| {
            let (x, y) = native_window::position(window)?;
            let (work, scale) = native_window::work_area(window, cx)?;
            let (x, y) = if self.peek {
                let (x, y, _) = work.dock(x as f64, y as f64, scale, false);
                (x, y)
            } else {
                (x, y)
            };
            Ok::<_, String>(metra_lib::settings::SavedPosition { x, y })
        })();
        match result {
            Ok(position) => {
                if let Err(error) = cx.global::<runtime::Runtime>().positions.try_send(position) {
                    eprintln!("{error}");
                }
            }
            Err(error) => eprintln!("{error}"),
        }
    }
    fn finish_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending = Some(cx.spawn_in(window, async move |this, cx| {
            for _ in 0..6000 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(20))
                    .await;
                if !platform::primary_button_pressed().unwrap_or(false) {
                    break;
                }
            }
            let _ = this.update_in(cx, |this, window, cx| {
                this.complete_drag(window, cx);
            });
        }));
    }

    fn complete_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dragging = false;
        self.drag_finished = Some(std::time::Instant::now());
        self.pointer = None;
        if self.service.payload().settings.bubble_snap_enabled {
            if let Err(error) = self.dock(true, true, window, cx) {
                eprintln!("{error}");
            }
        } else {
            if let Err(error) = native_window::clamp_to_work_area(window, cx) {
                eprintln!("{error}");
            }
            self.persist(window, cx);
        }
    }
}
impl Render for Bubble {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self.service.payload().settings.bubble_snap_enabled;
        if snap != self.snap {
            self.snap = snap;
            if let Err(error) = self.dock(snap, true, window, cx) {
                eprintln!("{error}");
            }
        }
        let t = I18n {
            locale: self.locale,
        };
        let usage = self
            .entries
            .iter()
            .map(|entry| {
                format!(
                    "{} {}",
                    entry.label,
                    entry
                        .value
                        .map(|v| format!("{v}%"))
                        .unwrap_or_else(|| "—".into())
                )
            })
            .collect::<Vec<_>>()
            .join(&t.t("bubble.usageSeparator"));
        let accessible = t.format(
            "bubble.ariaLabel",
            &[
                ("usage", usage),
                (
                    "dragHint",
                    t.t(if snap {
                        "bubble.snapHint"
                    } else {
                        "bubble.freeHint"
                    }),
                ),
            ],
        );
        let ring = linear_gradient(
            138.,
            linear_color_stop(rgb(0x8d72ff), 0.),
            linear_color_stop(rgb(0x28d1b3), 1.),
        );
        let hover_ring = linear_gradient(
            138.,
            linear_color_stop(rgb(0xa18aff), 0.),
            linear_color_stop(rgb(0x47e3c7), 1.),
        );
        let content = div()
            .id("bubble")
            .role(Role::Button)
            .aria_label(accessible)
            .track_focus(&self.focus)
            .tab_stop(true)
            .on_key_down(|event, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.defer(runtime::toggle_panel);
                }
            })
            .v_flex()
            .items_center()
            .justify_center()
            .gap_0()
            .size_full()
            .rounded_full()
            .p(px(3.))
            .cursor_pointer()
            .bg(ring)
            .map(|content| {
                if cfg!(target_os = "linux") {
                    content.when(window.is_window_hovered(), |content| content.bg(hover_ring))
                } else {
                    content.hover(move |style| style.bg(hover_ring))
                }
            })
            .text_color(rgb(0xf0f2f7))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.pointer = Some(BubblePress::At(event.position));
                    this.dragging = false;
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if event.dragging() && !this.dragging {
                    if let Some(BubblePress::Resized) = this.pointer {
                        this.pointer = Some(BubblePress::At(event.position));
                    } else if let Some(BubblePress::At(start)) = this.pointer {
                        let dx = f32::from(event.position.x - start.x);
                        let dy = f32::from(event.position.y - start.y);
                        if dx.hypot(dy) >= 4. {
                            let _ = this.dock(false, false, window, cx);
                            this.dragging = true;
                            window.start_window_move();
                            this.finish_drag(window, cx);
                        }
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.pointer = None;
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.pointer = None),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if !this.dragging
                    && this
                        .drag_finished
                        .is_none_or(|time| time.elapsed() > std::time::Duration::from_millis(300))
                {
                    cx.defer(runtime::toggle_panel);
                }
            }))
            .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
                // X11 may keep the last in-window pointer coordinates after a leave.
                if this.dragging
                    || (*hovered && cfg!(target_os = "linux") && !window.is_window_hovered())
                {
                    return;
                }
                this.pending = None;
                if *hovered {
                    if let Err(error) = this.dock(false, false, window, cx) {
                        eprintln!("{error}");
                    }
                } else {
                    this.pending = Some(cx.spawn_in(window, async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(500))
                            .await;
                        let _ = this.update_in(cx, |this, window, cx| {
                            if let Err(error) = this.dock(true, false, window, cx) {
                                eprintln!("{error}");
                            }
                        });
                    }));
                }
            }))
            .on_mouse_down(MouseButton::Right, |_, _, cx| {
                cx.defer(|cx| runtime::open_panel(Page::Settings, cx))
            });
        let mut rows = div()
            .v_flex()
            .size_full()
            .justify_center()
            .rounded_full()
            .bg(rgb(0x121a2b))
            .font_family("Inter Variable")
            .font_weight(FontWeight::EXTRA_BOLD)
            .line_height(relative(1.))
            .gap(px(1.))
            .px(px(if self.peek { 0.5 } else { 3.5 }));
        #[cfg(feature = "native-smoke")]
        LAST_RENDERED_USAGE.store(
            self.entries
                .first()
                .and_then(|entry| entry.value)
                .unwrap_or(255),
            std::sync::atomic::Ordering::SeqCst,
        );
        for entry in &self.entries {
            let accent =
                u32::from_str_radix(entry.color.trim_start_matches('#'), 16).unwrap_or(0x9c83ff);
            let value_color = quota_color(entry.value, self.percent_mode, accent);
            // A fourth glyph only fits in the capsule at a slightly smaller size.
            let value_size = if self.peek && entry.value == Some(100) {
                8.5
            } else {
                10.
            };
            rows = rows.child(
                div()
                    .h_flex()
                    .items_center()
                    .h(px(12.))
                    .w_full()
                    .gap(px(2.))
                    .when(self.peek, |row| row.justify_center())
                    .when(!self.peek, |row| row.justify_between())
                    .when(entry.stale, |row| row.opacity(0.55))
                    .when(!self.peek, |row| {
                        row.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .min_w(px(12.))
                                .max_w(px(15.))
                                .h(px(10.))
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .rounded(px(3.))
                                .text_size(px(7.5))
                                .text_color(rgb(accent))
                                .bg(rgba((accent << 8) | 46))
                                .child(entry.label.clone()),
                        )
                    })
                    .child(
                        div()
                            .flex_shrink_0()
                            .whitespace_nowrap()
                            .text_size(px(value_size))
                            .text_color(rgb(value_color))
                            .child(
                                entry
                                    .value
                                    .map(|v| format!("{v}%"))
                                    .unwrap_or_else(|| "—".into()),
                            ),
                    ),
            );
        }
        content.child(rows)
    }
}
pub(super) fn open(cx: &mut App) -> Result<AnyWindowHandle, String> {
    open_with_visibility(cx, true)
}

pub(super) fn initialize_position(cx: &mut App) -> Result<(), String> {
    let window = open_with_visibility(cx, false)?;
    window
        .update(cx, |_, window, _| window.remove_window())
        .map_err(|e| e.to_string())
}

fn open_with_visibility(cx: &mut App, show: bool) -> Result<AnyWindowHandle, String> {
    let dashboard = cx.global::<runtime::Runtime>().dashboard.clone();
    let bounds = Bounds::centered(None, size(px(56.), px(56.)), cx);
    open_overlay(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: false,
            show,
            kind: WindowKind::PopUp,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            app_id: Some("dev.metra.desktop".into()),
            ..Default::default()
        },
        cx,
        move |window, cx| cx.new(|cx| Bubble::new(dashboard, window, cx)),
    )
    .map(|(handle, _)| handle)
    .map_err(|e| e.to_string())
}

fn quota_color(
    value: Option<u8>,
    mode: metra_lib::settings::BubblePercentMode,
    accent: u32,
) -> u32 {
    let remaining = value.map(|v| {
        if mode == metra_lib::settings::BubblePercentMode::Used {
            100u8.saturating_sub(v)
        } else {
            v
        }
    });
    match remaining {
        Some(0..=10) => 0xff667d,
        Some(11..=25) => 0xffd15a,
        _ => accent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn warning_colors_follow_remaining_quota_in_both_display_modes() {
        use metra_lib::settings::BubblePercentMode::{Remaining, Used};
        for remaining in 0..=100 {
            let expected = if remaining <= 10 {
                0xff667d
            } else if remaining <= 25 {
                0xffd15a
            } else {
                0x123456
            };
            assert_eq!(quota_color(Some(remaining), Remaining, 0x123456), expected);
            assert_eq!(quota_color(Some(100 - remaining), Used, 0x123456), expected);
        }
        assert_eq!(quota_color(None, Remaining, 0x123456), 0x123456);
        assert_eq!(quota_color(None, Used, 0x123456), 0x123456);
    }
}
