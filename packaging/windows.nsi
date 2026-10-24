Unicode True
!include "MUI2.nsh"
Name "Metra"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Metra"
RequestExecutionLevel user
SetCompressor /SOLID lzma
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Section "Metra"
  IfSilent 0 +2
    Sleep 1000
  SetOutPath "$INSTDIR"
  File /oname=metra.exe "${BINARY}"
  CreateShortcut "$SMPROGRAMS\Metra.lnk" "$INSTDIR\metra.exe"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Metra" "DisplayName" "Metra"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Metra" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Metra" "UninstallString" '$"$INSTDIR\Uninstall.exe$"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Metra" "InstallLocation" "$INSTDIR"
  IfSilent 0 +2
    Exec '"$INSTDIR\metra.exe"'
SectionEnd
Section "Uninstall"
  Delete "$INSTDIR\metra.exe"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\Metra.lnk"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Metra"
  RMDir "$INSTDIR"
SectionEnd
