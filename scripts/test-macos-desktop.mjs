#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

if (process.platform !== 'darwin') throw new Error('Native macOS acceptance requires macOS');
const root = resolve(import.meta.dirname, '..');
const config = mkdtempSync(join(process.env.RUNNER_TEMP || tmpdir(), 'metra-native-'));
const result = spawnSync(join(root, 'target/debug/metra'), ['--desktop-smoke-test'], {
  cwd: root, env: { ...process.env, METRA_CONFIG_DIR: config },
  stdio: 'inherit', timeout: (Number(process.env.METRA_IDLE_SMOKE_SECONDS || 2) + 60) * 1000, killSignal: 'SIGKILL',
});
const reportPath = join(config, 'desktop-smoke.json');
const report = existsSync(reportPath) ? JSON.parse(readFileSync(reportPath, 'utf8')) : null;
if (report) {
  const output = join(root, 'test-results/native-macos');
  mkdirSync(output, { recursive: true });
  copyFileSync(reportPath, join(output, 'desktop-smoke.json'));
}
if (result.error || result.status !== 0 || !report?.ok || !report.nativeTemplate) {
  throw new Error(`Native macOS acceptance failed: ${result.error?.message || JSON.stringify(report)}`);
}
console.log(JSON.stringify(report, null, 2));
