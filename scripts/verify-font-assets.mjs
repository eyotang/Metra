import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, statSync } from "node:fs";
import { create as createFont } from "fontkit";

import { codePoints, requiredUiFontCharacters } from "./font-characters.mjs";

const read = (path) => readFileSync(new URL(path, import.meta.url));
const readText = (path) => read(path).toString("utf8").replace(/\r\n?/g, "\n");
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const manifest = JSON.parse(readText("../public/fonts/font-manifest.json"));
const requiredCharacters = requiredUiFontCharacters();
const styles = readText("../src/styles.css");
const fontStyles = readText("../public/fonts/metra-fonts.css");
const main = readText("../src/main.ts");
const tauriConfig = JSON.parse(readText("../src-tauri/tauri.conf.json"));
const packageJson = JSON.parse(readText("../package.json"));
const workflow = readText("../.github/workflows/ci.yml");

function parseUnicodeRange(value) {
  const points = new Set();
  for (const rawToken of value.split(",")) {
    const token = rawToken.trim();
    const match = token.match(/^U\+([0-9A-F]+)(?:-([0-9A-F]+))?$/i);
    assert.ok(match, `unsupported unicode-range token: ${token}`);
    const start = Number.parseInt(match[1], 16);
    const end = Number.parseInt(match[2] ?? match[1], 16);
    for (let codePoint = start; codePoint <= end; codePoint += 1) points.add(codePoint);
  }
  return [...points].sort((left, right) => left - right);
}

assert.equal(manifest.schemaVersion, 4);
assert.equal(manifest.generatedBy, "scripts/fetch-ui-fonts.mjs");
assert.match(manifest.inter.sourceUrl, /\/rsms\/inter\/v4\.1\//);
assert.equal(manifest.inter.sha256, "693b77d4f32ee9b8bfc995589b5fad5e99adf2832738661f5402f9978429a8e3");
assert.match(manifest.notoSansCjk.licenseUrl, /\/notofonts\/noto-cjk\/Sans2\.004\//);

const fontEntries = [
  manifest.inter,
  ...Object.values(manifest.notoSansCjk.fonts).flatMap((entry) => entry.shards),
];
let totalFontBytes = 0;
const actualCodePointsByFile = new Map();
const parsedFontByFile = new Map();
for (const entry of fontEntries) {
  const bytes = read(`../public/fonts/${entry.file}`);
  assert.equal(bytes.byteLength, entry.bytes, `${entry.file} byte size must match its manifest`);
  assert.equal(sha256(bytes), entry.sha256, `${entry.file} hash must match its manifest`);
  const font = createFont(bytes);
  parsedFontByFile.set(entry.file, font);
  actualCodePointsByFile.set(entry.file, [...new Set(font.characterSet)].sort((left, right) => left - right));
  if (entry.file === manifest.inter.file) {
    assert.match(font.version, /^Version 4\.001/);
    assert.deepEqual(Object.keys(font.variationAxes).sort(), ["opsz", "wght"]);
  } else {
    assert.match(font.version, /^Version 2\.004/);
    assert.ok(font.variationAxes.wght, `${entry.file} must keep its variable weight axis`);
  }
  totalFontBytes += bytes.byteLength;
}
assert.ok(totalFontBytes <= 2_500_000, `UI font assets must stay compact; received ${totalFontBytes} bytes`);

for (const [locale, revision] of [["zh-CN", "v40"], ["ja", "v56"], ["ko", "v39"]]) {
  const entry = manifest.notoSansCjk.fonts[locale];
  assert.ok(entry, `${locale} needs an offline Noto Sans UI subset`);
  assert.equal(entry.googleRevision, revision, `${locale} Google Fonts revision must stay pinned`);
  assert.equal(entry.requestedCharacters, requiredCharacters[locale], `${locale} font request must match current UI copy`);
  assert.deepEqual(entry.requestedCodePoints, codePoints(requiredCharacters[locale]), `${locale} requested code points must be deterministic`);
  assert.ok(entry.shards.length >= 1, `${locale} needs at least one compact WOFF2 shard`);
  assert.equal(entry.shards.map((shard) => shard.requestedCharacters).join(""), entry.requestedCharacters);
  const cssCodePoints = [...new Set(entry.shards.flatMap((shard) => shard.cssCodePoints))]
    .sort((left, right) => left - right);
  const actualCodePoints = [...new Set(entry.shards.flatMap((shard) => shard.actualCodePoints))]
    .sort((left, right) => left - right);
  const effectiveCodePoints = [...new Set(entry.shards.flatMap((shard) => shard.effectiveCodePoints))]
    .sort((left, right) => left - right);
  assert.deepEqual(entry.cssCodePoints, cssCodePoints, `${locale} CSS-declared code points must match its shards`);
  assert.equal(entry.cssCharacters, cssCodePoints.map((codePoint) => String.fromCodePoint(codePoint)).join(""));
  assert.deepEqual(entry.actualCodePoints, actualCodePoints, `${locale} actual code points must match its shards`);
  assert.equal(entry.actualCharacters, actualCodePoints.map((codePoint) => String.fromCodePoint(codePoint)).join(""));
  assert.deepEqual(entry.effectiveCodePoints, effectiveCodePoints, `${locale} effective code points must match its shards`);
  assert.equal(entry.effectiveCharacters, effectiveCodePoints.map((codePoint) => String.fromCodePoint(codePoint)).join(""));
  for (const shard of entry.shards) {
    assert.deepEqual(shard.requestedCodePoints, codePoints(shard.requestedCharacters));
    const declaredCodePoints = new Set(parseUnicodeRange(shard.unicodeRange));
    assert.ok(shard.cssCodePoints.every((codePoint) => declaredCodePoints.has(codePoint)), `${shard.file} requested characters must be declared in its CSS range`);
    assert.ok(shard.cssCodePoints.every((codePoint) => shard.requestedCodePoints.includes(codePoint)), `${shard.file} must not claim unrequested UI characters`);
    assert.equal(new URL(shard.assetUrl).searchParams.get("v"), revision, `${shard.file} asset revision must stay pinned`);
    const actualSet = new Set(actualCodePointsByFile.get(shard.file));
    const actualRequestedCodePoints = shard.requestedCodePoints.filter((codePoint) => actualSet.has(codePoint));
    assert.deepEqual(shard.actualCodePoints, actualRequestedCodePoints, `${shard.file} actual cmap must match its manifest`);
    assert.equal(shard.actualCharacters, actualRequestedCodePoints.map((codePoint) => String.fromCodePoint(codePoint)).join(""));
    assert.ok(parsedFontByFile.get(shard.file).familyName.startsWith(entry.family), `${shard.file} must keep the expected regional family`);
    const cssSet = new Set(shard.cssCodePoints);
    assert.ok(actualRequestedCodePoints.every((codePoint) => cssSet.has(codePoint)), `${shard.file} cmap must stay inside its CSS range`);
    const effectiveRequestedCodePoints = actualRequestedCodePoints.filter((codePoint) => cssSet.has(codePoint));
    assert.deepEqual(shard.effectiveCodePoints, effectiveRequestedCodePoints, `${shard.file} effective browser coverage must match cmap intersected with CSS`);
    assert.equal(shard.effectiveCharacters, effectiveRequestedCodePoints.map((codePoint) => String.fromCodePoint(codePoint)).join(""));
    assert.match(fontStyles, new RegExp(`${shard.file.replaceAll(".", "\\.")}[^}]*unicode-range:\\s*${shard.unicodeRange.replaceAll("+", "\\+")}`, "s"));
  }
}

const interCoverage = new Set(actualCodePointsByFile.get(manifest.inter.file));
const simplifiedChineseCoverage = new Set(manifest.notoSansCjk.fonts["zh-CN"].effectiveCodePoints);
for (const locale of ["zh-CN", "ja", "ko"]) {
  const fallbackCoverage = new Set([
    ...interCoverage,
    ...manifest.notoSansCjk.fonts[locale].effectiveCodePoints,
    ...simplifiedChineseCoverage,
  ]);
  const missing = codePoints(requiredCharacters[locale]).filter((codePoint) => !fallbackCoverage.has(codePoint));
  assert.deepEqual(missing, [], `${locale} font stack is missing pinned UI glyphs`);
}

const interLicense = read("../public/fonts/OFL-Inter.txt");
const notoLicense = read("../public/fonts/OFL-Noto-Sans-CJK.txt");
assert.equal(sha256(interLicense), "262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a");
assert.equal(sha256(notoLicense), "6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2");
assert.match(interLicense.toString("utf8"), /SIL OPEN FONT LICENSE Version 1\.1/);
assert.match(notoLicense.toString("utf8"), /SIL OPEN FONT LICENSE Version 1\.1/);
assert.match(styles, /^@import\s+url\("\/fonts\/metra-fonts\.css"\);/);
assert.match(fontStyles, /@font-face\s*\{[^}]*font-family:\s*"Metra Inter"[^}]*InterVariable\.woff2/s);
for (const [locale, family] of [
  ["zh-CN", "Metra CJK SC"],
  ["ja", "Metra CJK JP"],
  ["ko", "Metra CJK KR"],
]) {
  assert.match(fontStyles, new RegExp(`font-family:\\s*"${family}"`, "s"));
  assert.match(styles, new RegExp(`:lang\\(${locale.replace("-", "\\-")}\\)[^}]*--metra-cjk-fonts:\\s*"${family}"`, "s"));
}
assert.match(styles, /:lang\(ja\)[^}]*"Metra CJK JP",\s*"Metra CJK SC"/s);
assert.match(styles, /:lang\(ko\)[^}]*"Metra CJK KR",\s*"Metra CJK SC"/s);
assert.match(styles, /:root,\s*\[lang\][^}]*font-family:\s*"Metra Inter",\s*var\(--metra-cjk-fonts\),\s*sans-serif/s);
assert.match(styles, /font-synthesis:\s*none/);
assert.match(styles, /font-optical-sizing:\s*none/);
assert.match(styles, /\.bubble-config-item input[^}]*font-family:\s*inherit/s, "editable bubble labels must not use the platform form-control font");
assert.match(styles, /kbd[^}]*font-family:\s*inherit[^}]*font-size:\s*9px/s, "keyboard hints must not use the platform monospace font");
assert.match(main, /languageTag:/, "language choices must carry explicit language tags");
assert.match(main, /lang="\$\{selectedLanguage\.languageTag/s, "the selected language name needs its own language tag");
assert.match(main, /role="option"[^>]*lang="\$\{languageTag/s, "each language option needs its own language tag");
assert.match(main, /document\.fonts\.ready\.then/, "menu sizing must re-run after packaged fonts finish loading");
assert.doesNotMatch(main, /⠿/, "the drag handle must use deterministic CSS geometry, not a missing font glyph");
assert.match(styles, /\.drag-handle::before[^}]*radial-gradient/s);
assert.match(tauriConfig.app.security.csp, /font-src 'self'/, "Tauri CSP must explicitly allow packaged fonts only");
assert.equal(packageJson.scripts["verify:fonts"], "node --experimental-strip-types scripts/verify-font-assets.mjs");
assert.equal(packageJson.devDependencies.fontkit, "2.0.4", "WOFF2 cmap verification must stay on the reviewed parser version");
assert.match(workflow, /npm run verify:fonts/);
assert.equal(statSync(new URL("../public/fonts/font-manifest.json", import.meta.url)).isFile(), true);

console.log(`offline UI fonts and ${totalFontBytes} bytes of pinned assets verified`);
