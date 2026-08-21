import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");
const main = read("src/main.ts");
const i18n = read("src/i18n.ts");
const types = read("src/types.ts");
const css = read("src/styles.css");

assert.match(
  types,
  /export type QuotaKind\s*=\s*"cursor_models"\s*\|\s*"other_models"\s*\|\s*"grok_bot"/,
  "the frontend quota contract must expose Cursor Ultra quota kinds",
);
assert.match(types, /interface QuotaWindow[^}]*kind\?:\s*QuotaKind/s, "quota kind must be optional for older providers");
assert.match(types, /interface CostUsage[^}]*onDemandEnabled\?:\s*boolean/s, "On-Demand enabled state is missing");

const ultraDetection = main.match(/function isCursorUltraUsage\([\s\S]*?\n\}/)?.[0] ?? "";
assert.match(ultraDetection, /quota\.kind/, "Ultra detection must use quota kinds");
assert.match(ultraDetection, /provider\.plan\?\.toLowerCase\(\)\s*===\s*"ultra"/, "known Ultra plans must stay on the four-block view when a pool is temporarily unavailable");
assert.match(main, /const CURSOR_ULTRA_QUOTA_KINDS[^=]*=\s*\["cursor_models",\s*"other_models",\s*"grok_bot"\]/, "Ultra must define its three percentage pools in display order");
assert.match(main, /function cursorUltraBlocks\(/, "the Cursor Ultra four-block renderer is missing");
assert.match(main, /CURSOR_ULTRA_QUOTA_KINDS\.map/, "all three Ultra percentage pools must be rendered");
assert.match(main, /function cursorUltraOnDemandBlock\(/, "Ultra needs a dedicated On-Demand block");

const ultraOnDemand = main.match(/function cursorUltraOnDemandBlock\([\s\S]*?\n\}/)?.[0] ?? "";
assert.match(ultraOnDemand, /onDemandEnabled\s*===\s*false[\s\S]*t\("common\.disabled"\)/, "disabled On-Demand must use the localized disabled label");
assert.match(i18n, /"common\.disabled":\s*"未启用"[\s\S]*"common\.disabled":\s*"Disabled"/, "disabled On-Demand needs both Chinese and English copy");
assert.doesNotMatch(ultraOnDemand, /CURSOR_ON_DEMAND_FALLBACK_CENTS|50_000|\$500/, "Ultra On-Demand must never invent a $500 limit");
assert.doesNotMatch(main, /CURSOR_ULTRA_INCLUDED_FALLBACK_CENTS/, "the obsolete combined Ultra subscription fallback must not remain reachable");

const card = main.match(/function providerCard\([\s\S]*?\n\}/)?.[0] ?? "";
assert.match(card, /isCursorUltraUsage\(provider\)/, "the provider card must distinguish Ultra from legacy Cursor plans");
assert.match(card, /cursorUltraBlocks\(provider\)/, "the provider card must route Ultra to the four-block renderer");
assert.match(card, /cursorCostBlocks\(provider\)/, "Team and other legacy Cursor plans must keep the two monetary blocks");

const legacyCosts = main.match(/function cursorCostBlocks\([\s\S]*?\n\}/)?.[0] ?? "";
assert.match(legacyCosts, /cursorCostBlock\(t\("quota\.subscription"/, "the legacy subscription monetary block must remain localized");
assert.match(legacyCosts, /cursorCostBlock\(`On-Demand/, "the legacy On-Demand monetary block must remain");

const bubble = main.match(/function bubblePercent\([\s\S]*?\n\}/)?.[0] ?? "";
assert.match(bubble, /Math\.max\([\s\S]*usedPercent/, "used mode must show the most-used quota");
assert.match(bubble, /Math\.min\([\s\S]*remainingPercent/, "remaining mode must show the least-remaining quota");

assert.match(css, /\.cursor-ultra-blocks/, "Ultra block layout styles are missing");
assert.match(css, /\.cursor-ultra-on-demand\.disabled/, "disabled On-Demand styles are missing");

console.log("Cursor Ultra UI contract PASS");
