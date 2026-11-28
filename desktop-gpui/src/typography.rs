use gpui_kit::*;
use std::borrow::Cow;

pub(super) const UI_FONT_FAMILY: &str = "Metra Inter";
pub(super) const BODY_WEIGHT: FontWeight = FontWeight(if cfg!(windows) { 550. } else { 400. });
pub(super) const CONTROL_WEIGHT: FontWeight = FontWeight(if cfg!(windows) { 650. } else { 500. });
pub(super) const VALUE_WEIGHT: FontWeight = FontWeight(if cfg!(windows) { 650. } else { 550. });
pub(super) const HEADING_WEIGHT: FontWeight = FontWeight(if cfg!(windows) { 700. } else { 650. });
pub(super) const BADGE_WEIGHT: FontWeight = FontWeight(if cfg!(windows) { 850. } else { 750. });

pub(super) fn ui_font(locale: &str) -> Font {
    let mut face = font(UI_FONT_FAMILY);
    face.weight = BODY_WEIGHT;
    if cfg!(windows) {
        // Inter has no CJK glyphs. Match the UI language even when the
        // DirectWrite system fallback uses a different Windows language.
        let family = match locale {
            "zh-CN" => "Microsoft YaHei UI",
            "ja" => "Yu Gothic UI",
            "ko" => "Malgun Gothic",
            _ => return face,
        };
        face.fallbacks = Some(FontFallbacks::from_fonts(vec![family.to_owned()]));
    }
    face
}

pub(super) fn load_fonts(cx: &App) -> Result<(), String> {
    // Variable font axes cannot be relied on across GPUI's text backends.
    let faces: &[&[u8]] = &[
        include_bytes!("../../assets/fonts/MetraInter-400.ttf"),
        include_bytes!("../../assets/fonts/MetraInter-500.ttf"),
        include_bytes!("../../assets/fonts/MetraInter-550.ttf"),
        include_bytes!("../../assets/fonts/MetraInter-650.ttf"),
        include_bytes!("../../assets/fonts/MetraInter-700.ttf"),
        include_bytes!("../../assets/fonts/MetraInter-750.ttf"),
        #[cfg(windows)]
        include_bytes!("../../assets/fonts/MetraInter-850.ttf"),
    ];
    cx.text_system()
        .add_fonts(faces.iter().copied().map(Cow::Borrowed).collect())
        .map_err(|error| error.to_string())
}

#[cfg(feature = "native-smoke")]
pub(super) fn verify_fonts(cx: &App) {
    let text = cx.text_system();
    let mut previous_width = px(0.);
    for weight in [
        400.,
        500.,
        550.,
        650.,
        700.,
        750.,
        #[cfg(windows)]
        850.,
    ] {
        let mut request = font(UI_FONT_FAMILY);
        request.weight = FontWeight(weight);
        let id = text.resolve_font(&request);
        assert!(
            text.get_font_for_id(id)
                .is_some_and(|face| face.family == UI_FONT_FAMILY),
            "bundled font fell back at weight {weight}"
        );
        // The advance of Inter's 'm' increases with weight. Shape the glyph:
        // two request IDs can still resolve to the same physical font face.
        let width = text.layout_width(id, px(256.), 'm');
        assert!(width > previous_width, "missing bundled weight {weight}");
        previous_width = width;
    }
}
