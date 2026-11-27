#!/usr/bin/env node
// Exercise a published binary and the candidate against the same isolated SQLite settings.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const [published, candidate] = process.argv.slice(2);
assert.ok(published && candidate, 'Usage: test-packaged-settings.mjs <published executable> <candidate executable>');
const profile = mkdtempSync(join(tmpdir(), 'metra-packaged-settings-'));
const legacy = join(profile, 'settings.json');
const expected = {
  autostart: false,
  bubbleProviderOrder: ['claude', 'cursor', 'codex'],
  bubbleVisibleProviders: ['cursor', 'claude'],
  bubblePercentMode: 'used',
  bubbleSnapEnabled: false,
  bubblePosition: { x: 130, y: 240 },
  bubblePositionVersion: 2,
  claudeBubbleColor: '#ff6b6b',
  cursorBubbleLabel: 'CU',
  cursorCompatEnabled: false,
  displayMode: 'statusBar',
  refreshMinutes: 15,
  uiLanguage: 'ja',
};
function usage(executable) {
  const result = JSON.parse(execFileSync(resolve(executable), ['--usage-json'], {
    cwd: profile,
    env: {
      HOME: profile, USERPROFILE: profile, APPDATA: profile, LOCALAPPDATA: profile,
      XDG_CONFIG_HOME: profile, XDG_DATA_HOME: profile, PATH: profile, METRA_CONFIG_DIR: profile,
      ...(process.env.SystemRoot ? { SystemRoot: process.env.SystemRoot } : {}),
    },
    encoding: 'utf8', timeout: 20000,
  }));
  assert.equal(result.snapshot.refreshing, false);
  for (const provider of ['cursor', 'codex', 'claude']) {
    const supplied = result.snapshot[provider];
    assert.equal(supplied.provider, provider);
    assert.deepEqual(supplied.quotas, [], `${provider} fabricated a quota for an empty profile`);
  }
  for (const [key, value] of Object.entries(expected)) {
    assert.deepEqual(result.settings[key], value, `${executable} changed ${key}`);
  }
  return result.settings;
}
try {
  writeFileSync(legacy, `${JSON.stringify(expected)}\n`);
  const before = usage(published);
  const database = readFileSync(join(profile, 'settings.db'));
  assert.equal(database.subarray(0, 16).toString(), 'SQLite format 3\0');
  rmSync(legacy);
  assert.deepEqual(usage(candidate), before);
  assert.deepEqual(readFileSync(join(profile, 'settings.db')), database);
  console.log('Published configuration recovered from SQLite by candidate: PASS');
} finally {
  rmSync(profile, { recursive: true, force: true });
}
