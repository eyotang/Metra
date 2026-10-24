use gpui_kit::component::{
    IconName, Selectable, Sizable, Theme, ThemeMode,
    button::{Button, ButtonVariants},
};
use gpui_kit::{self as gpui, *};
use std::borrow::Cow;

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path == "metra-logo.png" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/128x128.png"
            ))));
        }
        gpui::assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        gpui::assets::Assets.list(path)
    }
}
pub fn init(cx: &mut App) {
    let _ = cx
        .text_system()
        .add_fonts(vec![Cow::Borrowed(include_bytes!(
            "../assets/fonts/InterVariable.ttf"
        ))]);
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
        theme.font_family = "Inter".into();
        theme.font_size = px(12.);
        theme.radius = px(9.);
        theme.radius_lg = px(20.);
        theme.shadow = false;
        let colors = &mut theme.colors;
        colors.background = rgb(0x151925).into();
        colors.foreground = rgb(0xe6eaf5).into();
        colors.border = rgb(0x303649).into();
        colors.button = rgb(0x1c2030).into();
        colors.button_foreground = rgb(0xa4aec5).into();
        colors.button_hover = rgb(0x262b3e).into();
        colors.button_active = rgb(0x302841).into();
        colors.primary = rgb(0x8c78ee).into();
        colors.primary_hover = rgb(0x9b8af5).into();
        colors.primary_active = rgb(0x7563d2).into();
        colors.primary_foreground = rgb(0xffffff).into();
        colors.secondary = rgb(0x242938).into();
        colors.secondary_foreground = rgb(0xcbd2e5).into();
        colors.accent = rgb(0x29253d).into();
        colors.accent_foreground = rgb(0xd3caff).into();
        colors.muted = rgb(0x252a3b).into();
        colors.muted_foreground = rgb(0x77829e).into();
        colors.input = rgb(0x181d2b).into();
        colors.ring = rgb(0x8d7ce0).into();
        colors.selection = rgba(0x8c78ee55).into();
        colors.switch_thumb = rgb(0xe8e5ff).into();
        colors.popover = rgb(0x191d2b).into();
        colors.popover_foreground = rgb(0xe6eaf5).into();
    });
}
pub fn segment(id: impl Into<ElementId>, label: impl Into<SharedString>, selected: bool) -> Button {
    Button::new(id)
        .small()
        .h(px(30.))
        .flex_1()
        .rounded(px(9.))
        .label(label)
        .selected(selected)
        .toggled(selected)
        .bg(if selected {
            rgba(0x8067ff25)
        } else {
            rgba(0xffffff06)
        })
        .border_color(if selected {
            rgba(0x917bff66)
        } else {
            rgba(0xffffff0b)
        })
        .text_color(if selected {
            rgb(0xd1c6ff)
        } else {
            rgb(0x858fa8)
        })
}
pub fn icon_button(
    id: impl Into<ElementId>,
    icon: IconName,
    label: impl Into<SharedString>,
) -> Button {
    let label = label.into();
    Button::new(id)
        .ghost()
        .small()
        .size(px(26.))
        .icon(icon)
        .accessibility_label(label.clone())
        .tooltip(label)
        .text_color(rgb(0x929cb5))
        .rounded(px(8.))
}
pub fn section_title(text: String) -> Div {
    div()
        .text_size(px(9.))
        .text_color(rgb(0x69748f))
        .mb(px(7.))
        .child(text)
}
pub fn setting_row(label: String, hint: Option<String>, control: impl IntoElement) -> Div {
    let mut text = div().flex().flex_col().gap(px(3.)).flex_1().child(
        div()
            .text_size(px(11.))
            .text_color(rgb(0xd0d6e6))
            .child(label),
    );
    if let Some(hint) = hint {
        text = text.child(
            div()
                .text_size(px(8.5))
                .text_color(rgb(0x6d7790))
                .child(hint),
        );
    }
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .min_h(px(40.))
        .px(px(7.))
        .child(text)
        .child(control)
}
pub fn brand(subtitle: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(9.))
        .child(img("metra-logo.png").size(px(28.)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(13.))
                        .text_color(rgb(0xf1f2fc))
                        .child("Metra"),
                )
                .child(
                    div()
                        .text_size(px(8.))
                        .text_color(rgb(0x77829b))
                        .child(subtitle),
                ),
        )
}
