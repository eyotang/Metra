use metra_lib::{
    model::Provider,
    settings::{AppSettings, SettingsStore},
    settings_change::Change,
};

#[test]
fn final_visible_provider_cannot_be_hidden() {
    let settings = AppSettings {
        bubble_visible_providers: vec![Provider::Codex],
        ..Default::default()
    };
    assert!(
        Change::Visible {
            provider: Provider::Codex,
            visible: false
        }
        .validate(&settings)
        .is_err()
    );
    assert!(
        Change::Visible {
            provider: Provider::Cursor,
            visible: false
        }
        .validate(&settings)
        .is_ok()
    );
}

#[test]
fn shell_commands_preserve_all_languages_and_provider_preferences_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(dir.path().join("settings.db"));
    for locale in ["system", "en", "zh-CN", "ja", "ko", "fr", "de", "ru"] {
        let mut settings = AppSettings::default();
        for value in [
            serde_json::json!({"kind":"language", "value":locale}),
            serde_json::json!({"kind":"order", "value":["claude","codex","cursor"]}),
            serde_json::json!({"kind":"label", "value":{"provider":"codex","label":"AI"}}),
            serde_json::json!({"kind":"percent", "value":"used"}),
            serde_json::json!({"kind":"refresh", "value":15}),
        ] {
            let change: Change = serde_json::from_value(value).unwrap();
            change.validate(&settings).unwrap();
            change.apply(&mut settings);
        }
        let settings = settings.normalized();
        store.save(&settings).unwrap();
        assert_eq!(
            serde_json::to_value(store.load()).unwrap(),
            serde_json::to_value(settings).unwrap()
        );
    }
}
