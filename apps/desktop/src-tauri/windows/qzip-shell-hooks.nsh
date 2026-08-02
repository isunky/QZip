!macro RegisterQZipArchive SLUG EXTENSION DISPLAY_NAME
  WriteRegStr HKCU "Software\Classes\QZip.Archive.${SLUG}" "" "${DISPLAY_NAME}"
  WriteRegStr HKCU "Software\Classes\QZip.Archive.${SLUG}\DefaultIcon" "" "$INSTDIR\file-icons\${SLUG}.ico"
  WriteRegStr HKCU "Software\Classes\QZip.Archive.${SLUG}\shell\open\command" "" '"$INSTDIR\qzip-desktop.exe" "%1"'
  WriteRegStr HKCU "Software\QZip\Capabilities\FileAssociations" "${EXTENSION}" "QZip.Archive.${SLUG}"
!macroend

!macro RemoveQZipArchive SLUG
  DeleteRegKey HKCU "Software\Classes\QZip.Archive.${SLUG}"
!macroend

!macro RegisterQZipLegacyArchive FILECLASS SLUG DISPLAY_NAME
  WriteRegStr HKCU "Software\Classes\${FILECLASS}" "" "${DISPLAY_NAME}"
  WriteRegStr HKCU "Software\Classes\${FILECLASS}\DefaultIcon" "" "$INSTDIR\file-icons\${SLUG}.ico"
  WriteRegStr HKCU "Software\Classes\${FILECLASS}\shell\open\command" "" '"$INSTDIR\qzip-desktop.exe" "%1"'
!macroend

!macro RefreshQZipAssociations
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Registering QZip file associations"
  ; Retain the previous generic ProgID so existing UserChoice values remain usable after upgrade.
  WriteRegStr HKCU "Software\Classes\QZip.Archive" "" "QZip Archive"
  WriteRegStr HKCU "Software\Classes\QZip.Archive\DefaultIcon" "" "$INSTDIR\file-icons\archive.ico"
  WriteRegStr HKCU "Software\Classes\QZip.Archive\shell\open\command" "" '"$INSTDIR\qzip-desktop.exe" "%1"'
  WriteRegStr HKCU "Software\QZip\Capabilities" "ApplicationName" "QZip"
  WriteRegStr HKCU "Software\QZip\Capabilities" "ApplicationDescription" "QZip archive manager"
  !insertmacro RegisterQZipArchive "7z" ".7z" "7-Zip Archive"
  !insertmacro RegisterQZipArchive "zip" ".zip" "ZIP Archive"
  !insertmacro RegisterQZipArchive "rar" ".rar" "RAR Archive"
  !insertmacro RegisterQZipArchive "tar" ".tar" "TAR Archive"
  !insertmacro RegisterQZipArchive "gz" ".gz" "GZip Archive"
  !insertmacro RegisterQZipArchive "tgz" ".tgz" "TAR.GZ Archive"
  !insertmacro RegisterQZipArchive "xz" ".xz" "XZ Archive"
  !insertmacro RegisterQZipArchive "txz" ".txz" "TAR.XZ Archive"
  !insertmacro RegisterQZipArchive "bz2" ".bz2" "BZip2 Archive"
  !insertmacro RegisterQZipArchive "iso" ".iso" "ISO Image"
  !insertmacro RegisterQZipArchive "cab" ".cab" "Windows Cabinet Archive"
  !insertmacro RegisterQZipArchive "wim" ".wim" "Windows Imaging Format"
  ; Older QZip builds used these display names as ProgIDs. Keep them functional
  ; so an existing Windows UserChoice immediately receives the per-format icon.
  !insertmacro RegisterQZipLegacyArchive "7-Zip Archive" "7z" "7-Zip Archive"
  !insertmacro RegisterQZipLegacyArchive "ZIP Archive" "zip" "ZIP Archive"
  !insertmacro RegisterQZipLegacyArchive "RAR Archive" "rar" "RAR Archive"
  !insertmacro RegisterQZipLegacyArchive "TAR Archive" "tar" "TAR Archive"
  !insertmacro RegisterQZipLegacyArchive "GZip Archive" "gz" "GZip Archive"
  !insertmacro RegisterQZipLegacyArchive "XZ Archive" "xz" "XZ Archive"
  !insertmacro RegisterQZipLegacyArchive "BZip2 Archive" "bz2" "BZip2 Archive"
  !insertmacro RegisterQZipLegacyArchive "ISO Image" "iso" "ISO Image"
  !insertmacro RegisterQZipLegacyArchive "Windows Cabinet Archive" "cab" "Windows Cabinet Archive"
  !insertmacro RegisterQZipLegacyArchive "Windows Imaging Format" "wim" "Windows Imaging Format"
  WriteRegStr HKCU "Software\RegisteredApplications" "QZip" "Software\QZip\Capabilities"
  !insertmacro RefreshQZipAssociations
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
  !insertmacro RemoveQZipArchive "7z"
  !insertmacro RemoveQZipArchive "zip"
  !insertmacro RemoveQZipArchive "rar"
  !insertmacro RemoveQZipArchive "tar"
  !insertmacro RemoveQZipArchive "gz"
  !insertmacro RemoveQZipArchive "tgz"
  !insertmacro RemoveQZipArchive "xz"
  !insertmacro RemoveQZipArchive "txz"
  !insertmacro RemoveQZipArchive "bz2"
  !insertmacro RemoveQZipArchive "iso"
  !insertmacro RemoveQZipArchive "cab"
  !insertmacro RemoveQZipArchive "wim"
  DeleteRegKey HKCU "Software\Classes\7-Zip Archive"
  DeleteRegKey HKCU "Software\Classes\ZIP Archive"
  DeleteRegKey HKCU "Software\Classes\RAR Archive"
  DeleteRegKey HKCU "Software\Classes\TAR Archive"
  DeleteRegKey HKCU "Software\Classes\GZip Archive"
  DeleteRegKey HKCU "Software\Classes\XZ Archive"
  DeleteRegKey HKCU "Software\Classes\BZip2 Archive"
  DeleteRegKey HKCU "Software\Classes\ISO Image"
  DeleteRegKey HKCU "Software\Classes\Windows Cabinet Archive"
  DeleteRegKey HKCU "Software\Classes\Windows Imaging Format"
  DeleteRegKey HKCU "Software\Classes\QZip.Archive"
  !insertmacro RefreshQZipAssociations
!macroend
