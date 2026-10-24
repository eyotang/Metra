#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';

const root = resolve(import.meta.dirname, '..');
const version = readFileSync(join(root,'Cargo.toml'),'utf8').match(/^version\s*=\s*"([^"]+)"/m)[1];
const option = name => { const index=process.argv.indexOf(name); if(index<0)return null; if(!process.argv[index+1]||process.argv[index+1].startsWith('--'))throw new Error(`${name} needs a path`);return resolve(process.argv[index+1]); };
const output = option('--output') || join(root,'release-assets');
const binaryOverride = option('--binary');
const noBuild = process.argv.includes('--no-build');
const universal = process.argv.includes('--universal');
const run = (command,args) => { const result=spawnSync(command,args,{cwd:root,stdio:'inherit'}); if(result.error||result.status!==0)throw new Error(result.error?.message||`${command} failed (${result.status})`); };
mkdirSync(output,{recursive:true});
const staging=mkdtempSync(join(tmpdir(),'metra-package-'));
try {
  let binary=binaryOverride||join(root,'target/release',process.platform==='win32'?'metra.exe':'metra');
  if (process.platform==='darwin'&&universal) {
    const targets=['aarch64-apple-darwin','x86_64-apple-darwin'];
    if(!noBuild){run('rustup',['target','add',...targets]);for(const target of targets)run('cargo',['build','--release','--locked','--target',target]);}
    binary=join(staging,'metra');
    run('lipo',['-create',...targets.map(target=>join(root,'target',target,'release/metra')),'-output',binary]);
    run('lipo',[binary,'-verify_arch','arm64','x86_64']);
  } else if(!noBuild)run('cargo',['build','--release','--locked']);
  if(process.platform==='darwin') {
    const app=join(staging,'Metra.app');
    mkdirSync(join(app,'Contents/MacOS'),{recursive:true});mkdirSync(join(app,'Contents/Resources'),{recursive:true});
    copyFileSync(binary,join(app,'Contents/MacOS/metra'));copyFileSync(join(root,'assets/icons/icon.icns'),join(app,'Contents/Resources/icon.icns'));
    writeFileSync(join(app,'Contents/Info.plist'),`<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleName</key><string>Metra</string><key>CFBundleDisplayName</key><string>Metra</string><key>CFBundleIdentifier</key><string>dev.metra.desktop</string><key>CFBundleExecutable</key><string>metra</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleShortVersionString</key><string>${version}</string><key>CFBundleVersion</key><string>${version}</string><key>CFBundleIconFile</key><string>icon.icns</string><key>LSUIElement</key><true/><key>NSHighResolutionCapable</key><true/><key>LSMinimumSystemVersion</key><string>13.0</string></dict></plist>`);
    run('plutil',['-lint',join(app,'Contents/Info.plist')]);run('codesign',['--force','--sign','-',app]);
    symlinkSync('/Applications',join(staging,'Applications'));
    const arch=universal?'universal':process.arch;
    const dmg=join(output,`Metra-${version}-macos-${arch}.dmg`);
    run('hdiutil',['create','-volname','Metra','-srcfolder',staging,'-ov','-format','UDZO',dmg]);
    run('hdiutil',['verify',dmg]);checksum(dmg);
  } else if(process.platform==='win32') {
    copyFileSync(binary,join(output,`Metra-${version}-windows-x64-portable.exe`));
    run(process.env.MAKENSIS||'makensis',[`/DVERSION=${version}`,`/DBINARY=${binary}`,`/DOUTFILE=${join(output,`Metra-${version}-windows-x64-setup.exe`)}`,join(root,'packaging/windows.nsi')]);
  } else if(process.platform==='linux') {
    const arch=process.arch==='x64'?'amd64':'arm64';
    mkdirSync(join(staging,'DEBIAN'),{recursive:true});mkdirSync(join(staging,'usr/bin'),{recursive:true});mkdirSync(join(staging,'usr/share/applications'),{recursive:true});mkdirSync(join(staging,'usr/share/icons/hicolor/128x128/apps'),{recursive:true});
    copyFileSync(binary,join(staging,'usr/bin/metra'));
    copyFileSync(join(root,'assets/icons/128x128.png'),join(staging,'usr/share/icons/hicolor/128x128/apps/metra.png'));
    writeFileSync(join(staging,'usr/share/applications/dev.metra.desktop'),`[Desktop Entry]\nName=Metra\nComment=AI usage monitor\nExec=metra\nIcon=metra\nTerminal=false\nType=Application\nCategories=Utility;Monitor;\nStartupWMClass=dev.metra.desktop\n`);
    writeFileSync(join(staging,'DEBIAN/control'),`Package: metra\nVersion: ${version}\nSection: utils\nPriority: optional\nArchitecture: ${arch}\nMaintainer: eyotang\nDepends: libgtk-3-0, libayatana-appindicator3-1, libvulkan1, libxkbcommon0, libfontconfig1\nDescription: Native GPUI Kit monitor for Cursor, Codex and Claude Code usage\n`);
    const deb=join(output,`Metra-${version}-linux-${process.arch==='x64'?'x64':'arm64'}.deb`);
    run('dpkg-deb',['--build','--root-owner-group',staging,deb]);checksum(deb);
  } else throw new Error('Unsupported packaging platform');
}finally{rmSync(staging,{recursive:true,force:true});}
function checksum(path){const name=path.split(/[\\/]/).at(-1);writeFileSync(path+'.sha256',`${createHash('sha256').update(readFileSync(path)).digest('hex')}  ${name}\n`);}
