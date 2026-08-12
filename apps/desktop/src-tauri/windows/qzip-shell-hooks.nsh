!include LogicLib.nsh
!include nsDialogs.nsh

!define QZIP_ASSOC_KEY "Software\QZip\Installer\FileAssociations"
!define QZIP_BACKUP_KEY "Software\QZip\Installer\AssociationBackups"
!define QZIP_CAPABILITIES_KEY "Software\QZip\Capabilities"

; Declare custom strings only after MUI has loaded every installer language.
; Declaring them from this hook immediately would place them before
; MUI_LANGUAGE and produces empty strings in the rendered custom page.
!macro QZIP_LANGUAGE_STRINGS
  LangString QZipAssocTitle ${LANG_ENGLISH} "File associations"
  LangString QZipAssocTitle ${LANG_SIMPCHINESE} "文件关联"
  LangString QZipAssocSubtitle ${LANG_ENGLISH} "Choose the archive types QZip should open. Windows may ask you to confirm default apps."
  LangString QZipAssocSubtitle ${LANG_SIMPCHINESE} "选择要由 QZip 打开的压缩文件类型。Windows 可能仍会要求确认默认应用。"
  LangString QZipAssocSelectAll ${LANG_ENGLISH} "Select all"
  LangString QZipAssocSelectAll ${LANG_SIMPCHINESE} "全选"
  LangString QZipAssocOpenDefaults ${LANG_ENGLISH} "Open Windows Default Apps settings after installation"
  LangString QZipAssocOpenDefaults ${LANG_SIMPCHINESE} "安装完成后打开 Windows 默认应用设置"
  LangString QZipAssoc7z ${LANG_ENGLISH} "7Z archive (.7z)"
  LangString QZipAssoc7z ${LANG_SIMPCHINESE} "7Z 压缩包 (.7z)"
  LangString QZipAssocZip ${LANG_ENGLISH} "ZIP archive (.zip)"
  LangString QZipAssocZip ${LANG_SIMPCHINESE} "ZIP 压缩包 (.zip)"
  LangString QZipAssocRar ${LANG_ENGLISH} "RAR archive (.rar)"
  LangString QZipAssocRar ${LANG_SIMPCHINESE} "RAR 压缩包 (.rar)"
  LangString QZipAssocTar ${LANG_ENGLISH} "TAR archive (.tar)"
  LangString QZipAssocTar ${LANG_SIMPCHINESE} "TAR 归档 (.tar)"
  LangString QZipAssocGz ${LANG_ENGLISH} "GZip archive (.gz)"
  LangString QZipAssocGz ${LANG_SIMPCHINESE} "GZip 压缩包 (.gz)"
  LangString QZipAssocTgz ${LANG_ENGLISH} "TAR.GZ archive (.tgz)"
  LangString QZipAssocTgz ${LANG_SIMPCHINESE} "TAR.GZ 压缩包 (.tgz)"
  LangString QZipAssocXz ${LANG_ENGLISH} "XZ archive (.xz)"
  LangString QZipAssocXz ${LANG_SIMPCHINESE} "XZ 压缩包 (.xz)"
  LangString QZipAssocTxz ${LANG_ENGLISH} "TAR.XZ archive (.txz)"
  LangString QZipAssocTxz ${LANG_SIMPCHINESE} "TAR.XZ 压缩包 (.txz)"
  LangString QZipAssocBz2 ${LANG_ENGLISH} "BZip2 archive (.bz2)"
  LangString QZipAssocBz2 ${LANG_SIMPCHINESE} "BZip2 压缩包 (.bz2)"
  LangString QZipAssocIso ${LANG_ENGLISH} "ISO image (.iso)"
  LangString QZipAssocIso ${LANG_SIMPCHINESE} "ISO 镜像 (.iso)"
  LangString QZipAssocCab ${LANG_ENGLISH} "Windows Cabinet (.cab)"
  LangString QZipAssocCab ${LANG_SIMPCHINESE} "Windows Cabinet (.cab)"
  LangString QZipAssocWim ${LANG_ENGLISH} "Windows image (.wim)"
  LangString QZipAssocWim ${LANG_SIMPCHINESE} "Windows 映像 (.wim)"
!macroend

Var QZipAssocAll
Var QZipAssoc7z
Var QZipAssocZip
Var QZipAssocRar
Var QZipAssocTar
Var QZipAssocGz
Var QZipAssocTgz
Var QZipAssocXz
Var QZipAssocTxz
Var QZipAssocBz2
Var QZipAssocIso
Var QZipAssocCab
Var QZipAssocWim
Var QZipOpenDefaultsCheckbox
Var QZipSelectionLoaded
Var QZipOpenDefaultsState
Var QZipState7z
Var QZipStateZip
Var QZipStateRar
Var QZipStateTar
Var QZipStateGz
Var QZipStateTgz
Var QZipStateXz
Var QZipStateTxz
Var QZipStateBz2
Var QZipStateIso
Var QZipStateCab
Var QZipStateWim

!macro QZipDetectLegacy EXTENSION PROGID LEGACY DEFAULT_STATE STATE
  StrCpy ${STATE} ${DEFAULT_STATE}
  ReadRegStr $0 HKCU "${QZIP_CAPABILITIES_KEY}\FileAssociations" "${EXTENSION}"
  ${If} $0 == "${PROGID}"
  ${OrIf} $0 == "QZip.Archive"
    StrCpy ${STATE} ${BST_CHECKED}
  ${Else}
    ReadRegStr $0 HKCU "Software\Classes\${EXTENSION}" ""
    ${If} $0 == "${PROGID}"
    ${OrIf} $0 == "QZip.Archive"
    ${OrIf} $0 == "${LEGACY}"
      StrCpy ${STATE} ${BST_CHECKED}
    ${EndIf}
  ${EndIf}
!macroend

!macro QZipReadSavedState SLUG DEFAULT_STATE STATE
  ClearErrors
  ReadRegDWORD ${STATE} HKCU "${QZIP_ASSOC_KEY}" "${SLUG}"
  ${If} ${Errors}
    StrCpy ${STATE} ${DEFAULT_STATE}
  ${EndIf}
!macroend

!macro QZipWriteSavedState SLUG STATE
  WriteRegDWORD HKCU "${QZIP_ASSOC_KEY}" "${SLUG}" ${STATE}
!macroend

!macro QZipSetControlState CONTROL STATE
  ${NSD_SetState} ${CONTROL} ${STATE}
!macroend

!macro QZipReadControlState CONTROL STATE
  ${NSD_GetState} ${CONTROL} ${STATE}
!macroend

!macro QZipCreateAssociationCheckbox CONTROL STATE X Y TEXT
  ${NSD_CreateCheckbox} ${X} ${Y} 31% 12u "${TEXT}"
  Pop ${CONTROL}
  ${NSD_SetState} ${CONTROL} ${STATE}
  ${NSD_OnClick} ${CONTROL} QZipAssociationItemChanged
!macroend

!macro QZipRegisterClass SLUG DISPLAY_NAME
  WriteRegStr HKCU "Software\Classes\QZip.Archive.${SLUG}" "" "${DISPLAY_NAME}"
  WriteRegStr HKCU "Software\Classes\QZip.Archive.${SLUG}\DefaultIcon" "" "$INSTDIR\file-icons\${SLUG}.ico"
  WriteRegStr HKCU "Software\Classes\QZip.Archive.${SLUG}\shell" "" "open"
  WriteRegStr HKCU "Software\Classes\QZip.Archive.${SLUG}\shell\open\command" "" '$"$INSTDIR\qzip-desktop.exe$" $"%1$"'
!macroend

!macro QZipRegisterLegacyClass FILECLASS SLUG DISPLAY_NAME
  WriteRegStr HKCU "Software\Classes\${FILECLASS}" "" "${DISPLAY_NAME}"
  WriteRegStr HKCU "Software\Classes\${FILECLASS}\DefaultIcon" "" "$INSTDIR\file-icons\${SLUG}.ico"
  WriteRegStr HKCU "Software\Classes\${FILECLASS}\shell" "" "open"
  WriteRegStr HKCU "Software\Classes\${FILECLASS}\shell\open\command" "" '$"$INSTDIR\qzip-desktop.exe$" $"%1$"'
!macroend

!macro QZipBackupCurrentAssociation SLUG EXTENSION PROGID
  ClearErrors
  ReadRegDWORD $1 HKCU "${QZIP_BACKUP_KEY}" "${SLUG}.Saved"
  ${If} ${Errors}
    ReadRegStr $0 HKCU "Software\Classes\${EXTENSION}" ""
    ${If} $0 == "${PROGID}"
      ClearErrors
      ReadRegStr $2 HKCU "Software\Classes\${EXTENSION}" "${PROGID}_backup"
      ${IfNot} ${Errors}
        StrCpy $0 $2
      ${EndIf}
    ${EndIf}
    WriteRegStr HKCU "${QZIP_BACKUP_KEY}" "${SLUG}" "$0"
    WriteRegDWORD HKCU "${QZIP_BACKUP_KEY}" "${SLUG}.Saved" 1
  ${EndIf}
!macroend

!macro QZipRestoreCurrentAssociation SLUG EXTENSION PROGID LEGACY
  ReadRegStr $0 HKCU "Software\Classes\${EXTENSION}" ""
  ${If} $0 == "${PROGID}"
  ${OrIf} $0 == "QZip.Archive"
  ${OrIf} $0 == "${LEGACY}"
    ClearErrors
    ReadRegDWORD $1 HKCU "${QZIP_BACKUP_KEY}" "${SLUG}.Saved"
    ${IfNot} ${Errors}
      ReadRegStr $2 HKCU "${QZIP_BACKUP_KEY}" "${SLUG}"
      ${If} $2 == ""
        DeleteRegValue HKCU "Software\Classes\${EXTENSION}" ""
      ${Else}
        WriteRegStr HKCU "Software\Classes\${EXTENSION}" "" "$2"
      ${EndIf}
    ${EndIf}
  ${EndIf}
  DeleteRegValue HKCU "${QZIP_BACKUP_KEY}" "${SLUG}"
  DeleteRegValue HKCU "${QZIP_BACKUP_KEY}" "${SLUG}.Saved"
!macroend

!macro QZipRemoveClassUnlessUserChoice EXTENSION PROGID
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\${EXTENSION}\UserChoice" "ProgId"
  ${If} $0 != "${PROGID}"
    DeleteRegKey HKCU "Software\Classes\${PROGID}"
  ${EndIf}
!macroend

!macro QZipApplyAssociation SLUG EXTENSION PROGID LEGACY DISPLAY_NAME STATE
  ${If} ${STATE} == ${BST_CHECKED}
    !insertmacro QZipBackupCurrentAssociation "${SLUG}" "${EXTENSION}" "${PROGID}"
    WriteRegStr HKCU "Software\Classes\${EXTENSION}" "" "${PROGID}"
    !insertmacro QZipRegisterClass "${SLUG}" "${DISPLAY_NAME}"
    !insertmacro QZipRegisterLegacyClass "${LEGACY}" "${SLUG}" "${DISPLAY_NAME}"
    WriteRegStr HKCU "${QZIP_CAPABILITIES_KEY}\FileAssociations" "${EXTENSION}" "${PROGID}"
  ${Else}
    DeleteRegValue HKCU "${QZIP_CAPABILITIES_KEY}\FileAssociations" "${EXTENSION}"
    !insertmacro QZipRestoreCurrentAssociation "${SLUG}" "${EXTENSION}" "${PROGID}" "${LEGACY}"
    !insertmacro QZipRemoveClassUnlessUserChoice "${EXTENSION}" "${PROGID}"
    !insertmacro QZipRemoveClassUnlessUserChoice "${EXTENSION}" "${LEGACY}"
  ${EndIf}
!macroend

!macro QZipUninstallAssociation SLUG EXTENSION PROGID LEGACY
  !insertmacro QZipRestoreCurrentAssociation "${SLUG}" "${EXTENSION}" "${PROGID}" "${LEGACY}"
  DeleteRegKey HKCU "Software\Classes\${PROGID}"
  DeleteRegKey HKCU "Software\Classes\${LEGACY}"
!macroend

!macro QZIP_ASSOCIATION_PAGE
  Page custom QZipAssociationPageCreate QZipAssociationPageLeave
!macroend

!macro QZIP_CAPTURE_UPGRADE_SELECTION
  Call QZipLoadAssociationSelection
!macroend

!macro QZIP_RESTORE_UPGRADE_SELECTION
  Call QZipPersistAssociationSelection
!macroend

!macro QZIP_FINISH_PAGE_OPTIONS
  !define MUI_PAGE_CUSTOMFUNCTION_SHOW QZipFinishPageShow
  !define MUI_PAGE_CUSTOMFUNCTION_LEAVE QZipFinishPageLeave
!macroend

Function QZipLoadAssociationSelection
  ${If} $QZipSelectionLoaded == 1
    Return
  ${EndIf}
  StrCpy $QZipSelectionLoaded 1
  StrCpy $QZipOpenDefaultsState ${BST_CHECKED}
  ClearErrors
  ReadRegDWORD $0 HKCU "${QZIP_ASSOC_KEY}" "Initialized"
  ${IfNot} ${Errors}
    !insertmacro QZipReadSavedState "7z" ${BST_CHECKED} $QZipState7z
    !insertmacro QZipReadSavedState "zip" ${BST_CHECKED} $QZipStateZip
    !insertmacro QZipReadSavedState "rar" ${BST_CHECKED} $QZipStateRar
    !insertmacro QZipReadSavedState "tar" ${BST_UNCHECKED} $QZipStateTar
    !insertmacro QZipReadSavedState "gz" ${BST_UNCHECKED} $QZipStateGz
    !insertmacro QZipReadSavedState "tgz" ${BST_UNCHECKED} $QZipStateTgz
    !insertmacro QZipReadSavedState "xz" ${BST_UNCHECKED} $QZipStateXz
    !insertmacro QZipReadSavedState "txz" ${BST_UNCHECKED} $QZipStateTxz
    !insertmacro QZipReadSavedState "bz2" ${BST_UNCHECKED} $QZipStateBz2
    !insertmacro QZipReadSavedState "iso" ${BST_UNCHECKED} $QZipStateIso
    !insertmacro QZipReadSavedState "cab" ${BST_UNCHECKED} $QZipStateCab
    !insertmacro QZipReadSavedState "wim" ${BST_UNCHECKED} $QZipStateWim
    Return
  ${EndIf}
  !insertmacro QZipDetectLegacy ".7z" "QZip.Archive.7z" "7-Zip Archive" ${BST_CHECKED} $QZipState7z
  !insertmacro QZipDetectLegacy ".zip" "QZip.Archive.zip" "ZIP Archive" ${BST_CHECKED} $QZipStateZip
  !insertmacro QZipDetectLegacy ".rar" "QZip.Archive.rar" "RAR Archive" ${BST_CHECKED} $QZipStateRar
  !insertmacro QZipDetectLegacy ".tar" "QZip.Archive.tar" "TAR Archive" ${BST_UNCHECKED} $QZipStateTar
  !insertmacro QZipDetectLegacy ".gz" "QZip.Archive.gz" "GZip Archive" ${BST_UNCHECKED} $QZipStateGz
  !insertmacro QZipDetectLegacy ".tgz" "QZip.Archive.tgz" "TAR.GZ Archive" ${BST_UNCHECKED} $QZipStateTgz
  !insertmacro QZipDetectLegacy ".xz" "QZip.Archive.xz" "XZ Archive" ${BST_UNCHECKED} $QZipStateXz
  !insertmacro QZipDetectLegacy ".txz" "QZip.Archive.txz" "TAR.XZ Archive" ${BST_UNCHECKED} $QZipStateTxz
  !insertmacro QZipDetectLegacy ".bz2" "QZip.Archive.bz2" "BZip2 Archive" ${BST_UNCHECKED} $QZipStateBz2
  !insertmacro QZipDetectLegacy ".iso" "QZip.Archive.iso" "ISO Image" ${BST_UNCHECKED} $QZipStateIso
  !insertmacro QZipDetectLegacy ".cab" "QZip.Archive.cab" "Windows Cabinet Archive" ${BST_UNCHECKED} $QZipStateCab
  !insertmacro QZipDetectLegacy ".wim" "QZip.Archive.wim" "Windows Imaging Format" ${BST_UNCHECKED} $QZipStateWim
FunctionEnd

Function QZipPersistAssociationSelection
  WriteRegDWORD HKCU "${QZIP_ASSOC_KEY}" "Initialized" 1
  !insertmacro QZipWriteSavedState "7z" $QZipState7z
  !insertmacro QZipWriteSavedState "zip" $QZipStateZip
  !insertmacro QZipWriteSavedState "rar" $QZipStateRar
  !insertmacro QZipWriteSavedState "tar" $QZipStateTar
  !insertmacro QZipWriteSavedState "gz" $QZipStateGz
  !insertmacro QZipWriteSavedState "tgz" $QZipStateTgz
  !insertmacro QZipWriteSavedState "xz" $QZipStateXz
  !insertmacro QZipWriteSavedState "txz" $QZipStateTxz
  !insertmacro QZipWriteSavedState "bz2" $QZipStateBz2
  !insertmacro QZipWriteSavedState "iso" $QZipStateIso
  !insertmacro QZipWriteSavedState "cab" $QZipStateCab
  !insertmacro QZipWriteSavedState "wim" $QZipStateWim
FunctionEnd

!macro QZipAccumulateControlState CONTROL
  ${NSD_GetState} ${CONTROL} $2
  ${If} $2 == ${BST_CHECKED}
    IntOp $0 $0 + 1
  ${Else}
    IntOp $1 $1 + 1
  ${EndIf}
!macroend

Function QZipRefreshSelectAll
  StrCpy $0 0
  StrCpy $1 0
  !insertmacro QZipAccumulateControlState $QZipAssoc7z
  !insertmacro QZipAccumulateControlState $QZipAssocZip
  !insertmacro QZipAccumulateControlState $QZipAssocRar
  !insertmacro QZipAccumulateControlState $QZipAssocTar
  !insertmacro QZipAccumulateControlState $QZipAssocGz
  !insertmacro QZipAccumulateControlState $QZipAssocTgz
  !insertmacro QZipAccumulateControlState $QZipAssocXz
  !insertmacro QZipAccumulateControlState $QZipAssocTxz
  !insertmacro QZipAccumulateControlState $QZipAssocBz2
  !insertmacro QZipAccumulateControlState $QZipAssocIso
  !insertmacro QZipAccumulateControlState $QZipAssocCab
  !insertmacro QZipAccumulateControlState $QZipAssocWim
  ${If} $0 == 12
    ${NSD_SetState} $QZipAssocAll ${BST_CHECKED}
  ${ElseIf} $1 == 12
    ${NSD_SetState} $QZipAssocAll ${BST_UNCHECKED}
  ${Else}
    ${NSD_SetState} $QZipAssocAll ${BST_INDETERMINATE}
  ${EndIf}
FunctionEnd

Function QZipAssociationItemChanged
  Call QZipRefreshSelectAll
FunctionEnd

Function QZipAssociationToggleAll
  ${NSD_GetState} $QZipAssocAll $0
  ${If} $0 == ${BST_CHECKED}
    StrCpy $0 ${BST_CHECKED}
  ${Else}
    StrCpy $0 ${BST_UNCHECKED}
  ${EndIf}
  !insertmacro QZipSetControlState $QZipAssoc7z $0
  !insertmacro QZipSetControlState $QZipAssocZip $0
  !insertmacro QZipSetControlState $QZipAssocRar $0
  !insertmacro QZipSetControlState $QZipAssocTar $0
  !insertmacro QZipSetControlState $QZipAssocGz $0
  !insertmacro QZipSetControlState $QZipAssocTgz $0
  !insertmacro QZipSetControlState $QZipAssocXz $0
  !insertmacro QZipSetControlState $QZipAssocTxz $0
  !insertmacro QZipSetControlState $QZipAssocBz2 $0
  !insertmacro QZipSetControlState $QZipAssocIso $0
  !insertmacro QZipSetControlState $QZipAssocCab $0
  !insertmacro QZipSetControlState $QZipAssocWim $0
FunctionEnd

Function QZipAssociationPageCreate
  ${If} $PassiveMode == 1
    Abort
  ${EndIf}
  IfSilent 0 +2
    Abort
  Call QZipLoadAssociationSelection
  !insertmacro MUI_HEADER_TEXT "$(QZipAssocTitle)" "$(QZipAssocSubtitle)"
  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}
  ${NSD_CreateCheckbox} 0 0 100% 12u "$(QZipAssocSelectAll)"
  Pop $QZipAssocAll
  ${NSD_OnClick} $QZipAssocAll QZipAssociationToggleAll
  !insertmacro QZipCreateAssociationCheckbox $QZipAssoc7z $QZipState7z 0 24u "$(QZipAssoc7z)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocZip $QZipStateZip 34% 24u "$(QZipAssocZip)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocRar $QZipStateRar 68% 24u "$(QZipAssocRar)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocTar $QZipStateTar 0 48u "$(QZipAssocTar)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocGz $QZipStateGz 34% 48u "$(QZipAssocGz)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocTgz $QZipStateTgz 68% 48u "$(QZipAssocTgz)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocXz $QZipStateXz 0 72u "$(QZipAssocXz)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocTxz $QZipStateTxz 34% 72u "$(QZipAssocTxz)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocBz2 $QZipStateBz2 68% 72u "$(QZipAssocBz2)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocIso $QZipStateIso 0 96u "$(QZipAssocIso)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocCab $QZipStateCab 34% 96u "$(QZipAssocCab)"
  !insertmacro QZipCreateAssociationCheckbox $QZipAssocWim $QZipStateWim 68% 96u "$(QZipAssocWim)"
  Call QZipRefreshSelectAll
  nsDialogs::Show
FunctionEnd

Function QZipAssociationPageLeave
  !insertmacro QZipReadControlState $QZipAssoc7z $QZipState7z
  !insertmacro QZipReadControlState $QZipAssocZip $QZipStateZip
  !insertmacro QZipReadControlState $QZipAssocRar $QZipStateRar
  !insertmacro QZipReadControlState $QZipAssocTar $QZipStateTar
  !insertmacro QZipReadControlState $QZipAssocGz $QZipStateGz
  !insertmacro QZipReadControlState $QZipAssocTgz $QZipStateTgz
  !insertmacro QZipReadControlState $QZipAssocXz $QZipStateXz
  !insertmacro QZipReadControlState $QZipAssocTxz $QZipStateTxz
  !insertmacro QZipReadControlState $QZipAssocBz2 $QZipStateBz2
  !insertmacro QZipReadControlState $QZipAssocIso $QZipStateIso
  !insertmacro QZipReadControlState $QZipAssocCab $QZipStateCab
  !insertmacro QZipReadControlState $QZipAssocWim $QZipStateWim
  Call QZipPersistAssociationSelection
FunctionEnd

Function QZipAnyAssociationSelected
  StrCpy $0 0
  ${If} $QZipState7z == ${BST_CHECKED}
  ${OrIf} $QZipStateZip == ${BST_CHECKED}
  ${OrIf} $QZipStateRar == ${BST_CHECKED}
  ${OrIf} $QZipStateTar == ${BST_CHECKED}
  ${OrIf} $QZipStateGz == ${BST_CHECKED}
  ${OrIf} $QZipStateTgz == ${BST_CHECKED}
  ${OrIf} $QZipStateXz == ${BST_CHECKED}
  ${OrIf} $QZipStateTxz == ${BST_CHECKED}
  ${OrIf} $QZipStateBz2 == ${BST_CHECKED}
  ${OrIf} $QZipStateIso == ${BST_CHECKED}
  ${OrIf} $QZipStateCab == ${BST_CHECKED}
  ${OrIf} $QZipStateWim == ${BST_CHECKED}
    StrCpy $0 1
  ${EndIf}
FunctionEnd

Function QZipFinishPageShow
  Call QZipAnyAssociationSelected
  ${If} $0 != 1
    Return
  ${EndIf}
  ; The MUI finish page is 193 dialog units high. Place this below the built-in
  ; Run QZip and desktop-shortcut choices, while keeping it inside the page.
  ${NSD_CreateCheckbox} 120u 132u 195u 20u "$(QZipAssocOpenDefaults)"
  Pop $QZipOpenDefaultsCheckbox
  ${NSD_SetState} $QZipOpenDefaultsCheckbox $QZipOpenDefaultsState
FunctionEnd

Function QZipFinishPageLeave
  ${If} $QZipOpenDefaultsCheckbox == ""
    Return
  ${EndIf}
  ${NSD_GetState} $QZipOpenDefaultsCheckbox $QZipOpenDefaultsState
  ${If} $QZipOpenDefaultsState == ${BST_CHECKED}
    ExecShell "open" "ms-settings:defaultapps?registeredAppUser=QZip"
  ${EndIf}
FunctionEnd

!macro NSIS_HOOK_POSTINSTALL
  Call QZipLoadAssociationSelection
  Call QZipPersistAssociationSelection
  DetailPrint "Applying QZip file association choices"
  WriteRegStr HKCU "${QZIP_CAPABILITIES_KEY}" "ApplicationName" "QZip"
  WriteRegStr HKCU "${QZIP_CAPABILITIES_KEY}" "ApplicationDescription" "QZip archive manager"
  !insertmacro QZipApplyAssociation "7z" ".7z" "QZip.Archive.7z" "7-Zip Archive" "7-Zip Archive" $QZipState7z
  !insertmacro QZipApplyAssociation "zip" ".zip" "QZip.Archive.zip" "ZIP Archive" "ZIP Archive" $QZipStateZip
  !insertmacro QZipApplyAssociation "rar" ".rar" "QZip.Archive.rar" "RAR Archive" "RAR Archive" $QZipStateRar
  !insertmacro QZipApplyAssociation "tar" ".tar" "QZip.Archive.tar" "TAR Archive" "TAR Archive" $QZipStateTar
  !insertmacro QZipApplyAssociation "gz" ".gz" "QZip.Archive.gz" "GZip Archive" "GZip Archive" $QZipStateGz
  !insertmacro QZipApplyAssociation "tgz" ".tgz" "QZip.Archive.tgz" "TAR.GZ Archive" "TAR.GZ Archive" $QZipStateTgz
  !insertmacro QZipApplyAssociation "xz" ".xz" "QZip.Archive.xz" "XZ Archive" "XZ Archive" $QZipStateXz
  !insertmacro QZipApplyAssociation "txz" ".txz" "QZip.Archive.txz" "TAR.XZ Archive" "TAR.XZ Archive" $QZipStateTxz
  !insertmacro QZipApplyAssociation "bz2" ".bz2" "QZip.Archive.bz2" "BZip2 Archive" "BZip2 Archive" $QZipStateBz2
  !insertmacro QZipApplyAssociation "iso" ".iso" "QZip.Archive.iso" "ISO Image" "ISO Image" $QZipStateIso
  !insertmacro QZipApplyAssociation "cab" ".cab" "QZip.Archive.cab" "Windows Cabinet Archive" "Windows Cabinet Archive" $QZipStateCab
  !insertmacro QZipApplyAssociation "wim" ".wim" "QZip.Archive.wim" "Windows Imaging Format" "Windows Imaging Format" $QZipStateWim
  Call QZipAnyAssociationSelected
  ${If} $0 == 1
    WriteRegStr HKCU "Software\RegisteredApplications" "QZip" "${QZIP_CAPABILITIES_KEY}"
  ${Else}
    DeleteRegValue HKCU "Software\RegisteredApplications" "QZip"
    DeleteRegKey /ifempty HKCU "${QZIP_CAPABILITIES_KEY}\FileAssociations"
    DeleteRegKey /ifempty HKCU "${QZIP_CAPABILITIES_KEY}"
  ${EndIf}
  WriteRegStr HKCU "Software\Classes\QZip.Archive" "" "QZip Archive"
  WriteRegStr HKCU "Software\Classes\QZip.Archive\DefaultIcon" "" "$INSTDIR\file-icons\archive.ico"
  WriteRegStr HKCU "Software\Classes\QZip.Archive\shell\open\command" "" '$"$INSTDIR\qzip-desktop.exe$" $"%1$"'
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  DetailPrint "QZip Explorer commands will register when QZip first starts"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Removing QZip Explorer commands"
  nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\qzip-shell\Unregister-QZipShell.ps1"'
  Pop $0
  DetailPrint "Restoring file associations previously owned by QZip"
  !insertmacro QZipUninstallAssociation "7z" ".7z" "QZip.Archive.7z" "7-Zip Archive"
  !insertmacro QZipUninstallAssociation "zip" ".zip" "QZip.Archive.zip" "ZIP Archive"
  !insertmacro QZipUninstallAssociation "rar" ".rar" "QZip.Archive.rar" "RAR Archive"
  !insertmacro QZipUninstallAssociation "tar" ".tar" "QZip.Archive.tar" "TAR Archive"
  !insertmacro QZipUninstallAssociation "gz" ".gz" "QZip.Archive.gz" "GZip Archive"
  !insertmacro QZipUninstallAssociation "tgz" ".tgz" "QZip.Archive.tgz" "TAR.GZ Archive"
  !insertmacro QZipUninstallAssociation "xz" ".xz" "QZip.Archive.xz" "XZ Archive"
  !insertmacro QZipUninstallAssociation "txz" ".txz" "QZip.Archive.txz" "TAR.XZ Archive"
  !insertmacro QZipUninstallAssociation "bz2" ".bz2" "QZip.Archive.bz2" "BZip2 Archive"
  !insertmacro QZipUninstallAssociation "iso" ".iso" "QZip.Archive.iso" "ISO Image"
  !insertmacro QZipUninstallAssociation "cab" ".cab" "QZip.Archive.cab" "Windows Cabinet Archive"
  !insertmacro QZipUninstallAssociation "wim" ".wim" "QZip.Archive.wim" "Windows Imaging Format"
  DeleteRegValue HKCU "Software\RegisteredApplications" "QZip"
  DeleteRegKey HKCU "${QZIP_CAPABILITIES_KEY}"
  DeleteRegKey HKCU "${QZIP_ASSOC_KEY}"
  DeleteRegKey HKCU "${QZIP_BACKUP_KEY}"
  DeleteRegKey /ifempty HKCU "Software\QZip\Installer"
  DeleteRegKey /ifempty HKCU "Software\QZip"
  DeleteRegKey HKCU "Software\Classes\QZip.Archive"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
