import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, writeFileSync, rmSync, chmodSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
export function packageLinux({root,binary,version,destination,arch}) {
 const directory=mkdtempSync(join(tmpdir(),'metra-native-deb-'));
 function run(command,args){const result=spawnSync(command,args,{cwd:directory,stdio:'inherit'});if(result.error || result.status!==0)throw Error(result.error?.message||`${command} failed`);}
 try {
  const data=join(directory,'data'),control=join(directory,'control');mkdirSync(control);
  for(const path of ['usr/bin','usr/share/applications','usr/share/icons/hicolor/128x128/apps','usr/share/doc/metra'])mkdirSync(join(data,path),{recursive:true});
  copyFileSync(binary,join(data,'usr/bin/metra'));chmodSync(join(data,'usr/bin/metra'),0o755);
  copyFileSync(join(root,'assets/icons/128x128.png'),join(data,'usr/share/icons/hicolor/128x128/apps/metra.png'));
  copyFileSync(join(root,'assets/THIRD_PARTY_NOTICES.txt'),join(data,'usr/share/doc/metra/THIRD_PARTY_NOTICES.txt'));
  copyFileSync(join(root,'LICENSE'),join(data,'usr/share/doc/metra/copyright'));
  writeFileSync(join(data,'usr/share/applications/metra.desktop'),'[Desktop Entry]\nType=Application\nName=Metra\nComment=AI coding usage monitor\nExec=metra\nIcon=metra\nTerminal=false\nCategories=Utility;\nStartupWMClass=dev.metra.desktop\n');
  const debArch={x64:'amd64',arm64:'arm64'}[arch];if(!debArch)throw Error(`Unsupported Debian architecture: ${arch}`);
  writeFileSync(join(control,'control'),`Package: metra\nVersion: ${version}\nArchitecture: ${debArch}\nMaintainer: eyotang\nSection: utils\nPriority: optional\nInstalled-Size: ${Math.ceil(statSync(binary).size/1024)+128}\nDepends: libc6, libgcc-s1, libxcb1, libxkbcommon0, libxkbcommon-x11-0, libfontconfig1, libvulkan1\nDescription: Cursor, Codex and Claude Code usage monitor\n Native desktop companion built with GPUI.\n`);
  writeFileSync(join(directory,'debian-binary'),'2.0\n');
  run('tar',['--owner=0','--group=0','--numeric-owner','-czf',join(directory,'control.tar.gz'),'-C',control,'.']);
  run('tar',['--owner=0','--group=0','--numeric-owner','-czf',join(directory,'data.tar.gz'),'-C',data,'.']);
  run('ar',['rcD','package.deb','debian-binary','control.tar.gz','data.tar.gz']);
  copyFileSync(join(directory,'package.deb'),destination);
 } finally {rmSync(directory,{recursive:true,force:true});}
}
