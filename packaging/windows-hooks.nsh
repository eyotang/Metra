; v0.4.1 and earlier invoked updates with /S only. Keep their automatic restart
; when migrating to Tauri; /R clients are restarted once by Tauri's own handler.
!macro NSIS_HOOK_POSTINSTALL
  ${If} ${Silent}
    ClearErrors
    ${GetOptions} $CMDLINE "/R" $R0
    ${If} ${Errors}
      nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" ""
    ${EndIf}
  ${EndIf}
!macroend
