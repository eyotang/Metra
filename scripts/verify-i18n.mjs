import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  createI18n,
  detectLocale,
  localizeProviderMessage,
  localizeQuotaLabel,
  resolveLocale,
  translations,
} from "../src/i18n.ts";

assert.equal(resolveLocale("zh-CN"), "zh-CN");
assert.equal(resolveLocale("zh_Hans_CN"), "zh-CN");
assert.equal(resolveLocale("zh-SG"), "zh-CN");
assert.equal(resolveLocale("zh-TW"), "en");
assert.equal(resolveLocale("zh-Hant"), "en");
assert.equal(resolveLocale("en-US"), "en");
assert.equal(resolveLocale("fr-FR"), "en");
assert.equal(detectLocale(["fr-FR", "zh-SG"]), "zh-CN");
assert.equal(detectLocale(["fr-FR", "en-GB"]), "en");

assert.deepEqual(Object.keys(translations.en).sort(), Object.keys(translations["zh-CN"]).sort());

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
  localizeProviderMessage("未检测到 Codex CLI", "codex", "not_installed", "en"),
  "Codex CLI was not detected",
);

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
const readme = readFileSync(new URL("../README.md", import.meta.url), "utf8");
const chineseReadme = readFileSync(new URL("../README.zh-CN.md", import.meta.url), "utf8");
assert.match(main, /applyDocumentLocale\(i18n\.locale\)/, "the document language must follow the detected locale");
assert.match(main, /localizeProviderMessage\(/, "native provider messages must cross a localization boundary");
assert.match(main, /localizeQuotaLabel\(/, "native quota labels must cross a localization boundary");
assert.doesNotMatch(main, /[\p{Script=Han}]/u, "frontend business logic must not contain hard-coded Chinese copy");
assert.doesNotMatch(main, /String\(reason\)/, "native errors must pass through the localized error boundary");
assert.match(nativeApp, /fn set_runtime_locale\(/, "the native title and status menu need a locale bridge");
assert.match(nativeApp, /generate_handler!\[[\s\S]*set_runtime_locale/, "the native locale command must be registered");
assert.match(readme, /\[简体中文\]\(README\.zh-CN\.md\)/, "the English README must link to Chinese");
assert.match(chineseReadme, /\[English\]\(README\.md\)/, "the Chinese README must link to English");
assert.match(readme, /immediately enters a 32 px partially hidden peek state/, "English docs must describe immediate post-drag peek");
assert.match(chineseReadme, /立即缩成 32 px 半隐藏状态/, "Chinese docs must describe immediate post-drag peek");

console.log("i18n locale negotiation, catalogs, interpolation, and formatters verified");
