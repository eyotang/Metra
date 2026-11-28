use gpui_kit::*;
use std::borrow::Cow;

pub(super) const UI_FONT_FAMILY: &str = "Metra Inter";
pub(super) const HEADING_WEIGHT: FontWeight = FontWeight(650.);
pub(super) const BADGE_WEIGHT: FontWeight = FontWeight(750.);

pub(super) fn load_fonts(cx: &App) -> Result<(), String> {
    // Variable font axes cannot be relied on across GPUI's text backends.
    let faces: [&[u8]; 6] = [
        include_bytes!("../../assets/fonts/MetraInter-400.ttf"),
        include_bytes!("../../assets/fonts/MetraInter-500.ttf"),
        include_bytes!("../../assets/fonts/MetraInter-550.ttf"),
        include_bytes!("../../assets/fonts/MetraInter-650.ttf"),
        include_bytes!("../../assets/fonts/MetraInter-700.ttf"),
        include_bytes!("../../assets/fonts/MetraInter-750.ttf"),
    ];
    cx.text_system()
        .add_fonts(faces.into_iter().map(Cow::Borrowed).collect())
        .map_err(|error| error.to_string())
}

#[cfg(feature = "native-smoke")]
pub(super) fn verify_fonts(cx: &App) {
    let text = cx.text_system();
    let mut faces = Vec::new();
    for weight in [400., 500., 550., 650., 700., 750.] {
        let mut request = font(UI_FONT_FAMILY);
        request.weight = FontWeight(weight);
        let id = text.resolve_font(&request);
        assert!(
            text.get_font_for_id(id)
                .is_some_and(|face| face.family == UI_FONT_FAMILY),
            "bundled font fell back at weight {weight}"
        );
        assert!(!faces.contains(&id), "missing bundled weight {weight}");
        faces.push(id);
    }
}
