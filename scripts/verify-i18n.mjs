import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  createI18n,
  detectLocale,
  localizeProviderMessage,
  localizeQuotaLabel,
  resolveLocale,
  SUPPORTED_LOCALES,
  translations,
} from "../src/i18n.ts";

function rgb(hex) {
  return hex.slice(1).match(/../g).map((channel) => Number.parseInt(channel, 16));
}

function relativeLuminance(hex) {
  const [red, green, blue] = rgb(hex).map((channel) => {
    const normalized = channel / 255;
    return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4;
  });
  return (0.2126 * red) + (0.7152 * green) + (0.0722 * blue);
}

function contrastRatio(foreground, background) {
  const foregroundLuminance = relativeLuminance(foreground);
  const backgroundLuminance = relativeLuminance(background);
  return (Math.max(foregroundLuminance, backgroundLuminance) + 0.05)
    / (Math.min(foregroundLuminance, backgroundLuminance) + 0.05);
}

function colorDistance(first, second) {
  const firstRgb = rgb(first);
  const secondRgb = rgb(second);
  return Math.hypot(...firstRgb.map((channel, index) => channel - secondRgb[index]));
}

assert.equal(resolveLocale("zh-CN"), "zh-CN");
assert.equal(resolveLocale("zh_Hans_CN"), "zh-CN");
assert.equal(resolveLocale("zh-SG"), "zh-CN");
assert.equal(resolveLocale("zh-TW"), "en");
assert.equal(resolveLocale("zh-Hant"), "en");
assert.equal(resolveLocale("en-US"), "en");
assert.equal(resolveLocale("ja-JP"), "ja");
assert.equal(resolveLocale("ko-KR"), "ko");
assert.equal(resolveLocale("fr-FR"), "en");
assert.equal(detectLocale(["fr-FR", "zh-SG"]), "zh-CN");
assert.equal(detectLocale(["fr-FR", "en-GB"]), "en");
assert.equal(detectLocale(["fr-FR", "ja-JP"]), "ja");

assert.deepEqual(SUPPORTED_LOCALES, ["zh-CN", "en", "ja", "ko"]);
for (const locale of SUPPORTED_LOCALES) {
  assert.deepEqual(
    Object.keys(translations[locale]).sort(),
    Object.keys(translations["zh-CN"]).sort(),
    `${locale} catalog keys must match zh-CN`,
  );
}

const english = createI18n({ locale: "en-GB" });
assert.equal(english.locale, "en");
assert.equal(english.t("quota.remaining", { value: 42 }), "42% remaining");
assert.equal(english.t("quota.remaining"), "{value}% remaining");
assert.equal(english.formatNumber(12_345), "12,345");
assert.equal(english.formatNumber(undefined), "—");
assert.equal(english.formatDate("not-a-date"), "—");
assert.equal(english.formatPercentage(42), "42%");
assert.equal(localizeQuotaLabel("Agent 请求", "en"), "Agent requests");
assert.equal(localizeQuotaLabel("5 小时 · 次级", "en"), "5 hours · Secondary");
assert.equal(localizeQuotaLabel("Cursor 总额度 · $520", "en"), "Cursor total limit · $520");
assert.equal(
  localizeProviderMessage("Claude Code 未提供套餐额度；Token 为本地会话统计", "claude", "available", "en"),
  "Claude Code does not provide subscription limits; tokens are counted from local sessions.",
);
assert.equal(
  localizeProviderMessage("官方 API 今日用量按 UTC 统计，最多延迟 1 小时", "claude", "available", "en"),
  "Official daily API usage uses UTC and may be delayed by up to 1 hour.",
);
assert.equal(
  localizeProviderMessage("官方用量不可用：请求失败", "claude", "available", "ko"),
  "공식 사용량을 현재 이용할 수 없습니다.",
);
assert.equal(
  localizeProviderMessage("未检测到 Codex CLI", "codex", "not_installed", "en"),
  "Codex CLI was not detected",
);

const japanese = createI18n({ locale: "ja-JP" });
assert.equal(japanese.locale, "ja");
assert.equal(japanese.t("quota.remaining", { value: 42 }), "残り 42%");
assert.equal(localizeQuotaLabel("Agent 请求", "ja"), "Agent リクエスト");

const korean = createI18n({ locale: "ko-KR" });
assert.equal(korean.locale, "ko");
assert.equal(korean.t("quota.remaining", { value: 42 }), "42% 남음");
assert.equal(localizeProviderMessage("未检测到 Codex CLI", "codex", "not_installed", "ko"), "Codex CLI가 감지되지 않았습니다");

const localeChanges = [];
const unsubscribe = english.subscribe((locale) => localeChanges.push(locale));
assert.equal(english.setLocale("zh-CN"), "zh-CN");
assert.equal(english.t("quota.remaining", { value: 42 }), "剩余 42%");
assert.equal(english.setLocale("zh-Hans"), "zh-CN");
unsubscribe();
english.setLocale("en-US");
assert.deepEqual(localeChanges, ["zh-CN"]);

const main = readFileSync(new URL("../src/main.ts", import.meta.url), "utf8");
const nativeApp = readFileSync(new URL("../src-tauri/src/app.rs", import.meta.url), "utf8");
const settings = readFileSync(new URL("../src-tauri/src/settings.rs", import.meta.url), "utf8");
const types = readFileSync(new URL("../src/types.ts", import.meta.url), "utf8");
const readme = readFileSync(new URL("../README.md", import.meta.url), "utf8");
const chineseReadme = readFileSync(new URL("../README.zh-CN.md", import.meta.url), "utf8");
const japaneseReadme = readFileSync(new URL("../README.ja.md", import.meta.url), "utf8");
const koreanReadme = readFileSync(new URL("../README.ko.md", import.meta.url), "utf8");
const styles = readFileSync(new URL("../src/styles.css", import.meta.url), "utf8");
const menuRenderer = main.match(/function renderMenu\(\): void \{[\s\S]*?\n\}/)?.[0] ?? "";
assert.match(main, /applyDocumentLocale\(i18n\.locale\)/, "the document language must follow the detected locale");
assert.match(main, /function applyLanguagePreference\(/, "saved language preferences need one shared application boundary");
assert.match(main, /localizeProviderMessage\(/, "native provider messages must cross a localization boundary");
assert.match(main, /localizeQuotaLabel\(/, "native quota labels must cross a localization boundary");
assert.doesNotMatch(main, /[\p{Script=Han}]/u, "frontend business logic must not contain hard-coded Chinese copy");
assert.doesNotMatch(main, /String\(reason\)/, "native errors must pass through the localized error boundary");
assert.match(menuRenderer, /menu-language-row[\s\S]{0,500}data-ui-language/, "the context menu must start with a direct language selector");
assert.doesNotMatch(menuRenderer, /data-action="refresh"/, "the context menu must not duplicate the details refresh action");
assert.match(main, /set_ui_language/, "the context-menu language selector must persist changes");
assert.match(main, /STATUS_TEXT_KEYS/, "status copy must be translated at render time");
assert.match(main, /COLOR_HUE_KEYS/, "color accessibility copy must be translated at render time");
assert.match(types, /UiLanguage\s*=\s*"system"\s*\|\s*"zh-CN"\s*\|\s*"en"\s*\|\s*"ja"\s*\|\s*"ko"/, "frontend settings must expose all language preferences");
assert.match(settings, /pub ui_language:\s*UiLanguage/, "the language preference must be stored with app settings");
assert.match(settings, /_ => Self::System/, "unknown saved languages must fall back without discarding other settings");
assert.match(nativeApp, /fn set_runtime_locale\(/, "the native title and status menu need a locale bridge");
assert.match(nativeApp, /generate_handler!\[[\s\S]*set_runtime_locale/, "the native locale command must be registered");
assert.match(nativeApp, /fn set_ui_language\(/, "the native language preference command must exist");
assert.match(nativeApp, /native_copy\("ja-JP"\)/, "native Japanese copy must be covered by tests");
assert.match(nativeApp, /native_copy\("ko-KR"\)/, "native Korean copy must be covered by tests");
assert.match(readme, /\[简体中文\]\(README\.zh-CN\.md\)/, "the English README must link to Chinese");
assert.match(readme, /\[日本語\]\(README\.ja\.md\)/, "the English README must link to Japanese");
assert.match(readme, /\[한국어\]\(README\.ko\.md\)/, "the English README must link to Korean");
assert.match(chineseReadme, /\[English\]\(README\.md\)/, "the Chinese README must link to English");
assert.match(readme, /immediately enters a 32 px partially hidden peek state/, "English docs must describe immediate post-drag peek");
assert.match(chineseReadme, /立即缩成 32 px 半隐藏状态/, "Chinese docs must describe immediate post-drag peek");
assert.match(japaneseReadme, /32 px.*直ちに/u, "Japanese docs must describe immediate post-drag peek");
assert.match(koreanReadme, /즉시 32px/u, "Korean docs must describe immediate post-drag peek");
assert.match(readme, /context menu starts with a direct language selector[\s\S]*Auto detect[\s\S]*takes effect immediately/, "English docs must describe the context-menu language preference");
assert.match(chineseReadme, /自动检测[\s\S]*立即生效/, "Chinese docs must describe the context-menu language preference");
assert.match(japaneseReadme, /自動検出[\s\S]*すぐに反映/u, "Japanese docs must describe the context-menu language preference");
assert.match(koreanReadme, /자동 감지[\s\S]*즉시 적용/u, "Korean docs must describe the context-menu language preference");
const languageSelectRule = styles.match(/\.language-select-control select \{([^}]*)\}/)?.[1] ?? "";
const hoveredLanguageSelectRule = styles.match(/\.language-select-control select:hover,[^{]+\{([^}]*)\}/)?.[1] ?? "";
const languageOptionRule = styles.match(/\.language-select-control select option \{([^}]*)\}/)?.[1] ?? "";
const checkedLanguageOptionRule = styles.match(/\.language-select-control select option:checked \{([^}]*)\}/)?.[1] ?? "";
const panelRule = styles.match(/\.panel \{([^}]*)\}/)?.[1] ?? "";
const rootRule = styles.match(/:root \{([^}]*)\}/)?.[1] ?? "";
assert.match(rootRule, /color-scheme:\s*dark/, "the app must advertise a dark platform theme");
assert.match(languageSelectRule, /color-scheme:\s*dark/, "the native language selector must request dark platform controls");
assert.match(languageSelectRule, /background:\s*#[\da-f]{6}(?![\da-f])/i, "the closed language selector must use an opaque panel-colored background");
assert.match(languageOptionRule, /background(?:-color)?:\s*#[\da-f]{6}(?![\da-f])/i, "Windows language options must use an explicit opaque background");
assert.match(languageOptionRule, /color:\s*#[\da-f]{6}(?![\da-f])/i, "Windows language options must use an explicit readable text color");
const selectBackground = languageSelectRule.match(/background:\s*(#[\da-f]{6})(?![\da-f])/i)?.[1] ?? "";
const selectText = languageSelectRule.match(/(?:^|;)\s*color:\s*(#[\da-f]{6})(?![\da-f])/i)?.[1] ?? "";
const hoveredSelectBackground = hoveredLanguageSelectRule.match(/background:\s*(#[\da-f]{6})(?![\da-f])/i)?.[1] ?? "";
const hoveredSelectText = hoveredLanguageSelectRule.match(/(?:^|;)\s*color:\s*(#[\da-f]{6})(?![\da-f])/i)?.[1] ?? "";
const optionBackground = languageOptionRule.match(/background(?:-color)?:\s*(#[\da-f]{6})(?![\da-f])/i)?.[1] ?? "";
const optionText = languageOptionRule.match(/(?:^|;)\s*color:\s*(#[\da-f]{6})(?![\da-f])/i)?.[1] ?? "";
const checkedOptionBackground = checkedLanguageOptionRule.match(/background(?:-color)?:\s*(#[\da-f]{6})(?![\da-f])/i)?.[1] ?? "";
const checkedOptionText = checkedLanguageOptionRule.match(/(?:^|;)\s*color:\s*(#[\da-f]{6})(?![\da-f])/i)?.[1] ?? "";
const panelBackground = panelRule.match(/background:\s*([^;]+)/)?.[1] ?? "";
const panelColors = [...panelBackground.matchAll(/#[\da-f]{6}(?:[\da-f]{2})?/gi)].map((match) => match[0].slice(0, 7));
assert.ok(panelColors.some((color) => colorDistance(selectBackground, color) <= 12), "the selector background must stay in the panel gradient color family");
assert.ok(relativeLuminance(optionBackground) <= 0.12, "Windows language options must keep a dark opaque surface");
assert.ok(contrastRatio(selectText, selectBackground) >= 4.5, "the closed language selector must meet WCAG AA text contrast");
assert.ok(contrastRatio(hoveredSelectText, hoveredSelectBackground) >= 4.5, "the focused language selector must meet WCAG AA text contrast");
assert.ok(contrastRatio(optionText, optionBackground) >= 4.5, "Windows language options must meet WCAG AA text contrast");
assert.ok(contrastRatio(checkedOptionText, checkedOptionBackground) >= 4.5, "the selected language option must meet WCAG AA text contrast");

console.log("four-language detection, persistence contract, catalogs, interpolation, and docs verified");
