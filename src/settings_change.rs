//! Shared settings commands for desktop shells.
use crate::{
    model::Provider,
    settings::{AppSettings, BubblePercentMode, DisplayMode, UiLanguage},
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Change {
    Display(DisplayMode),
    Percent(BubblePercentMode),
    Refresh(u64),
    Language(UiLanguage),
    Snap(bool),
    Autostart(bool),
    Compat(bool),
    Visible { provider: Provider, visible: bool },
    Label { provider: Provider, label: String },
    Color { provider: Provider, color: String },
    Order(Vec<Provider>),
}
impl Change {
    /// Reject changes that would make every provider inaccessible in the bubble.
    pub fn validate(&self, settings: &AppSettings) -> Result<(), String> {
        if let Self::Visible {
            provider,
            visible: false,
        } = self
            && settings.bubble_visible_providers.len() == 1
            && settings.bubble_visible_providers.contains(provider)
        {
            return Err(crate::i18n::I18n::new(settings.ui_language).t("config.keepOne"));
        }
        Ok(())
    }

    pub fn apply(&self, settings: &mut AppSettings) {
        match self {
            Self::Display(v) => settings.display_mode = *v,
            Self::Percent(v) => settings.bubble_percent_mode = *v,
            Self::Refresh(v) => settings.refresh_minutes = *v,
            Self::Language(v) => settings.ui_language = *v,
            Self::Snap(v) => settings.bubble_snap_enabled = *v,
            Self::Autostart(v) => settings.autostart = *v,
            Self::Compat(v) => settings.cursor_compat_enabled = *v,
            Self::Visible { provider, visible } => {
                settings.bubble_visible_providers.retain(|p| p != provider);
                if *visible {
                    settings.bubble_visible_providers.push(*provider);
                }
            }
            Self::Order(order) => settings.bubble_provider_order = order.clone(),
            Self::Label { provider, label } => {
                *match provider {
                    Provider::Cursor => &mut settings.cursor_bubble_label,
                    Provider::Codex => &mut settings.codex_bubble_label,
                    Provider::Claude => &mut settings.claude_bubble_label,
                } = label.clone()
            }
            Self::Color { provider, color } => {
                *match provider {
                    Provider::Cursor => &mut settings.cursor_bubble_color,
                    Provider::Codex => &mut settings.codex_bubble_color,
                    Provider::Claude => &mut settings.claude_bubble_color,
                } = color.clone()
            }
        }
    }
}
