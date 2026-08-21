use metra_lib::{
    model::Provider,
    settings::{AppSettings, BUBBLE_COLOR_PALETTE, BubblePercentMode, SettingsStore},
};

#[test]
fn missing_settings_file_uses_safe_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(temp.path().join("settings.db"));
    let settings = store.load();
    assert_eq!(settings.refresh_minutes, 5);
    assert!(!settings.autostart);
    assert!(!settings.cursor_compat_enabled);
    assert!(!settings.bubble_snap_enabled);
    assert_eq!(settings.bubble_percent_mode, BubblePercentMode::Remaining);
    assert_eq!(
        settings.bubble_provider_order,
        vec![Provider::Cursor, Provider::Codex, Provider::Claude]
    );
    assert_eq!(settings.cursor_bubble_label, "C");
    assert_eq!(settings.codex_bubble_label, "X");
    assert_eq!(settings.claude_bubble_label, "A");
    assert_eq!(settings.cursor_bubble_color, "#9c83ff");
    assert_eq!(settings.codex_bubble_color, "#4bd8c0");
    assert_eq!(settings.claude_bubble_color, "#e99068");
    assert_eq!(
        settings.bubble_visible_providers,
        vec![Provider::Cursor, Provider::Codex, Provider::Claude]
    );
}

#[test]
fn bubble_snap_is_opt_in_and_persisted() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.db");
    let store = SettingsStore::new(path.clone());
    assert!(!store.load().bubble_snap_enabled);

    let settings = AppSettings {
        bubble_snap_enabled: true,
        ..AppSettings::default()
    };
    store.save(&settings).unwrap();
    drop(store);
    assert!(SettingsStore::new(path).load().bubble_snap_enabled);
}

#[test]
fn unsupported_refresh_interval_is_normalized_to_five_minutes() {
    let settings = AppSettings {
        refresh_minutes: 7,
        ..AppSettings::default()
    }
    .normalized();
    assert_eq!(settings.refresh_minutes, 5);
}
#[test]
fn bubble_percent_mode_is_persisted_and_old_settings_default_to_remaining() {
    let temp = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(temp.path().join("settings.db"));
    let settings = AppSettings {
        bubble_percent_mode: BubblePercentMode::Used,
        ..AppSettings::default()
    };
    store.save(&settings).unwrap();
    assert_eq!(store.load().bubble_percent_mode, BubblePercentMode::Used);

    std::fs::write(
        temp.path().join("settings.json"),
        r#"{"refreshMinutes":15,"autostart":false,"cursorCompatEnabled":false}"#,
    )
    .unwrap();
    std::fs::remove_file(store.path()).unwrap();
    assert_eq!(
        store.load().bubble_percent_mode,
        BubblePercentMode::Remaining
    );
    assert_eq!(
        store.load().bubble_visible_providers,
        vec![Provider::Cursor, Provider::Codex, Provider::Claude]
    );
    assert!(store.path().exists(), "legacy JSON should be migrated to SQLite");
}

#[test]
fn bubble_labels_and_order_are_normalized_and_persisted_in_sqlite() {
    let temp = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(temp.path().join("settings.db"));
    let settings = AppSettings {
        bubble_provider_order: vec![Provider::Codex, Provider::Cursor],
        cursor_bubble_label: " Cursor ".into(),
        codex_bubble_label: "  ".into(),
        ..AppSettings::default()
    };

    store.save(&settings).unwrap();
    let loaded = store.load();
    assert_eq!(
        loaded.bubble_provider_order,
        vec![Provider::Codex, Provider::Cursor, Provider::Claude]
    );
    assert_eq!(loaded.cursor_bubble_label, "Cur");
    assert_eq!(loaded.codex_bubble_label, "X");
    assert_eq!(loaded.claude_bubble_label, "A");

    let header = std::fs::read(store.path()).unwrap();
    assert!(header.starts_with(b"SQLite format 3\0"));
}

#[test]
fn visible_provider_selection_is_deduplicated_and_persisted() {
    let temp = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(temp.path().join("settings.db"));
    let settings = AppSettings {
        bubble_visible_providers: vec![
            Provider::Claude,
            Provider::Cursor,
            Provider::Claude,
        ],
        ..AppSettings::default()
    };

    store.save(&settings).unwrap();

    assert_eq!(
        store.load().bubble_visible_providers,
        vec![Provider::Claude, Provider::Cursor]
    );
}

#[test]
fn empty_visible_provider_selection_falls_back_to_all_providers() {
    let temp = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(temp.path().join("settings.db"));
    let settings = AppSettings {
        bubble_visible_providers: Vec::new(),
        ..AppSettings::default()
    };

    store.save(&settings).unwrap();

    assert_eq!(
        store.load().bubble_visible_providers,
        vec![Provider::Cursor, Provider::Codex, Provider::Claude]
    );
}

#[test]
fn old_two_provider_sqlite_settings_preserve_order_and_append_claude() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE app_settings (\
                id INTEGER PRIMARY KEY CHECK (id = 1),\
                settings_json TEXT NOT NULL\
            );",
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO app_settings (id, settings_json) VALUES (1, ?1)",
            [r#"{"refreshMinutes":15,"autostart":false,"cursorCompatEnabled":false,"bubbleProviderOrder":["codex","cursor"],"cursorBubbleLabel":"Q","codexBubbleLabel":"Z"}"#],
        )
        .unwrap();
    drop(connection);

    let settings = SettingsStore::new(path).load();
    assert_eq!(
        settings.bubble_provider_order,
        vec![Provider::Codex, Provider::Cursor, Provider::Claude]
    );
    assert_eq!(settings.cursor_bubble_label, "Q");
    assert_eq!(settings.codex_bubble_label, "Z");
    assert_eq!(settings.claude_bubble_label, "A");
    assert_eq!(settings.cursor_bubble_color, "#9c83ff");
    assert_eq!(settings.codex_bubble_color, "#4bd8c0");
    assert_eq!(settings.claude_bubble_color, "#e99068");
    assert!(!settings.bubble_snap_enabled);
    assert_eq!(
        settings.bubble_visible_providers,
        vec![Provider::Cursor, Provider::Codex, Provider::Claude]
    );
}

#[test]
fn bubble_colors_only_accept_the_fixed_palette() {
    let settings = AppSettings {
        cursor_bubble_color: " #487BEA ".into(),
        codex_bubble_color: "#123456".into(),
        claude_bubble_color: "url(javascript:alert(1))".into(),
        ..AppSettings::default()
    }
    .normalized();

    assert_eq!(settings.cursor_bubble_color, "#487bea");
    assert_eq!(settings.codex_bubble_color, "#4bd8c0");
    assert_eq!(settings.claude_bubble_color, "#e99068");
}

#[test]
fn every_fixed_palette_color_survives_normalization() {
    assert_eq!(BUBBLE_COLOR_PALETTE.len(), 55);
    for color in BUBBLE_COLOR_PALETTE {
        let settings = AppSettings {
            cursor_bubble_color: color.into(),
            codex_bubble_color: color.into(),
            claude_bubble_color: color.into(),
            ..AppSettings::default()
        }
        .normalized();
        assert_eq!(settings.cursor_bubble_color, color);
        assert_eq!(settings.codex_bubble_color, color);
        assert_eq!(settings.claude_bubble_color, color);
    }
}

#[test]
fn bubble_colors_are_persisted_in_sqlite() {
    let temp = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(temp.path().join("settings.db"));
    let settings = AppSettings {
        cursor_bubble_color: "#ff6b6b".into(),
        codex_bubble_color: "#ffc21a".into(),
        claude_bubble_color: "#8752df".into(),
        ..AppSettings::default()
    };

    store.save(&settings).unwrap();
    let loaded = store.load();
    assert_eq!(loaded.cursor_bubble_color, "#ff6b6b");
    assert_eq!(loaded.codex_bubble_color, "#ffc21a");
    assert_eq!(loaded.claude_bubble_color, "#8752df");

    let connection = rusqlite::Connection::open(store.path()).unwrap();
    let raw: String = connection
        .query_row(
            "SELECT settings_json FROM app_settings WHERE id = 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(raw.contains(r##""cursorBubbleColor":"#ff6b6b""##));
    assert!(raw.contains(r##""codexBubbleColor":"#ffc21a""##));
    assert!(raw.contains(r##""claudeBubbleColor":"#8752df""##));
}
