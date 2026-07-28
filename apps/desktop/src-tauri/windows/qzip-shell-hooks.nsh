!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Registering QZip Explorer commands"
  nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\qzip-shell\Register-QZipShell.ps1" -InstallPath "$INSTDIR" -PackagePath "$INSTDIR\qzip-shell\QZip.Shell.msix"'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "QZip Explorer menu registration failed. Installation was cancelled."
    Abort
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Removing QZip Explorer commands"
  nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\qzip-shell\Unregister-QZipShell.ps1"'
  Pop $0
!macroend
