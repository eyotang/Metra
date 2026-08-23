import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { nextLanguageOptionIndex } from "../src/language-listbox.ts";

const readSource = (path) => readFileSync(new URL(path, import.meta.url), "utf8").replace(/\r\n?/g, "\n");
const main = readSource("../src/main.ts");
const styles = readSource("../src/styles.css");
const nativeApp = readSource("../src-tauri/src/app.rs");
const workflow = readSource("../.github/workflows/ci.yml");
const menuRenderer = main.match(/function renderMenu\(\): void \{[\s\S]*?\n\}/)?.[0] ?? "";

assert.equal(nextLanguageOptionIndex("ArrowDown", 1, 5), 2);
assert.equal(nextLanguageOptionIndex("ArrowDown", 4, 5), 4);
assert.equal(nextLanguageOptionIndex("ArrowUp", 0, 5), 0);
assert.equal(nextLanguageOptionIndex("Home", 3, 5), 0);
assert.equal(nextLanguageOptionIndex("End", 1, 5), 4);

assert.doesNotMatch(menuRenderer, /<select\b|<option\b/, "the language control must not use a platform-native select popup");
assert.match(menuRenderer, /data-language-trigger/, "the language control needs a dedicated trigger button");
assert.match(menuRenderer, /aria-haspopup="listbox"/, "the language trigger must expose listbox semantics");
assert.match(menuRenderer, /role="listbox"/, "the popup must expose listbox semantics");
assert.match(menuRenderer, /role="option"/, "every language choice must expose option semantics");
assert.match(main, /ArrowDown|ArrowUp/, "the custom listbox needs arrow-key navigation");
assert.match(main, /Home|End/, "the custom listbox needs first/last keyboard navigation");
assert.match(main, /Escape/, "the custom listbox must close with Escape");

const listboxRule = styles.match(/\.language-listbox\s*\{([^}]*)\}/)?.[1] ?? "";
assert.match(listboxRule, /position:\s*absolute/, "the open listbox must overlay menu content instead of changing its measured height");
assert.match(listboxRule, /top:\s*calc\(100%\s*\+/, "the language listbox must always anchor below its trigger");
assert.doesNotMatch(listboxRule, /\bbottom\s*:/, "the language listbox must never flip above its trigger");
assert.match(listboxRule, /background:\s*#[\da-f]{6}(?![\da-f])/i, "the listbox needs an opaque dark surface on Windows");

const menuPanelRule = styles.match(/\.menu-panel\s*\{([^}]*)\}/)?.[1] ?? "";
assert.match(menuPanelRule, /height:\s*auto/, "the menu surface must expose its actual content height");
assert.match(main, /getBoundingClientRect\(\)\.height/, "the frontend must measure the rendered menu surface");
assert.match(main, /surface\.scrollHeight/, "clamped menu windows must preserve their full scrollable content height");
assert.match(main, /surface\.offsetHeight\s*-\s*surface\.clientHeight/, "menu measurement must include its border box without leaving a phantom scrollbar");
assert.match(menuPanelRule, /max-height:\s*100vh/, "the menu surface must respect the native viewport height");
assert.match(menuPanelRule, /overflow-y:\s*auto/, "a work-area-clamped menu must keep every action scrollable");
assert.match(main, /requestAnimationFrame/, "menu measurement must wait for layout");
assert.match(main, /"resize_menu_panel"/, "the measured height must be sent to the native window layer");
assert.match(main, /requestId/, "menu resize requests must carry the current panel generation");
assert.match(nativeApp, /fn resize_menu_panel\(/, "the native layer must expose a menu resize command");
assert.match(nativeApp, /generate_handler!\[[\s\S]*resize_menu_panel/, "the menu resize command must be registered");
assert.match(nativeApp, /PANEL_MODE_MENU/, "native resizing must be restricted to the active menu mode");
assert.match(nativeApp, /latest_request[\s\S]*request_id|request_id[\s\S]*latest_request/, "stale menu measurements must be rejected");
const applyPanelFrame = nativeApp.match(/fn apply_panel_frame\([\s\S]*?\n\}/)?.[0] ?? "";
assert.ok(
  applyPanelFrame.indexOf(".set_position(") < applyPanelFrame.indexOf(".set_size("),
  "cross-DPI panels must move to the target monitor before applying their physical size",
);
assert.match(nativeApp, /menu_height[\s\S]*store\(requested_height\.round\(\)/, "the native cache must preserve intrinsic menu height before work-area clamping");
assert.doesNotMatch(main, /const MENU_PANEL_HEIGHT\s*=\s*480/, "the frontend must not encode a fixed menu height");
assert.match(workflow, /npm run verify:menu-layout/, "CI must enforce the menu layout and interaction contract");

console.log("dynamic menu sizing and downward custom language listbox contract verified");
