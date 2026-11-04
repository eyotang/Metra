#!/usr/bin/env node
// Create once; reuse the private key in the login Keychain for future builds.
import { randomBytes } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

if (process.platform !== 'darwin') throw new Error('Requires macOS');
const name = 'Metra Local Development';
function run(command, args, env = process.env) {
  const r = spawnSync(command, args, { encoding: 'utf8', env });
  if (r.error || r.status !== 0) throw new Error(`${command}: ${r.error?.message || r.stderr}`);
  return r.stdout;
}
if (run('/usr/bin/security', ['find-certificate', '-a', '-c', name, '-Z']).includes('SHA-1 hash:')) {
  console.log(`${name} already exists; keeping the existing identity.`);
  process.exit(0);
}
const directory = mkdtempSync(join(tmpdir(), 'metra-signing-'));
try {
  const config = join(directory, 'certificate.cnf');
  writeFileSync(config, `[req]\nprompt = no\ndistinguished_name = dn\nx509_extensions = extensions\n[dn]\nCN = ${name}\n[extensions]\nbasicConstraints = critical,CA:false\nkeyUsage = critical,digitalSignature\nextendedKeyUsage = critical,codeSigning\n`, { mode: 0o600 });
  const key = join(directory, 'key.pem');
  const cert = join(directory, 'certificate.pem');
  const bundle = join(directory, 'identity.p12');
  run('/usr/bin/openssl', ['req', '-new', '-newkey', 'rsa:2048', '-nodes', '-x509', '-days', '3650', '-config', config, '-keyout', key, '-out', cert]);
  const password = randomBytes(32).toString('hex');
  run('/usr/bin/openssl', ['pkcs12', '-export', '-inkey', key, '-in', cert, '-name', name, '-out', bundle, '-keypbe', 'PBE-SHA1-3DES', '-certpbe', 'PBE-SHA1-3DES', '-macalg', 'sha1', '-passout', 'env:METRA_TEMP_SIGNING_PASSWORD'], { ...process.env, METRA_TEMP_SIGNING_PASSWORD: password });
  const keychain = run('/usr/bin/security', ['default-keychain', '-d', 'user']).trim().replace(/^"|"$/g, '');
  run('/usr/bin/security', ['import', bundle, '-k', keychain, '-P', password, '-T', '/usr/bin/codesign']);
  console.log(`Created ${name} in the default user Keychain. Temporary private-key files removed. This identity is for local installs, not public distribution.`);
} finally {
  rmSync(directory, { recursive: true, force: true });
}
