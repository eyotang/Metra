#!/usr/bin/env node
// Build the native shell, then reuse the established installer metadata and hooks.
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { resolve, join, basename } from 'node:path';
import { createHash } from 'node:crypto';
import { packageLinux } from './package-gpui-linux.mjs';
const root=resolve(import.meta.dirname,'..');
const { APPLE_CERTIFICATE, APPLE_CERTIFICATE_PASSWORD, ...buildEnv } = process.env;
if (process.platform === 'darwin' && APPLE_CERTIFICATE && APPLE_CERTIFICATE_PASSWORD === undefined) {
  throw Error('Set APPLE_CERTIFICATE_PASSWORD for the signing certificate');
}
if(process.argv.includes('--binary') || process.argv.includes('--no-build')) throw Error('Native packaging builds and verifies its own executable.');
const debug=process.argv.includes('--debug');
const universal=process.argv.includes('--universal');
const outputIndex=process.argv.indexOf('--output');
if(outputIndex>=0 && (!process.argv[outputIndex+1] || process.argv[outputIndex+1].startsWith('--'))) throw Error('--output requires a path');
const output=resolve(outputIndex>=0?process.argv[outputIndex+1]:join(root,'release-assets'));
const version=readFileSync(join(root,'Cargo.toml'),'utf8').match(/^version\s*=\s*"([^"]+)"/m)[1];
const nativeVersion=readFileSync(join(root,'desktop-gpui/Cargo.toml'),'utf8').match(/^version\s*=\s*"([^"]+)"/m)[1];
if(version!==nativeVersion) throw Error('Native and core versions must match');
if(universal && process.platform!=='darwin') throw Error('--universal requires macOS');
const bundle={linux:'deb',darwin:'dmg',win32:'nsis'}[process.platform];
if(!bundle) throw Error('Unsupported platform');
const profile=debug?'debug':'release';
const nativeTarget=resolve(process.env.CARGO_TARGET_DIR || join(root,'desktop-gpui/target'));
const staging=join(root,'target/gpui-package');
const targetName=universal?'universal-apple-darwin':null;
const stagedProfile=join(staging,...(targetName?[targetName]:[]),profile);
mkdirSync(stagedProfile,{recursive:true});
const suffix=process.platform==='win32'?'.exe':'';
function run(command,args,env=buildEnv){
 const result=spawnSync(command,args,{cwd:root,env,stdio:'inherit'});
 if(result.error || result.status!==0) throw Error(result.error?.message || `${command} failed (${result.status})`);
}
const targets=universal?['aarch64-apple-darwin','x86_64-apple-darwin']:[null];
const binaries=[];
for(const target of targets){
 const args=['build','--locked','--manifest-path','desktop-gpui/Cargo.toml','--target-dir',nativeTarget];
 if(!debug) args.push('--release');
 if(target) args.push('--target',target);
 run('cargo',args);
 binaries.push(join(nativeTarget,...(target?[target]:[]),profile,`metra-gpui${suffix}`));
}
const binary=join(stagedProfile,`metra${suffix}`);
if(universal) run('lipo',['-create',...binaries,'-output',binary]);else copyFileSync(binaries[0],binary);
if(process.platform==='win32') run(process.execPath,[join(root,'scripts/verify-gpui-windows.mjs'),binary,...(debug?['--debug']:[])]);
if(process.platform==='linux') {
 mkdirSync(output,{recursive:true});
 const destination=join(output,`Metra-${version}-linux-${process.arch}.deb`);
 packageLinux({root,binary,version,destination,arch:process.arch});
 writeFileSync(`${destination}.sha256`,`${createHash('sha256').update(readFileSync(destination)).digest('hex')}  ${basename(destination)}\n`);
 console.log(destination);process.exit(0);
}
// Tauri selects the identity from an imported PKCS#12. Passing "-" here
// would explicitly request ad-hoc signing and conflict with the certificate.
const signingIdentity = buildEnv.APPLE_SIGNING_IDENTITY || (APPLE_CERTIFICATE ? undefined : '-');
const config = {
  build: { beforeBuildCommand: null, beforeBundleCommand: null },
  bundle: {
    macOS: { signingIdentity },
    windows: { webviewInstallMode: { type: 'skip', silent: null } },
    linux: { deb: { depends: ['libfontconfig1', 'libxkbcommon0', 'libxkbcommon-x11-0', 'libvulkan1'] } },
  },
};
const args=[join(root,'node_modules/@tauri-apps/cli/tauri.js'),'bundle','--ci','--no-binary-patching','--bundles',process.platform==='darwin'?'app,dmg':bundle,'--config',JSON.stringify(config)];
if(debug) args.push('--debug');
if(universal) args.push('--target',targetName);
const bundleEnv = { ...buildEnv, CARGO_TARGET_DIR: staging };
if (process.platform === 'darwin' && APPLE_CERTIFICATE) {
  Object.assign(bundleEnv, { APPLE_CERTIFICATE, APPLE_CERTIFICATE_PASSWORD });
}
run(process.execPath,args,bundleEnv);
if(process.platform==='darwin') {
 const app=join(stagedProfile,'bundle/macos/Metra.app');
 run('/usr/bin/codesign',['--verify','--deep','--strict',app]);
 const info=join(app,'Contents/Info.plist');
 const property=key=> {
  const result=spawnSync('/usr/bin/plutil',['-extract',key,'raw','-o','-',info],{encoding:'utf8',env:buildEnv});
  if(result.status!==0) throw Error(`Missing macOS property ${key}`);
  return result.stdout.trim();
 };
 if(property('CFBundleIdentifier')!=='dev.metra.desktop' || property('CFBundleExecutable')!=='metra' || property('CFBundleShortVersionString')!==version || property('LSUIElement')!=='true') throw Error('Native app metadata differs from the installed Metra identity');
 run('/usr/bin/lipo',[join(app,'Contents/MacOS/metra'),'-verify_arch',...(universal?['arm64','x86_64']:[process.arch==='arm64'?'arm64':'x86_64'])]);
}
const directory=join(stagedProfile,'bundle',bundle);
const files=readdirSync(directory).filter(name=>name.includes(version) && name.endsWith(bundle==='nsis'?'-setup.exe':`.${bundle}`));
if(files.length!==1) throw Error(`Expected one ${bundle}, found ${files.length}`);
mkdirSync(output,{recursive:true});
const platform={linux:'linux',darwin:'macos',win32:'windows'}[process.platform];
const arch=universal?'universal':process.arch;
const destination=join(output,`Metra-${version}-${platform}-${arch}${bundle==='nsis'?'-setup.exe':`.${bundle}`}`);
copyFileSync(join(directory,files[0]),destination);
if(process.platform==='darwin') run('/usr/bin/hdiutil',['verify',destination]);
if(bundle==='nsis') copyFileSync(binary,join(output,`Metra-${version}-windows-${arch}-portable.exe`));
else writeFileSync(`${destination}.sha256`,`${createHash('sha256').update(readFileSync(destination)).digest('hex')}  ${basename(destination)}\n`);
console.log(destination);
