!ifdef MUI_INCLUDED
  !include "${__FILEDIR__}\installer\appearance.nsh"
!endif

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Downloading and verifying the media engine..."
  nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\runtime\install-mpv.ps1"'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "Could not download or verify the media engine. Check your internet connection and run the installer again." /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$INSTDIR\runtime\install-mpv.log"
  Delete "$INSTDIR\runtime\mpv.exe"
  Delete "$INSTDIR\runtime\d3dcompiler_43.dll"
!macroend
