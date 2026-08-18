use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::model::Provider;

pub const REFRESH_INTERVALS: [u64; 5] = [1, 5, 15, 30, 60];
pub const DEFAULT_CURSOR_BUBBLE_COLOR: &str = "#9c83ff";
pub const DEFAULT_CODEX_BUBBLE_COLOR: &str = "#4bd8c0";
pub const DEFAULT_CLAUDE_BUBBLE_COLOR: &str = "#e99068";
pub const BUBBLE_COLOR_PALETTE: [&str; 55] = [
    "#ff6b6b", "#e99068", "#ffc21a", "#91b800", "#34b84a", "#4bd8c0", "#2da9dc",
    "#7698ee", "#d86bb3", "#9c83ff", "#949aa4", "#ffe0df", "#ffe3c7", "#fff0c2",
    "#e4f3ad", "#cceecd", "#c5f0eb", "#caebf7", "#dce6fb", "#f4d9e9", "#e8def9",
    "#e8eaed", "#ffb5b0", "#ffc98f", "#ffe08a", "#c9e45f", "#92da96", "#72d8cc",
    "#80d1ed", "#b4c8f6", "#efb2d7", "#d2bdf2", "#cbd0d6", "#ef4e48", "#eb6f17",
    "#dfa20a", "#749900", "#2fa43d", "#119b88", "#158eb9", "#487bea", "#c23f91",
    "#8752df", "#656c76", "#cf332d", "#a84d08", "#8f6508", "#496600", "#208c2b",
    "#087164", "#0b6787", "#2456d9", "#98246e", "#6d2bd1", "#3f454d",
];
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BubblePercentMode {
    Used,
    #[default]
    Remaining,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub refresh_minutes: u64,
    pub autostart: bool,
    pub cursor_compat_enabled: bool,
    pub bubble_percent_mode: BubblePercentMode,
    pub bubble_position: Option<SavedPosition>,
    pub bubble_position_version: u8,
    pub bubble_provider_order: Vec<Provider>,
    pub bubble_visible_providers: Vec<Provider>,
    pub cursor_bubble_label: String,
    pub codex_bubble_label: String,
    pub claude_bubble_label: String,
    pub cursor_bubble_color: String,
    pub codex_bubble_color: String,
    pub claude_bubble_color: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            refresh_minutes: 5,
            autostart: false,
            cursor_compat_enabled: false,
            bubble_percent_mode: BubblePercentMode::Remaining,
            bubble_position: None,
            bubble_position_version: 0,
            bubble_provider_order: vec![Provider::Cursor, Provider::Codex, Provider::Claude],
            bubble_visible_providers: vec![
                Provider::Cursor,
                Provider::Codex,
                Provider::Claude,
            ],
            cursor_bubble_label: "C".into(),
            codex_bubble_label: "X".into(),
            claude_bubble_label: "A".into(),
            cursor_bubble_color: DEFAULT_CURSOR_BUBBLE_COLOR.into(),
            codex_bubble_color: DEFAULT_CODEX_BUBBLE_COLOR.into(),
            claude_bubble_color: DEFAULT_CLAUDE_BUBBLE_COLOR.into(),
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        if !REFRESH_INTERVALS.contains(&self.refresh_minutes) {
            self.refresh_minutes = 5;
        }
        let mut normalized_order = Vec::with_capacity(3);
        for provider in self.bubble_provider_order {
            if !normalized_order.contains(&provider) {
                normalized_order.push(provider);
            }
        }
        for provider in [Provider::Cursor, Provider::Codex, Provider::Claude] {
            if !normalized_order.contains(&provider) {
                normalized_order.push(provider);
            }
        }
        self.bubble_provider_order = normalized_order;
        let mut normalized_visible = Vec::with_capacity(3);
        for provider in self.bubble_visible_providers {
            if !normalized_visible.contains(&provider) {
                normalized_visible.push(provider);
            }
        }
        if normalized_visible.is_empty() {
            normalized_visible = vec![Provider::Cursor, Provider::Codex, Provider::Claude];
        }
        self.bubble_visible_providers = normalized_visible;
        self.cursor_bubble_label = normalize_bubble_label(&self.cursor_bubble_label, "C");
        self.codex_bubble_label = normalize_bubble_label(&self.codex_bubble_label, "X");
        self.claude_bubble_label = normalize_bubble_label(&self.claude_bubble_label, "A");
        self.cursor_bubble_color = normalize_bubble_color(
            &self.cursor_bubble_color,
            DEFAULT_CURSOR_BUBBLE_COLOR,
        );
        self.codex_bubble_color =
            normalize_bubble_color(&self.codex_bubble_color, DEFAULT_CODEX_BUBBLE_COLOR);
        self.claude_bubble_color = normalize_bubble_color(
            &self.claude_bubble_color,
            DEFAULT_CLAUDE_BUBBLE_COLOR,
        );
        self
    }
}

fn normalize_bubble_label(value: &str, fallback: &str) -> String {
    let value = value.trim().chars().take(3).collect::<String>();
    if value.is_empty() {
        fallback.into()
    } else {
        value
    }
}

fn normalize_bubble_color(value: &str, fallback: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    if BUBBLE_COLOR_PALETTE.contains(&value.as_str()) {
        value
    } else {
        fallback.into()
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> AppSettings {
        if let Ok(Some(settings)) = self.load_from_database() {
            return settings.normalized();
        }

        let legacy_settings = fs::read_to_string(self.legacy_json_path())
            .ok()
            .and_then(|raw| serde_json::from_str::<AppSettings>(&raw).ok())
            .map(AppSettings::normalized);
        if let Some(settings) = legacy_settings {
            let _ = self.save(&settings);
            return settings;
        }

        AppSettings::default()
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| "无法创建配置目录".to_string())?;
        }
        let connection = self.open_database()?;
        let json = serde_json::to_string(&settings.clone().normalized())
            .map_err(|_| "无法序列化配置".to_string())?;
        connection
            .execute(
                "INSERT INTO app_settings (id, settings_json) VALUES (1, ?1)\n                 ON CONFLICT(id) DO UPDATE SET settings_json = excluded.settings_json",
                params![json],
            )
            .map_err(|_| "无法保存配置".to_string())?;
        Ok(())
    }

    fn legacy_json_path(&self) -> PathBuf {
        self.path.with_file_name("settings.json")
    }

    fn open_database(&self) -> Result<Connection, String> {
        let connection =
            Connection::open(&self.path).map_err(|_| "无法打开配置数据库".to_string())?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS app_settings (\n                    id INTEGER PRIMARY KEY CHECK (id = 1),\n                    settings_json TEXT NOT NULL\n                );",
            )
            .map_err(|_| "无法初始化配置数据库".to_string())?;
        Ok(connection)
    }

    fn load_from_database(&self) -> Result<Option<AppSettings>, String> {
        let connection = self.open_database()?;
        let raw = connection
            .query_row(
                "SELECT settings_json FROM app_settings WHERE id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| "无法读取配置数据库".to_string())?;
        Ok(raw.and_then(|json| serde_json::from_str(&json).ok()))
    }
}
