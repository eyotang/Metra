use super::*;
use metra_lib::status_bar::{StatusEntry, entries};

pub(super) struct Bubble {
    entries: Vec<StatusEntry>,
    _subscription: Subscription,
    pointer: Option<Point<Pixels>>,
    dragging: bool,
}
impl Bubble {
    fn new(dashboard: Entity<Dashboard>, cx: &mut Context<Self>) -> Self {
        let data = dashboard.read(cx).service.payload();
        let subscription = cx.observe(&dashboard, |this, view, cx| {
            let data = view.read(cx).service.payload();
            let next = entries(&data.snapshot, &data.settings);
            if next != this.entries {
                this.entries = next;
                cx.notify();
            }
        });
        Self {
            entries: entries(&data.snapshot, &data.settings),
            _subscription: subscription,
            pointer: None,
            dragging: false,
        }
    }
}
impl Render for Bubble {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div()
            .id("bubble")
            .h_flex()
            .items_center()
            .justify_center()
            .gap_2()
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
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, _| {
                if event.dragging() && !this.dragging {
                    if let Some(start) = this.pointer {
                        let dx = f32::from(event.position.x - start.x);
                        let dy = f32::from(event.position.y - start.y);
                        if dx.hypot(dy) >= 4. {
                            this.dragging = true;
                            window.start_window_move();
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
            .on_mouse_down(MouseButton::Right, |_, _, cx| {
                runtime::open_panel(Page::Settings, cx)
            });
        for entry in &self.entries {
            let color =
                u32::from_str_radix(entry.color.trim_start_matches('#'), 16).unwrap_or(0x9c83ff);
            content = content.child(
                div()
                    .v_flex()
                    .items_center()
                    .text_color(rgb(color))
                    .child(entry.label.clone())
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
    let bounds = Bounds::centered(None, size(px(190.), px(56.)), cx);
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
        move |_, cx| cx.new(|cx| Bubble::new(dashboard, cx)),
    )
    .map(|(handle, _)| handle)
    .map_err(|e| e.to_string())
}
