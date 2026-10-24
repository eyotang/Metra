use crate::{i18n::I18n, service::AppPayload, settings::DisplayMode, status_bar};
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuItem, PredefinedMenuItem},
};

pub struct StatusTray {
    icon: TrayIcon,
    last_presentation: String,
}
fn icon() -> tray_icon::Icon {
    #[cfg(target_os = "macos")]
    let bytes = include_bytes!("../assets/icons/trayTemplate.png");
    #[cfg(not(target_os = "macos"))]
    let bytes = include_bytes!("../assets/icons/trayColor.png");
    let image = image::load_from_memory(bytes)
        .expect("embedded tray image")
        .to_rgba8();
    let (width, height) = image.dimensions();
    tray_icon::Icon::from_rgba(image.into_raw(), width, height).expect("valid tray icon")
}
impl StatusTray {
    pub fn new(payload: &AppPayload) -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        gtk::init().map_err(|error| error.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_id("metra-status")
            .with_icon(icon())
            .with_tooltip("Metra")
            .with_menu(Box::new(Menu::new()))
            .with_menu_on_left_click(false)
            .build()
            .map_err(|error| error.to_string())?;
        #[cfg(target_os = "macos")]
        tray.set_icon_as_template(true);
        let mut result = Self {
            icon: tray,
            last_presentation: String::new(),
        };
        result.rebuild_menu(payload)?;
        result.update(payload, 0);
        Ok(result)
    }
    pub fn rebuild_menu(&mut self, payload: &AppPayload) -> Result<(), String> {
        let i18n = I18n::new(payload.settings.ui_language);
        let menu = Menu::new();
        for (id, label) in [
            ("details", "native.details"),
            ("settings", "native.settings"),
            ("refresh", "native.refresh"),
            (
                "display",
                if payload.settings.display_mode == DisplayMode::Bubble {
                    "menu.displayStatusBar"
                } else {
                    "menu.displayBubble"
                },
            ),
        ] {
            menu.append(&MenuItem::with_id(id, i18n.t(label), true, None))
                .map_err(|error| error.to_string())?;
        }
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|error| error.to_string())?;
        menu.append(&MenuItem::with_id("quit", i18n.t("menu.quit"), true, None))
            .map_err(|error| error.to_string())?;
        self.icon.set_menu(Some(Box::new(menu)));
        Ok(())
    }
    pub fn update(&mut self, payload: &AppPayload, cycle: usize) {
        let entries = status_bar::entries(&payload.snapshot, &payload.settings);
        let summary = entries
            .iter()
            .map(status_bar::StatusEntry::headline)
            .collect::<Vec<_>>()
            .join("  |  ");
        let presentation = format!(
            "{:?}:{}:{summary}:{:?}",
            payload.settings.display_mode,
            cycle % entries.len().max(1),
            entries.iter().map(|entry| &entry.color).collect::<Vec<_>>()
        );
        if presentation == self.last_presentation {
            return;
        }
        self.last_presentation = presentation;
        let _ = self.icon.set_tooltip(Some(format!("Metra · {summary}")));
        if payload.settings.display_mode == DisplayMode::StatusBar {
            if let Some(entry) = entries.get(cycle % entries.len().max(1)) {
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                self.icon.set_title(Some(entry.headline()));
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                if let Ok(icon) = tray_icon::Icon::from_rgba(status_bar::usage_icon(entry), 32, 32)
                {
                    let _ = self.icon.set_icon(Some(icon));
                }
            }
        } else {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            self.icon.set_title(Some(""));
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            let _ = self.icon.set_icon(Some(icon()));
        }
    }
    pub fn position(&self) -> Option<(f32, f32)> {
        let rect = self.icon.rect()?;
        Some((rect.position.x as f32, rect.position.y as f32))
    }
}
