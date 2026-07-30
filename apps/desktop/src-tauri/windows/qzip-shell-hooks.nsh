!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Registering QZip file associations"
  WriteRegStr HKCU "Software\Classes\QZip.Archive" "" "QZip Archive"
  WriteRegStr HKCU "Software\Classes\QZip.Archive\DefaultIcon" "" "$INSTDIR\qzip-desktop.exe,0"
  WriteRegStr HKCU "Software\Classes\QZip.Archive\shell\open\command" "" '"$INSTDIR\qzip-desktop.exe" "%1"'
  WriteRegStr HKCU "Software\QZip\Capabilities" "ApplicationName" "QZip"
  WriteRegStr HKCU "Software\QZip\Capabilities" "ApplicationDescription" "QZip archive manager"
  WriteRegStr HKCU "Software\QZip\Capabilities\FileAssociations" ".7z" "QZip.Archive"
  WriteRegStr HKCU "Software\QZip\Capabilities\FileAssociations" ".zip" "QZip.Archive"
  WriteRegStr HKCU "Software\QZip\Capabilities\FileAssociations" ".rar" "QZip.Archive"
  WriteRegStr HKCU "Software\QZip\Capabilities\FileAssociations" ".tar" "QZip.Archive"
  WriteRegStr HKCU "Software\QZip\Capabilities\FileAssociations" ".gz" "QZip.Archive"
  WriteRegStr HKCU "Software\QZip\Capabilities\FileAssociations" ".xz" "QZip.Archive"
  WriteRegStr HKCU "Software\QZip\Capabilities\FileAssociations" ".bz2" "QZip.Archive"
  WriteRegStr HKCU "Software\RegisteredApplications" "QZip" "Software\QZip\Capabilities"
  DetailPrint "QZip Explorer commands will register when QZip first starts"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Removing QZip Explorer commands"
  nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\qzip-shell\Unregister-QZipShell.ps1"'
  Pop $0
  DetailPrint "Removing QZip file associations"
  DeleteRegValue HKCU "Software\RegisteredApplications" "QZip"
  DeleteRegKey HKCU "Software\QZip\Capabilities"
  DeleteRegKey /ifempty HKCU "Software\QZip"
  DeleteRegKey HKCU "Software\Classes\QZip.Archive"
!macroend
