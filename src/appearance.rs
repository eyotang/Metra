use gpui_kit::component::{
    IconName, Selectable, Sizable, Theme, ThemeMode,
    button::{Button, ButtonVariants},
};
use gpui_kit::{self as gpui, *};
use std::borrow::Cow;

gpui::assets::icon_assets!(ControlIcons, [GripVertical]);

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if path == "metra-logo.png" {
            return Ok(Some(Cow::Borrowed(include_bytes!(
                "../assets/icons/128x128.png"
            ))));
        }
        if let Some(icon) = ControlIcons.load(path)? {
            return Ok(Some(icon));
        }
        gpui::assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        let mut assets = gpui::assets::Assets.list(path)?;
        assets.extend(ControlIcons.list(path)?);
        Ok(assets)
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
        colors.background = rgba(0x151c2bcc).into();
        colors.foreground = rgb(0xe6eaf5).into();
        colors.border = rgba(0xffffff26).into();
        colors.button = rgba(0xffffff0c).into();
        colors.button_foreground = rgb(0xa4aec5).into();
        colors.button_hover = rgba(0xffffff1a).into();
        colors.button_active = rgba(0x8c78ee30).into();
        colors.primary = rgb(0x8c78ee).into();
        colors.primary_hover = rgb(0x9b8af5).into();
        colors.primary_active = rgb(0x7563d2).into();
        colors.primary_foreground = rgb(0xffffff).into();
        colors.secondary = rgba(0xffffff0c).into();
        colors.secondary_foreground = rgb(0xcbd2e5).into();
        colors.accent = rgba(0x8c78ee28).into();
        colors.accent_foreground = rgb(0xd3caff).into();
        colors.muted = rgba(0xffffff0a).into();
        colors.muted_foreground = rgb(0xa6b0c6).into();
        colors.input = rgba(0x10172855).into();
        colors.ring = rgb(0x8d7ce0).into();
        colors.selection = rgba(0x8c78ee55).into();
        colors.switch_thumb = rgb(0xe8e5ff).into();
        colors.popover = rgba(0x151c2bb3).into();
        colors.popover_foreground = rgb(0xe6eaf5).into();
        colors.list_hover = rgba(0xffffff10).into();
        colors.list_active = rgba(0x8c78ee30).into();
        colors.group_box = rgba(0xffffff08).into();
        colors.tab_bar = transparent_black();
        colors.tab_bar_segmented = rgba(0xffffff08).into();
        colors.scrollbar = transparent_black();
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
            rgb(0xa6b0c6)
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
        .text_color(rgb(0xa1acc3))
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
                .text_color(rgb(0xa1acc3))
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
                        .text_color(rgb(0xa1acc3))
                        .child(subtitle),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::Assets;
    use gpui_kit::AssetSource;

    #[test]
    fn provider_drag_handle_icon_is_embedded() {
        let path = gpui_kit::assets::IconName::GripVertical.path();
        assert!(Assets.load(&path).unwrap().is_some());
    }
}
