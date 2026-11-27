; Metra and older /S-only clients use this restart hook. The bundler handles
; restart for published clients that also pass /R.
!macro NSIS_HOOK_POSTINSTALL
  ${If} ${Silent}
    ClearErrors
    ${GetOptions} $CMDLINE "/R" $R0
    ${If} ${Errors}
      nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" ""
    ${EndIf}
  ${EndIf}
!macroend
