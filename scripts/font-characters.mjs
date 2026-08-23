import { readdirSync, readFileSync } from "node:fs";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { translations } from "../src/i18n.ts";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const nativeSourceRoot = join(repositoryRoot, "src-tauri", "src");

const COMMON_UI_CHARACTERS = [
  "\u00a0\u202f",
  "、。，．：；！？（）［］【】「」『』《》〈〉",
  "“”‘’…—–·•",
  "％＋−＝／＠＃＆＊",
  "↻✓×←→↑↓",
  "¥₩€£℃",
  "年月日时分秒",
  "오전후년월일시분초",
].join("");

function sortedCharacters(value) {
  return [...new Set(value)].sort((left, right) => left.codePointAt(0) - right.codePointAt(0)).join("");
}

function sourceFiles(directory, extensions) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...sourceFiles(path, extensions));
    else if (extensions.has(extname(path))) files.push(path);
  }
  return files.sort();
}

function nativeStringLiterals() {
  return sourceFiles(nativeSourceRoot, new Set([".rs"]))
    .flatMap((path) => readFileSync(path, "utf8").match(/"(?:\\.|[^"\\])*"/g) ?? [])
    .join("");
}

function frontendStringLiterals() {
  const frontendSourceRoot = join(repositoryRoot, "src");
  return sourceFiles(frontendSourceRoot, new Set([".ts"]))
    .flatMap((path) => readFileSync(path, "utf8").match(/"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`/gs) ?? [])
    .join("");
}

function matches(character, expression) {
  expression.lastIndex = 0;
  return expression.test(character);
}

export function requiredUiFontCharacters() {
  const catalogText = Object.values(translations)
    .flatMap((catalog) => Object.values(catalog))
    .join("");
  const nativeText = nativeStringLiterals();
  const frontendText = frontendStringLiterals();
  const allText = `${catalogText}${frontendText}${nativeText}${COMMON_UI_CHARACTERS}`;
  const han = [...allText].filter((character) => matches(character, /\p{Script=Han}/u)).join("");
  const common = [...allText].filter((character) => {
    const codePoint = character.codePointAt(0);
    return codePoint > 0x7f
      && !matches(character, /\p{Script=Han}|\p{Script=Hiragana}|\p{Script=Katakana}|\p{Script=Hangul}|\p{Extended_Pictographic}/u);
  }).join("");
  const japanese = [...`${Object.values(translations.ja).join("")}${frontendText}${nativeText}`]
    .filter((character) => matches(character, /\p{Script=Hiragana}|\p{Script=Katakana}/u))
    .join("");
  const korean = [...`${Object.values(translations.ko).join("")}${frontendText}${nativeText}`]
    .filter((character) => matches(character, /\p{Script=Hangul}/u))
    .join("");

  return {
    "zh-CN": sortedCharacters(`${han}${common}`),
    ja: sortedCharacters(`${han}${japanese}${common}`),
    ko: sortedCharacters(`${han}${korean}${common}`),
  };
}

export function codePoints(value) {
  return [...value].map((character) => character.codePointAt(0));
}
