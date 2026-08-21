import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
assert.equal(
  packageJson.scripts["build:macos-universal"],
  "node scripts/build-macos-universal.mjs",
  "package.json must expose one unambiguous Universal macOS build command",
);

const result = spawnSync(process.execPath, [fileURLToPath(new URL("./build-macos-universal.mjs", import.meta.url)), "--dry-run"], {
  encoding: "utf8",
});
assert.equal(result.status, 0, result.stderr || "the Universal build planner must run");
const plan = JSON.parse(result.stdout);
assert.equal(plan.toolchain, "stable");
assert.deepEqual(plan.targets, ["aarch64-apple-darwin", "x86_64-apple-darwin"]);
assert.deepEqual(plan.tauriArgs, ["build", "--target", "universal-apple-darwin"]);
assert.equal(plan.usesRustupToolchain, true);
assert.equal(plan.usesSccache, false);

console.log("Universal macOS build command and rustup toolchain plan verified");
