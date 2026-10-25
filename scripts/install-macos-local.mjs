#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, renameSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

if (process.platform !== 'darwin') throw new Error('Local macOS installation requires macOS');
const root = resolve(import.meta.dirname, '..');
const source = resolve(process.argv[2] || join(root, 'target/release/bundle/macos/Metra.app'));
const destination = '/Applications/Metra.app';
const id = randomUUID();
const staging = `/Applications/.Metra-install-${id}.app`;
const backup = join(root, 'release-assets', `local-install-backup-${Date.now()}-${id}`, 'Metra.app');
const executable = 'Contents/MacOS/metra';
const processPattern = '^/Applications/Metra[.]app/Contents/MacOS/metra([[:space:]]|$)';
function run(command, args) {
  const result = spawnSync(command, args, { encoding: 'utf8' });
  if (result.error || result.status !== 0) throw new Error(`${command}: ${result.error?.message || result.stderr || result.stdout}`);
  return result.stdout;
}
const digest = path => createHash('sha256').update(readFileSync(path)).digest('hex');
if (!existsSync(join(source, executable))) throw new Error(`Build Metra.app before installing: ${source}`);
run('/usr/bin/ditto', [source, staging]);
if (digest(join(source, executable)) !== digest(join(staging, executable))) throw new Error('Staged executable does not match the build');
run('/usr/bin/xattr', ['-cr', staging]);
// Local unsigned bundles may carry only the linker's incomplete bundle signature.
if (spawnSync('/usr/bin/codesign', ['--verify', '--deep', '--strict', staging]).status !== 0) {
  run('/usr/bin/codesign', ['--force', '--deep', '--sign', '-', staging]);
}
run('/usr/bin/codesign', ['--verify', '--deep', '--strict', staging]);
const installedDigest = digest(join(staging, executable));
const stopped = spawnSync('/usr/bin/pkill', ['-x', '-f', processPattern]);
if (stopped.error || ![0, 1].includes(stopped.status)) throw new Error('Could not stop the installed Metra process');
for (let attempt = 0; ; attempt++) {
  const active = spawnSync('/usr/bin/pgrep', ['-f', processPattern]);
  if (active.status === 1) break;
  if (active.error || active.status !== 0 || attempt === 49) throw new Error('Installed Metra did not stop');
  await new Promise(resolve => setTimeout(resolve, 100));
}
mkdirSync(dirname(backup), { recursive: true });
if (existsSync(destination)) renameSync(destination, backup);
try {
  renameSync(staging, destination);
  run('/usr/bin/xattr', ['-cr', destination]);
  if (run('/usr/bin/xattr', ['-lr', destination]).includes('com.apple.quarantine')) throw new Error('Quarantine attribute remains after installation');
  run('/usr/bin/codesign', ['--verify', '--deep', '--strict', destination]);
  if (digest(join(destination, executable)) !== installedDigest) throw new Error('Installed executable does not match staging');
} catch (error) {
  if (existsSync(destination)) renameSync(destination, staging);
  if (existsSync(backup)) renameSync(backup, destination);
  throw error;
}
console.log(`Installed Metra; xattr cleared, quarantine absent, signature verified. Settings preserved. Backup: ${backup}`);
