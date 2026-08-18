import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");
const main = read("src/main.ts");
const css = read("src/styles.css");
const types = read("src/types.ts");
const waterLogo = read("src/metra-water.svg");
const appRust = read("src-tauri/src/app.rs");
const settingsRust = read("src-tauri/src/settings.rs");

const hexes = (value) => [...value.matchAll(/#[0-9a-f]{6}/g)].map((match) => match[0]);
const frontendPalette = main.match(/const COLOR_PALETTE = \[([\s\S]*?)\] as const satisfies/)?.[1] ?? "";
const backendPalette = settingsRust.match(/BUBBLE_COLOR_PALETTE:\s*\[&str;\s*55\]\s*=\s*\[([\s\S]*?)\];/)?.[1] ?? "";
const frontendColors = hexes(frontendPalette);
const backendColors = hexes(backendPalette);
const frontendRows = [...frontendPalette.matchAll(/\[([^\[\]]+)\]/g)].map((match) => hexes(match[1]));

assert.equal(frontendColors.length, 55, "frontend palette must contain 55 colors");
assert.equal(new Set(frontendColors).size, 55, "frontend palette colors must be unique");
assert.equal(frontendRows.length, 5, "frontend palette must contain five tone rows");
assert.deepEqual(frontendRows.map((row) => row.length), [11, 11, 11, 11, 11], "each palette row must contain eleven hues");
assert.deepEqual(frontendColors, backendColors, "frontend and SQLite validation palettes must stay identical");

for (const field of ["cursorBubbleColor", "codexBubbleColor", "claudeBubbleColor"]) {
  assert.match(types, new RegExp(`${field}:\\s*string`), `${field} is missing from AppSettings`);
}
for (const argument of ["cursorColor", "codexColor", "claudeColor"]) {
  assert.match(main, new RegExp(`\\b${argument}\\b`), `${argument} is missing from the frontend save payload`);
  assert.match(appRust, new RegExp(`${argument.replace("Color", "_color")}:\\s*String`), `${argument} is missing from the native command`);
  const settingsField = argument.replace("Color", "_bubble_color");
  assert.match(appRust, new RegExp(`settings\\.${settingsField}\\s*=\\s*${argument.replace("Color", "_color")}`), `${argument} is not assigned to AppSettings`);
}

assert.match(main, /<button[^>]*class="bubble-config-dot color-trigger \$\{provider\}"/, "the color dot must be a button trigger");
assert.match(main, /aria-haspopup="dialog"/, "the trigger must expose its popup semantics");
assert.match(main, /aria-expanded="false"/, "the trigger must expose its expanded state");
assert.match(main, /className\s*=\s*"color-palette-popover"/, "the in-panel palette is missing");
assert.match(main, /aria-pressed=/, "the selected swatch must be announced");
assert.match(main, /if \(closeColorPalette\(true\)\)/, "Escape must close the palette before hiding the panel");
assert.match(main, /dismissColorPaletteOnPointerDown/, "outside-click dismissal is missing");
assert.match(main, /class="bubble-row[^\n]+data-provider-accent/, "the floating bubble color hook is missing");
assert.match(main, /class="provider-card[^\n]+data-provider-accent/, "the provider card color hook is missing");
assert.match(main, /class="bubble-config-dot color-trigger[^\n]+data-provider-accent/, "the configuration dot color hook is missing");
assert.match(css, /var\(--provider-color\)/, "provider visuals must consume the shared color variable");
assert.doesNotMatch(main, /type="color"/, "native color inputs close the Tauri panel and must not be used");
assert.doesNotMatch(css, /\.provider-dot\.(?:cursor|codex|claude)::after/, "provider dots must not keep hard-coded colors");
assert.doesNotMatch(css, /\.bubble-row\.healthy\.(?:cursor|codex|claude)/, "healthy bubble text must not keep hard-coded colors");

const logoRing = waterLogo.match(/<linearGradient id="ring"[\s\S]*?<\/linearGradient>/)?.[0] ?? "";
const bubbleShell = css.match(/\.bubble-shell\s*\{[^}]+\}/)?.[0] ?? "";
assert.deepEqual(hexes(bubbleShell), hexes(logoRing), "the floating bubble ring must use the logo ring colors");
assert.match(bubbleShell, /linear-gradient\(/, "the floating bubble ring must follow the logo's linear gradient direction");

console.log("Color picker contract PASS");
