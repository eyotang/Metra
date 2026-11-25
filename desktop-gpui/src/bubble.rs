use super::*;
use metra_lib::status_bar::{StatusEntry, entries};

pub(super) struct Bubble {
    entries: Vec<StatusEntry>,
    _subscription: Subscription,
    pointer: Option<Point<Pixels>>,
    dragging: bool,
    service: Arc<RefreshService>,
    peek: bool,
    snap: bool,
    pending: Option<Task<()>>,
}
impl Bubble {
    fn new(dashboard: Entity<Dashboard>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let data = dashboard.read(cx).service.payload();
        let subscription = cx.observe(&dashboard, |this, view, cx| {
            let data = view.read(cx).service.payload();
            let next = entries(&data.snapshot, &data.settings);
            if next != this.entries || this.snap != data.settings.bubble_snap_enabled {
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
            snap: data.settings.bubble_snap_enabled,
            pending: None,
        };
        let result = (|| {
            let (work, scale) = native_window::work_area(window, cx)?;
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
            view.dock(view.snap, true, window, cx)
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
                let service = self.service.clone();
                cx.background_executor()
                    .spawn(async move {
                        if let Err(error) = service.update_settings(|s| {
                            s.bubble_position = Some(position);
                            s.bubble_position_version = 3;
                        }) {
                            eprintln!("{error}");
                        }
                    })
                    .detach();
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
                this.dragging = false;
                this.pointer = None;
                if this.service.payload().settings.bubble_snap_enabled {
                    if let Err(error) = this.dock(true, true, window, cx) {
                        eprintln!("{error}");
                    }
                } else {
                    this.persist(window, cx);
                }
            });
        }));
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
        let mut content = div()
            .id("bubble")
            .v_flex()
            .items_center()
            .justify_center()
            .gap_0()
            .text_xs()
            .size_full()
            .rounded_full()
            .bg(rgb(0x171b26))
            .text_color(rgb(0xf0f2f7))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, _| {
                    this.pointer = Some(event.position);
                    this.dragging = false;
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if event.dragging() && !this.dragging {
                    if let Some(start) = this.pointer {
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
                cx.listener(|this, _, _, cx| {
                    if !this.dragging && this.pointer.take().is_some() {
                        runtime::open_panel(Page::Details, cx);
                    }
                    this.pointer = None;
                }),
            )
            .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
                if this.dragging {
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
                runtime::open_panel(Page::Settings, cx)
            });
        for entry in &self.entries {
            let color =
                u32::from_str_radix(entry.color.trim_start_matches('#'), 16).unwrap_or(0x9c83ff);
            content = content.child(
                div()
                    .h_flex()
                    .items_center()
                    .text_color(rgb(color))
                    .when(!self.peek, |row| row.child(entry.label.clone()))
                    .child(
                        entry
                            .value
                            .map(|v| format!("{v}%"))
                            .unwrap_or_else(|| "—".into()),
                    ),
            );
        }
        content
    }
}
pub(super) fn open(cx: &mut App) -> Result<AnyWindowHandle, String> {
    let dashboard = cx.global::<runtime::Runtime>().dashboard.clone();
    let bounds = Bounds::centered(None, size(px(56.), px(56.)), cx);
    gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: false,
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
