import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const workspace = readFileSync(new URL("../pnpm-workspace.yaml", import.meta.url), "utf8");
const cargoConfig = readFileSync(new URL("../.cargo/config.toml", import.meta.url), "utf8");
const portableBuild = readFileSync(new URL("./build-portable.ps1", import.meta.url), "utf8");
const renderedBubbleCheck = readFileSync(new URL("./verify-rendered-bubble.ps1", import.meta.url), "utf8");
const ciWorkflow = readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
const readmes = ["README.md", "README.zh-CN.md", "README.ja.md", "README.ko.md"].map((name) =>
  readFileSync(new URL(`../${name}`, import.meta.url), "utf8"),
);

assert.match(
  workspace,
  /^packages:\s*\n(?:[ \t]+#.*\n)*[ \t]+-[ \t]+["']?\.["']?[ \t]*$/m,
  "pnpm-workspace.yaml must explicitly include the root package for cross-version Windows support",
);
assert.match(cargoConfig, /rustc-wrapper\s*=\s*""/, "project builds must disable any user-level rustc wrapper such as sccache");
assert.match(
  cargoConfig,
  /rustc-workspace-wrapper\s*=\s*""/,
  "project builds must disable any user-level workspace rustc wrapper",
);
assert.equal(packageJson.packageManager, "pnpm@11.22.0", "package.json must pin the tested pnpm version");
assert.equal(packageJson.engines?.node, ">=22.13.0", "pnpm 11.22 requires the documented Node 22.13+ baseline");
for (const readme of readmes) {
  assert.match(readme, /Node(?:\.js)? 22\.13(?:\+| 以降| 이상)/, "every README must document the supported Node baseline");
  assert.match(readme, /npm install --global pnpm@11\.22\.0/, "every README must install the pinned Windows pnpm version");
}
assert.equal(
  packageJson.scripts["build:portable"],
  "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-portable.ps1",
  "the portable Windows command must invoke the checked-in PowerShell build",
);
assert.doesNotMatch(renderedBubbleCheck, /TickCount64/, "Windows PowerShell 5.1 does not expose Environment.TickCount64");
assert.match(renderedBubbleCheck, /Diagnostics\.Stopwatch/, "the Windows render timeout must use a PowerShell 5.1-compatible monotonic clock");
assert.match(renderedBubbleCheck, /\.A\s+-gt\s+0/, "the rendered bubble check must reject fully transparent pixels");
assert.match(portableBuild, /Get-Process[\s\S]*ProcessName[\s\S]*Metra/, "portable builds must detect a running single-instance Metra before smoke testing");
assert.ok(
  (portableBuild.match(/Assert-MetraNotRunning/g) ?? []).length >= 3,
  "portable builds must check for Metra before the build and again immediately before copying",
);
assert.match(portableBuild, /CARGO_TARGET_DIR/, "portable builds must use an absolute Cargo target directory");
assert.match(portableBuild, /rustc -vV/, "portable builds must derive the native Rust target from rustc");
assert.match(portableBuild, /--target \$hostTarget/, "portable builds must pass the native Rust target to Tauri");
assert.match(renderedBubbleCheck, /EndOfMessage/, "the DevTools client must reassemble fragmented WebSocket messages");
assert.match(
  renderedBubbleCheck,
  /document\.querySelector\(['"]\.bubble-shell['"]\)/,
  "the portable smoke test must wait for the bubble DOM before taking a screenshot",
);
assert.match(renderedBubbleCheck, /requestAnimationFrame/, "the portable smoke test must wait for a painted frame");
assert.match(renderedBubbleCheck, /GetWindowThreadProcessId/, "the Windows click smoke test must only inspect windows owned by Metra");
assert.match(
  renderedBubbleCheck,
  /GetPhysicalCursorPos[\s\S]*SetPhysicalCursorPos[\s\S]*WindowFromPhysicalPoint/,
  "the Windows click smoke test must use one physical coordinate space on mixed-DPI desktops",
);
assert.match(renderedBubbleCheck, /SendInput/, "the Windows click smoke test must send native mouse input instead of a synthetic DOM click");
assert.match(
  renderedBubbleCheck,
  /MouseEventLeftDown[\s\S]*MouseEventLeftUp/,
  "the Windows click smoke test must send a complete left-button gesture",
);
assert.match(
  renderedBubbleCheck,
  /\$panelWindow[\s\S]*\.Handle\s+-ne\s+\$bubbleWindow\.Handle[\s\S]*\.Width\s+-ge\s+\$minimumPanelWidth/,
  "the Windows click smoke test must wait for a distinct, visible panel-sized window",
);
assert.match(
  renderedBubbleCheck,
  /Start-Sleep\s+-Milliseconds\s+400[\s\S]*\.Handle\s+-eq\s+\$panelWindow\.Handle[\s\S]*\$stablePanelWindow/,
  "the Windows click smoke test must reject a panel that is immediately toggled closed again",
);
assert.match(
  renderedBubbleCheck,
  /Stop-Process[^\r\n]*-ErrorAction\s+SilentlyContinue/,
  "smoke-test cleanup must tolerate the app exiting between process checks",
);
assert.match(
  ciWorkflow,
  /Verify rendered Windows bubble[\s\S]*verify-rendered-bubble\.ps1[\s\S]*src-tauri\/target\/release\/metra\.exe/,
  "Windows release artifacts must run the rendered-bubble smoke test in isolated CI",
);

console.log("pnpm workspace and portable Windows command verified");
