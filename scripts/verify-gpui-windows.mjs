#!/usr/bin/env node
// Inspect the linked PE resources rather than trusting build-script success.
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const path = process.argv[2];
if (!path) throw Error('Usage: node scripts/verify-gpui-windows.mjs <metra.exe> [--debug]');
const bytes = readFileSync(path);
assert.equal(bytes.toString('ascii', 0, 2), 'MZ');
const pe = bytes.readUInt32LE(0x3c);
assert.equal(bytes.readUInt32LE(pe), 0x4550);
const optional = pe + 24;
const magic = bytes.readUInt16LE(optional);
assert.ok(magic === 0x20b || magic === 0x10b, 'PE32 or PE32+ required');
const subsystem = bytes.readUInt16LE(optional + 68);
assert.equal(subsystem, process.argv.includes('--debug') ? 3 : 2, 'GUI release must not open a console');
const directories = optional + (magic === 0x20b ? 112 : 96);
const resourceRva = bytes.readUInt32LE(directories + 16);
const sections = optional + bytes.readUInt16LE(pe + 20);
function offset(rva) {
  for (let i = 0; i < bytes.readUInt16LE(pe + 6); i++) {
    const section = sections + i * 40;
    const start = bytes.readUInt32LE(section + 12);
    const length = Math.max(bytes.readUInt32LE(section + 8), bytes.readUInt32LE(section + 16));
    if (rva >= start && rva < start + length) return bytes.readUInt32LE(section + 20) + rva - start;
  }
  throw Error(`Unmapped resource RVA ${rva}`);
}
const root = offset(resourceRva);
const resources = [];
function walk(relative, ids = []) {
  assert.ok(ids.length <= 3, 'Invalid PE resource tree');
  const directory = root + relative;
  const count = bytes.readUInt16LE(directory + 12) + bytes.readUInt16LE(directory + 14);
  for (let i = 0; i < count; i++) {
    const entry = directory + 16 + i * 8;
    const id = bytes.readUInt32LE(entry);
    const target = bytes.readUInt32LE(entry + 4);
    if (target & 0x80000000) walk(target & 0x7fffffff, [...ids, id]);
    else {
      const data = root + target;
      const start = offset(bytes.readUInt32LE(data));
      resources.push({ ids: [...ids, id], bytes: bytes.subarray(start, start + bytes.readUInt32LE(data + 4)) });
    }
  }
}
walk(0);
assert.ok(resources.some(r => r.ids[0] === 3), 'Application icon images missing');
assert.ok(resources.some(r => r.ids[0] === 14 && r.ids[1] === 1), 'GPUI icon group 1 missing');
const version = resources.find(r => r.ids[0] === 16)?.bytes.toString('utf16le');
assert.ok(version?.includes('Metra'), 'Metra version resource missing');
const expected = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8').match(/^version\s*=\s*"([^"]+)"/m)[1];
assert.ok(version.includes(expected), 'Version resource differs from the package');
const manifests = resources.filter(r => r.ids[0] === 24);
assert.equal(manifests.length, 1, 'Exactly one application manifest required');
const manifest = manifests[0].bytes.toString('utf8');
assert.ok(manifest.includes('PerMonitorV2'), 'Per-monitor DPI awareness missing');
assert.ok(manifest.includes('asInvoker'), 'Ordinary-user execution manifest missing');
console.log(`Verified ${path}: icon, version ${expected}, DPI, privilege level, subsystem ${subsystem}`);
