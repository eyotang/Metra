import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, unlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { create as createFont } from "fontkit";

import { codePoints, requiredUiFontCharacters } from "./font-characters.mjs";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const outputDirectory = join(repositoryRoot, "public", "fonts");
const browserUserAgent = "Mozilla/5.0 AppleWebKit/537.36 Chrome/140.0.0.0 Safari/537.36";
const interUrl = "https://raw.githubusercontent.com/rsms/inter/v4.1/docs/font-files/InterVariable.woff2";
const interLicenseUrl = "https://raw.githubusercontent.com/rsms/inter/v4.1/LICENSE.txt";
const notoLicenseUrl = "https://raw.githubusercontent.com/notofonts/noto-cjk/Sans2.004/LICENSE";
const pinnedFiles = {
  "InterVariable.woff2": "693b77d4f32ee9b8bfc995589b5fad5e99adf2832738661f5402f9978429a8e3",
  "OFL-Inter.txt": "262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a",
  "OFL-Noto-Sans-CJK.txt": "6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2",
};
const fontDefinitions = [
  { locale: "zh-CN", family: "Noto Sans SC", cssFamily: "Metra CJK SC", fileStem: "NotoSansSC-UI", googleRevision: "v40" },
  { locale: "ja", family: "Noto Sans JP", cssFamily: "Metra CJK JP", fileStem: "NotoSansJP-UI", googleRevision: "v56" },
  { locale: "ko", family: "Noto Sans KR", cssFamily: "Metra CJK KR", fileStem: "NotoSansKR-UI", googleRevision: "v39" },
];
const charactersPerShard = 140;
const refreshLock = process.argv.includes("--refresh-lock");
const unsupportedArguments = process.argv.slice(2).filter((argument) => argument !== "--refresh-lock");
if (unsupportedArguments.length) throw new Error(`Unsupported arguments: ${unsupportedArguments.join(", ")}`);
const manifestPath = join(outputDirectory, "font-manifest.json");
const previousManifest = existsSync(manifestPath)
  ? JSON.parse(readFileSync(manifestPath, "utf8"))
  : null;

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function download(url, headers = {}) {
  let lastError;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    try {
      const response = await fetch(url, { headers, redirect: "follow", signal: AbortSignal.timeout(45_000) });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      return Buffer.from(await response.arrayBuffer());
    } catch (error) {
      lastError = error;
      if (attempt < 3) await new Promise((resolve) => setTimeout(resolve, attempt * 750));
    }
  }
  throw new Error(`Could not download ${url}`, { cause: lastError });
}

async function downloadPinned(url, file) {
  const existingPath = join(outputDirectory, file);
  if (existsSync(existingPath)) {
    const existing = readFileSync(existingPath);
    if (sha256(existing) === pinnedFiles[file]) return existing;
  }
  try {
    const downloaded = await download(url);
    if (sha256(downloaded) !== pinnedFiles[file]) {
      throw new Error(`Downloaded ${file} does not match its pinned SHA-256`);
    }
    return downloaded;
  } catch (error) {
    if (!existsSync(existingPath)) throw error;
    const existing = readFileSync(existingPath);
    if (sha256(existing) !== pinnedFiles[file]) throw error;
    console.warn(`Using the verified local ${file} because its pinned source is unavailable.`);
    return existing;
  }
}

async function downloadGoogleFont(definition, characters) {
  const stylesheetUrl = new URL("https://fonts.googleapis.com/css2");
  stylesheetUrl.searchParams.set("family", `${definition.family}:wght@100..900`);
  stylesheetUrl.searchParams.set("display", "block");
  stylesheetUrl.searchParams.set("text", characters);
  const stylesheet = (await download(stylesheetUrl, { "User-Agent": browserUserAgent })).toString("utf8");
  const urls = [...stylesheet.matchAll(/src:\s*url\((https:[^)]+)\)\s*format\(['"]woff2['"]\)/g)]
    .map((match) => match[1]);
  if (urls.length !== 1) throw new Error(`Expected one WOFF2 asset for ${definition.family}, received ${urls.length}`);
  const assetRevision = new URL(urls[0]).searchParams.get("v");
  if (assetRevision !== definition.googleRevision) {
    throw new Error(`Expected ${definition.family} ${definition.googleRevision}, received ${assetRevision ?? "no revision"}`);
  }
  const range = stylesheet.match(/unicode-range:\s*([^;]+);/i)?.[1]?.trim();
  if (!range) throw new Error(`Expected a unicode-range for ${definition.family}`);
  return {
    bytes: await download(urls[0], { "User-Agent": browserUserAgent }),
    stylesheetUrl: stylesheetUrl.toString(),
    assetUrl: urls[0],
    unicodeRange: range,
    codePoints: parseUnicodeRange(range),
  };
}

function characterShards(characters) {
  const values = [...characters];
  const shards = [];
  for (let index = 0; index < values.length; index += charactersPerShard) {
    shards.push(values.slice(index, index + charactersPerShard).join(""));
  }
  return shards;
}

function parseUnicodeRange(value) {
  const points = new Set();
  for (const rawToken of value.split(",")) {
    const token = rawToken.trim();
    const match = token.match(/^U\+([0-9A-F]+)(?:-([0-9A-F]+))?$/i);
    if (!match) throw new Error(`Unsupported unicode-range token: ${token}`);
    const start = Number.parseInt(match[1], 16);
    const end = Number.parseInt(match[2] ?? match[1], 16);
    for (let codePoint = start; codePoint <= end; codePoint += 1) points.add(codePoint);
  }
  return [...points].sort((left, right) => left - right);
}

function charactersFromCodePoints(points) {
  return points.map((codePoint) => String.fromCodePoint(codePoint)).join("");
}

mkdirSync(outputDirectory, { recursive: true });
const requiredCharacters = requiredUiFontCharacters();
console.log("Downloading pinned Inter font and OFL licenses...");
const [interBytes, interLicense, notoLicense] = await Promise.all([
  downloadPinned(interUrl, "InterVariable.woff2"),
  downloadPinned(interLicenseUrl, "OFL-Inter.txt"),
  downloadPinned(notoLicenseUrl, "OFL-Noto-Sans-CJK.txt"),
]);
const interFont = createFont(interBytes);
if (!interFont.version.startsWith("Version 4.001") || !interFont.variationAxes.opsz || !interFont.variationAxes.wght) {
  throw new Error("Pinned Inter must remain the expected optical-size and weight variable font");
}
const interActualCodePoints = new Set(interFont.characterSet);

const downloadedFonts = await Promise.all(fontDefinitions.map(async (definition) => {
  console.log(`Downloading ${definition.family} UI shards...`);
  const shards = [];
  const chunks = characterShards(requiredCharacters[definition.locale]);
  for (const [index, requestedCharacters] of chunks.entries()) {
    const downloaded = await downloadGoogleFont(definition, requestedCharacters);
    const file = `${definition.fileStem}-${index + 1}.woff2`;
    const requestedCodePoints = codePoints(requestedCharacters);
    const requestedSet = new Set(requestedCodePoints);
    const cssCodePoints = downloaded.codePoints.filter((codePoint) => requestedSet.has(codePoint));
    const parsedFont = createFont(downloaded.bytes);
    if (!parsedFont.familyName.startsWith(definition.family)
      || !parsedFont.version.startsWith("Version 2.004")
      || !parsedFont.variationAxes.wght) {
      throw new Error(`${file} must remain a Noto Sans CJK 2.004 weight variable font`);
    }
    const actualSet = new Set(parsedFont.characterSet);
    const actualCodePoints = requestedCodePoints.filter((codePoint) => actualSet.has(codePoint));
    const cssSet = new Set(cssCodePoints);
    if (!actualCodePoints.every((codePoint) => cssSet.has(codePoint))) {
      throw new Error(`${file} has requested cmap entries outside its CSS unicode-range`);
    }
    const effectiveCodePoints = actualCodePoints.filter((codePoint) => cssSet.has(codePoint));
    const digest = sha256(downloaded.bytes);
    const lockedShard = previousManifest?.notoSansCjk?.fonts?.[definition.locale]?.shards
      ?.find((shard) => shard.file === file);
    if (!refreshLock && lockedShard?.requestedCharacters === requestedCharacters && lockedShard.sha256 !== digest) {
      throw new Error(`${file} changed without UI-copy changes; inspect the font and rerun with --refresh-lock to accept it`);
    }
    shards.push({
      file,
      bytes: downloaded.bytes.byteLength,
      sha256: digest,
      stylesheetUrl: downloaded.stylesheetUrl,
      assetUrl: downloaded.assetUrl,
      requestedCharacters,
      requestedCodePoints,
      cssCharacters: charactersFromCodePoints(cssCodePoints),
      cssCodePoints,
      actualCharacters: charactersFromCodePoints(actualCodePoints),
      actualCodePoints,
      effectiveCharacters: charactersFromCodePoints(effectiveCodePoints),
      effectiveCodePoints,
      unicodeRange: downloaded.unicodeRange,
      bytesContent: downloaded.bytes,
    });
  }
  return { definition, shards };
}));

const fonts = {};
const cjkCss = [];
const generatedShardFiles = new Set();
for (const { definition, shards } of downloadedFonts) {
  const cssCodePoints = [...new Set(shards.flatMap((shard) => shard.cssCodePoints))]
    .sort((left, right) => left - right);
  const actualCodePoints = [...new Set(shards.flatMap((shard) => shard.actualCodePoints))]
    .sort((left, right) => left - right);
  const effectiveCodePoints = [...new Set(shards.flatMap((shard) => shard.effectiveCodePoints))]
    .sort((left, right) => left - right);
  for (const shard of shards) {
    generatedShardFiles.add(shard.file);
    cjkCss.push(`@font-face {\n  font-family: "${definition.cssFamily}";\n  src: url("/fonts/${shard.file}") format("woff2");\n  font-style: normal;\n  font-weight: 100 900;\n  font-display: block;\n  unicode-range: ${shard.unicodeRange};\n}`);
  }
  fonts[definition.locale] = {
    family: definition.family,
    cssFamily: definition.cssFamily,
    googleRevision: definition.googleRevision,
    requestedCharacters: requiredCharacters[definition.locale],
    requestedCodePoints: codePoints(requiredCharacters[definition.locale]),
    cssCharacters: charactersFromCodePoints(cssCodePoints),
    cssCodePoints,
    actualCharacters: charactersFromCodePoints(actualCodePoints),
    actualCodePoints,
    effectiveCharacters: charactersFromCodePoints(effectiveCodePoints),
    effectiveCodePoints,
    shards,
  };
}

const simplifiedChineseEffectiveCodePoints = new Set(fonts["zh-CN"].effectiveCodePoints);
for (const locale of ["zh-CN", "ja", "ko"]) {
  const stack = new Set([
    ...interActualCodePoints,
    ...fonts[locale].effectiveCodePoints,
    ...simplifiedChineseEffectiveCodePoints,
  ]);
  const missing = codePoints(requiredCharacters[locale]).filter((codePoint) => !stack.has(codePoint));
  if (missing.length) {
    throw new Error(`${locale} actual font stack is missing ${missing.map((codePoint) => `U+${codePoint.toString(16).toUpperCase()}`).join(", ")}`);
  }
}

for (const { shards } of downloadedFonts) {
  for (const shard of shards) {
    writeFileSync(join(outputDirectory, shard.file), shard.bytesContent);
    delete shard.bytesContent;
  }
}

for (const file of readdirSync(outputDirectory)) {
  if (/^NotoSans(?:SC|JP|KR)-UI-\d+\.woff2$/.test(file) && !generatedShardFiles.has(file)) {
    unlinkSync(join(outputDirectory, file));
  }
}

writeFileSync(join(outputDirectory, "InterVariable.woff2"), interBytes);
writeFileSync(join(outputDirectory, "OFL-Inter.txt"), interLicense);
writeFileSync(join(outputDirectory, "OFL-Noto-Sans-CJK.txt"), notoLicense);
writeFileSync(join(outputDirectory, "metra-fonts.css"), `@font-face {\n  font-family: "Metra Inter";\n  src: url("/fonts/InterVariable.woff2") format("woff2");\n  font-style: normal;\n  font-weight: 100 900;\n  font-display: block;\n}\n\n${cjkCss.sort().join("\n\n")}\n`);
writeFileSync(manifestPath, `${JSON.stringify({
  schemaVersion: 4,
  generatedBy: "scripts/fetch-ui-fonts.mjs",
  inter: {
    version: "4.1",
    sourceUrl: interUrl,
    file: "InterVariable.woff2",
    bytes: interBytes.byteLength,
    sha256: sha256(interBytes),
  },
  notoSansCjk: {
    sourceVersion: "2.004",
    licenseUrl: notoLicenseUrl,
    fonts,
  },
}, null, 2)}\n`);

console.log(`Wrote offline UI fonts to ${outputDirectory}`);
