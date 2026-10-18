; zipforge NSIS installer hooks.
; Registers Explorer context-menu entries for the current user (HKCU) and
; removes them again on uninstall. Menu labels follow the Windows UI language
; (Turkish on Turkish Windows, English elsewhere) and always point at the
; installed executable.

!include LogicLib.nsh

!macro zfVerb ROOT VERB LABEL ARGS
WriteRegStr HKCU "Software\Classes\${ROOT}\shell\${VERB}" "" "${LABEL}"
WriteRegStr HKCU "Software\Classes\${ROOT}\shell\${VERB}" "Icon" "$INSTDIR\zipforge.exe,0"
WriteRegStr HKCU "Software\Classes\${ROOT}\shell\${VERB}\command" "" '"$INSTDIR\zipforge.exe" ${ARGS}'
!macroend

!macro zfDel ROOT VERB
DeleteRegKey HKCU "Software\Classes\${ROOT}\shell\${VERB}"
!macroend

!macro zfArchive EXT
!insertmacro zfVerb "SystemFileAssociations\.${EXT}" "ZipforgeOpen" $R2 '"%1"'
!insertmacro zfVerb "SystemFileAssociations\.${EXT}" "ZipforgeExtractHere" $R3 '--extract "%1"'
!insertmacro zfVerb "SystemFileAssociations\.${EXT}" "ZipforgeExtractTo" $R4 '--extract-sub "%1"'
!macroend

!macro zfArchiveDel EXT
!insertmacro zfDel "SystemFileAssociations\.${EXT}" "ZipforgeOpen"
!insertmacro zfDel "SystemFileAssociations\.${EXT}" "ZipforgeExtractHere"
!insertmacro zfDel "SystemFileAssociations\.${EXT}" "ZipforgeExtractTo"
!macroend

!macro NSIS_HOOK_POSTINSTALL
; Menu labels: Turkish for Turkish Windows (1055), English otherwise.
System::Call 'kernel32::GetUserDefaultUILanguage() i .r0'
StrCpy $R1 "Add to zipforge archive"
StrCpy $R2 "Open with zipforge"
StrCpy $R3 "Extract here with zipforge"
StrCpy $R4 "Extract to folder with zipforge"
${If} $0 == 1055
StrCpy $R1 "zipforge ile arşivle"
StrCpy $R2 "zipforge ile aç"
StrCpy $R3 "zipforge ile buraya çıkart"
StrCpy $R4 "zipforge ile klasöre çıkart"
${EndIf}

; Folder / file / background "add to archive"
!insertmacro zfVerb "Directory" "ZipforgeAdd" $R1 '--add "%1"'
!insertmacro zfVerb "Directory\Background" "ZipforgeAdd" $R1 '--add "%V"'
!insertmacro zfVerb "*" "ZipforgeAdd" $R1 '--add "%1"'

; Archive file verbs
!insertmacro zfArchive "zip"
!insertmacro zfArchive "7z"
!insertmacro zfArchive "rar"
!insertmacro zfArchive "tar"
!insertmacro zfArchive "gz"
!insertmacro zfArchive "tgz"
!insertmacro zfArchive "bz2"
!insertmacro zfArchive "tbz2"
!insertmacro zfArchive "xz"
!insertmacro zfArchive "txz"
!insertmacro zfArchive "zst"
!insertmacro zfArchive "tzst"
!insertmacro zfArchive "iso"
!insertmacro zfArchive "cab"
!insertmacro zfArchive "jar"
!insertmacro zfArchive "001"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!insertmacro zfDel "Directory" "ZipforgeAdd"
!insertmacro zfDel "Directory\Background" "ZipforgeAdd"
!insertmacro zfDel "*" "ZipforgeAdd"

!insertmacro zfArchiveDel "zip"
!insertmacro zfArchiveDel "7z"
!insertmacro zfArchiveDel "rar"
!insertmacro zfArchiveDel "tar"
!insertmacro zfArchiveDel "gz"
!insertmacro zfArchiveDel "tgz"
!insertmacro zfArchiveDel "bz2"
!insertmacro zfArchiveDel "tbz2"
!insertmacro zfArchiveDel "xz"
!insertmacro zfArchiveDel "txz"
!insertmacro zfArchiveDel "zst"
!insertmacro zfArchiveDel "tzst"
!insertmacro zfArchiveDel "iso"
!insertmacro zfArchiveDel "cab"
!insertmacro zfArchiveDel "jar"
!insertmacro zfArchiveDel "001"
!macroend
