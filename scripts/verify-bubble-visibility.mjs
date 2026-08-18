import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");
const main = read("src/main.ts");
const css = read("src/styles.css");
const types = read("src/types.ts");
const appRust = read("src-tauri/src/app.rs");
const settingsRust = read("src-tauri/src/settings.rs");

assert.match(types, /bubbleVisibleProviders:\s*ProviderName\[\]/, "AppSettings must expose visible providers");
assert.match(main, /function bubbleVisibleProviderOrder\(/, "the bubble needs a normalized visible order");
assert.match(main, /const order = bubbleVisibleProviderOrder\(payload\?\.settings\)/, "the bubble must render only visible providers");
assert.match(main, /bubbleProviderOrder\(settings\)\.map\(\(provider\) => bubbleConfigItem/, "the editor must keep all providers configurable");
assert.match(main, /<button[^>]*class="visibility-toggle/, "each provider row needs an actual visibility button");
assert.match(main, /role="switch"/, "the visibility control must expose switch semantics");
assert.match(main, /aria-checked="\$\{visible\}"/, "the visibility state must be accessible");
assert.match(main, /\{ order, visibleProviders, cursorLabel, codexLabel, claudeLabel, cursorColor, codexColor, claudeColor \}/, "the frontend save payload must include visible providers");
assert.match(main, /bubbleVisibleProviders:\s*visibleProviders/, "the optimistic settings update must include visible providers");
assert.match(main, /visibleCount === 1/, "the UI must keep at least one provider visible");
assert.match(main, /aria-disabled/, "the last visible switch must explain why it cannot be hidden");
assert.match(appRust, /visible_providers:\s*Vec<Provider>/, "the native command must accept visible providers");
assert.match(appRust, /settings\.bubble_visible_providers\s*=\s*visible_providers/, "the native command must persist visibility");
assert.match(settingsRust, /bubble_visible_providers:\s*Vec<Provider>/, "SQLite settings must include visible providers");
assert.match(settingsRust, /normalized_visible\.is_empty\(\)/, "native normalization must reject an empty visible set");
assert.match(css, /\.bubble-center\.provider-count-1/, "the one-row bubble layout is missing");
assert.match(css, /\.bubble-center\.provider-count-2/, "the two-row bubble layout is missing");
assert.match(css, /\.bubble-center\.provider-count-3/, "the three-row bubble layout is missing");
assert.match(css, /\.bubble-center\s*\{[^}]*justify-content:\s*center/, "visible rows must stay vertically centered");
assert.match(css, /\.bubble-row\s*\{[^}]*width:\s*100%/, "visible rows must keep their full bubble width");
assert.match(css, /\.visibility-toggle/, "the visibility switch styles are missing");

console.log("Bubble visibility contract PASS");
