#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync, existsSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
const root = resolve(import.meta.dirname, '..');
const config = mkdtempSync(join(tmpdir(), 'metra-gpui-smoke-'));
const legacy = process.argv.includes('--legacy-status');
if (legacy) writeFileSync(join(config, 'settings.json'), JSON.stringify({ displayMode: 'statusBar', bubbleProviderOrder: ['codex', 'cursor', 'claude'], bubbleVisibleProviders: ['codex', 'cursor'], bubblePercentMode: 'used', cursorBubbleLabel: 'CU', autostart: false }));
const executable = resolve(process.argv.slice(2).find(arg => !arg.startsWith('--')) || join(root, 'desktop-gpui/target/debug', process.platform === 'win32' ? 'metra-gpui.exe' : 'metra-gpui'));
const child = spawn(executable, [], {
  cwd: root,
  env: { ...process.env, METRA_CONFIG_DIR: config, METRA_GPUI_SMOKE: '1', ...(legacy ? { METRA_GPUI_MIGRATION_SMOKE: '1', METRA_GPUI_DEMO: '1' } : {}) },
  stdio: ['ignore', 'pipe', 'pipe'],
});
let output = '';
child.stdout.on('data', data => { output += data; });
child.stderr.on('data', data => { output += data; });
const timer = setTimeout(() => child.kill('SIGKILL'), 45000);
try {
  const code = await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('close', (code, signal) => resolve({ code, signal }));
  });
  const resultPath = join(config, 'native-smoke.result');
  const result = existsSync(resultPath) ? readFileSync(resultPath, 'utf8') : '';
  if (code.code !== 0 || !result.startsWith('PASS:')) {
    throw Error(`Native smoke failed: ${JSON.stringify(code)}; configuration: ${config}\n${output}`);
  }
  console.log(result);
  rmSync(config, { recursive: true });
} finally {
  clearTimeout(timer);
}
