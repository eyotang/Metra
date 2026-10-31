use crate::settings::UiLanguage;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy)]
pub struct I18n {
    pub locale: &'static str,
}

fn catalogs() -> &'static BTreeMap<String, BTreeMap<String, String>> {
    static CATALOGS: OnceLock<BTreeMap<String, BTreeMap<String, String>>> = OnceLock::new();
    CATALOGS.get_or_init(|| {
        [
            ("zh-CN", include_str!("../assets/locales/zh-CN.json")),
            ("en", include_str!("../assets/locales/en.json")),
            ("ja", include_str!("../assets/locales/ja.json")),
            ("ko", include_str!("../assets/locales/ko.json")),
            ("fr", include_str!("../assets/locales/fr.json")),
            ("de", include_str!("../assets/locales/de.json")),
            ("ru", include_str!("../assets/locales/ru.json")),
        ]
        .into_iter()
        .map(|(locale, json)| {
            (
                locale.into(),
                serde_json::from_str(json).expect("embedded translation catalog"),
            )
        })
        .collect()
    })
}

pub fn resolve_locale(value: &str) -> &'static str {
    let value = value.replace('_', "-").to_ascii_lowercase();
    if value == "zh"
        || value.starts_with("zh-cn")
        || value.starts_with("zh-hans")
        || value.starts_with("zh-sg")
    {
        "zh-CN"
    } else if value == "ja" || value.starts_with("ja-") {
        "ja"
    } else if value == "ko" || value.starts_with("ko-") {
        "ko"
    } else if value == "fr" || value.starts_with("fr-") {
        "fr"
    } else if value == "de" || value.starts_with("de-") {
        "de"
    } else if value == "ru" || value.starts_with("ru-") {
        "ru"
    } else {
        "en"
    }
}

impl I18n {
    pub fn new(preference: UiLanguage) -> Self {
        Self {
            locale: preference.explicit_locale().unwrap_or_else(|| {
                resolve_locale(&sys_locale::get_locale().unwrap_or_else(|| "en".into()))
            }),
        }
    }
    pub fn catalog(self) -> BTreeMap<String, String> {
        catalogs()
            .get(self.locale)
            .unwrap_or(&catalogs()["en"])
            .clone()
    }
    pub fn t(self, key: &str) -> String {
        catalogs()
            .get(self.locale)
            .and_then(|catalog| catalog.get(key))
            .or_else(|| catalogs()["en"].get(key))
            .cloned()
            .unwrap_or_else(|| key.into())
    }
    pub fn format(self, key: &str, values: &[(&str, String)]) -> String {
        values.iter().fold(self.t(key), |text, (key, value)| {
            text.replace(&format!("{{{key}}}"), value)
        })
    }
    pub fn message(self, message: &str) -> String {
        for (prefix, key) in [
            ("官方用量刷新失败，显示缓存数据：", "claude.officialStale"),
            (
                "官方用量不可用，继续显示本机会话 Token：",
                "claude.officialUnavailableWithLocal",
            ),
            ("官方用量不可用：", "claude.officialUnavailable"),
        ] {
            if message.starts_with(prefix) {
                return self.t(key);
            }
        }
        static MESSAGE_KEYS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
        let keys = MESSAGE_KEYS.get_or_init(|| {
            serde_json::from_str(include_str!("../assets/provider-message-keys.json"))
                .expect("embedded message keys")
        });
        if let Some(key) = keys.get(message) {
            return self.t(key);
        }
        let chinese = &catalogs()["zh-CN"];
        chinese
            .iter()
            .find(|(_, value)| value.as_str() == message)
            .map_or_else(|| message.to_owned(), |(key, _)| self.t(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_locales_have_the_same_keys_and_placeholders() {
        let reference = &catalogs()["en"];
        for (locale, catalog) in catalogs() {
            assert_eq!(
                catalog.keys().collect::<Vec<_>>(),
                reference.keys().collect::<Vec<_>>(),
                "{locale}"
            );
            for (key, value) in catalog {
                let placeholders = |text: &str| {
                    text.split('{')
                        .skip(1)
                        .filter_map(|part| part.split_once('}').map(|(key, _)| key.to_owned()))
                        .collect::<std::collections::BTreeSet<_>>()
                };
                assert_eq!(
                    placeholders(value),
                    placeholders(&reference[key]),
                    "{locale}: {key}"
                );
            }
        }
        assert_eq!(resolve_locale("zh_Hans_CN"), "zh-CN");
        assert_eq!(resolve_locale("ja-JP"), "ja");
        for locale in [
            "fr-FR", "fr_CA", "fr", "de-DE", "de_AT", "de", "ru-RU", "ru_BY", "ru",
        ] {
            assert_eq!(resolve_locale(locale), &locale[..2]);
        }
        assert_eq!(resolve_locale("es-ES"), "en");
    }
}
