#!/usr/bin/env node
// Use Tauri's platform bundlers, retaining the public release names and updater contract.
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve, basename } from 'node:path';
import { createHash } from 'node:crypto';
const root = resolve(import.meta.dirname, '..');
const version = readFileSync(join(root, 'Cargo.toml'), 'utf8').match(/^version\s*=\s*"([^"]+)"/m)[1];
const arg = name => {
  const i = process.argv.indexOf(name);
  if (i < 0) return undefined;
  if (!process.argv[i + 1] || process.argv[i + 1].startsWith('--')) throw new Error(`${name} requires a value`);
  return process.argv[i + 1];
};
const output = resolve(arg('--output') || join(root, 'release-assets'));
const universal = process.argv.includes('--universal');
if (universal && process.platform !== 'darwin') throw new Error('--universal requires macOS');
if (process.argv.includes('--binary') || process.argv.includes('--no-build')) throw new Error('Use Tauri bundles; arbitrary binary packaging is no longer supported.');
const bundle = { darwin: 'dmg', win32: 'nsis', linux: 'deb' }[process.platform];
if (!bundle) throw new Error('Unsupported packaging platform');
const args = [join(root, 'node_modules/@tauri-apps/cli/tauri.js'), 'build', '--ci', '--bundles', bundle];
if (universal) args.push('--target', 'universal-apple-darwin');
args.push('--', '--locked');
const result = spawnSync(process.execPath, args, { cwd: root, stdio: 'inherit' });
if (result.error || result.status !== 0) throw new Error(result.error?.message || `Tauri packaging failed (${result.status})`);
const target = resolve(process.env.CARGO_TARGET_DIR || join(root, 'target'), ...(universal ? ['universal-apple-darwin'] : []), 'release');
const bundleDir = join(target, 'bundle', bundle);
const candidates = readdirSync(bundleDir).filter(name => name.endsWith(bundle === 'nsis' ? '-setup.exe' : `.${bundle}`) && name.includes(version));
if (candidates.length !== 1) throw new Error(`Expected one ${bundle} for ${version}, found ${candidates.length}`);
mkdirSync(output, { recursive: true });
const platform = { darwin: 'macos', win32: 'windows', linux: 'linux' }[process.platform];
const arch = universal ? 'universal' : process.arch;
const filename = `Metra-${version}-${platform}-${arch}${bundle === 'nsis' ? '-setup.exe' : `.${bundle}`}`;
const destination = join(output, filename);
copyFileSync(join(bundleDir, candidates[0]), destination);
if (bundle === 'nsis') copyFileSync(join(target, 'metra.exe'), join(output, `Metra-${version}-windows-${arch}-portable.exe`));
else writeFileSync(destination + '.sha256', `${createHash('sha256').update(readFileSync(destination)).digest('hex')}  ${basename(destination)}\n`);
