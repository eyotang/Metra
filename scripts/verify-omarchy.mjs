import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runInNewContext } from "node:vm";
import { spawn } from "node:child_process";

const model = runInNewContext(readFileSync(new URL("../integrations/omarchy/metra.usage/Model.js", import.meta.url), "utf8")
  + ";({providers, remaining, summary, headline, reset, costs, tokens})");
const snapshot = {
  cursor: { provider: "cursor", quotas: [] },
  codex: { provider: "codex", quotas: [{ remainingPercent: 70 }, { remainingPercent: 20 }] },
  claude: { provider: "claude", quotas: [{ remainingPercent: 0 }] },
};
const payload = { snapshot, settings: { bubbleProviderOrder: ["codex", "cursor", "claude"], bubbleVisibleProviders: ["cursor", "codex", "claude"] } };
assert.equal(model.summary(payload), "X 20%  ·  C —  ·  A 0%", "minimum window, no-quota and exhausted must be distinct");
assert.equal(model.remaining({ quotas: [{ remainingPercent: null }, { remainingPercent: NaN }] }), null);
assert.equal(model.summary({ ...payload, settings: { bubbleProviderOrder: ["claude", "codex"], bubbleVisibleProviders: ["codex"] } }), "X 20%");
assert.equal(model.reset("2026-01-01T00:00:00Z", Date.parse("2026-01-02T00:00:00Z")), "已到重置时间，等待刷新");
assert.equal(model.reset(null, Date.now()), "");
assert.equal(model.tokens(null), "—");
assert.equal(model.tokens(0), "0");
assert.equal(model.costs({ currency: "USD", todayUsedCents: 0 }), "今日费用 $0.00");
assert.equal(model.headline(snapshot.codex), "X 20%");
assert.equal(model.headline(snapshot.cursor), "C —");
assert.equal(model.headline(null), "AI …");
console.log("Omarchy presentation: PASS");

// Optional real executable tests use an empty HOME/PATH and no display, so no
// account is contacted and accidentally initializing GTK fails immediately.
if (process.platform === "linux" && process.argv[2]) {
  const home = mkdtempSync(join(tmpdir(), "metra-headless-test-"));
  async function collect(mode, stream) {
    const child = spawn(process.argv[2], [mode], {
      env: { HOME: home, XDG_CONFIG_HOME: home, PATH: home },
      stdio: ["pipe", "pipe", "pipe"],
    });
    const timer = setTimeout(() => child.kill("SIGKILL"), 15000);
    let output = "";
    let error = "";
    let requestedRefresh = false;
    child.stdout.on("data", data => {
      output += data;
      const lines = output.split("\n").length - 1;
      if (stream && lines >= 2 && !requestedRefresh) {
        requestedRefresh = true;
        child.stdin.write("refresh\n");
      }
      if (stream && lines >= 3) child.stdin.end();
    });
    child.stderr.on("data", data => { error += data; });
    if (!stream) child.stdin.end();
    await new Promise((resolve, reject) => {
      child.on("error", reject);
      child.on("close", (code, signal) => {
        clearTimeout(timer);
        if (code !== 0) reject(new Error(`${mode}: ${code || signal}: ${error}`));
        else resolve();
      });
    });
    return output.trim().split("\n").map(line => JSON.parse(line));
  }
  try {
    const once = await collect("--usage-json", false);
    assert.equal(once.length, 1);
    assert.equal(once[0].snapshot.refreshing, false);
    assert.ok(once[0].settings);
    const stream = await collect("--watch-json", true);
    assert.equal(stream[0].snapshot.refreshing, true);
    assert.equal(stream[1].snapshot.refreshing, false);
    assert.equal(stream[2].snapshot.refreshing, false, "throttled refresh must acknowledge the current payload");
    assert.equal(stream[1].snapshot.codex.quotas.length, 0, "missing account cannot fabricate allowance");
    console.log("Headless JSON, no display, watch lifecycle and stdin EOF: PASS");
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
}
