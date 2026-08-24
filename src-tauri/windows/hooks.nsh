; Hallmark · component: nsis-installer · genre: playful-técnico · theme: press-shop banana
; 64-bit: SetWindowLongPtr (SetWindowLong truncates and clips the bitmap to "NE"/"BAC").

!define MUI_BGCOLOR FAF8F2
!define MUI_TEXTCOLOR 111A14
!define MUI_INSTFILESPAGE_COLORS "111A14 FAF8F2"
!define MUI_ABORTWARNING

!ifndef BM_SETIMAGE
  !define BM_SETIMAGE 0x00F7
!endif
!ifndef IMAGE_BITMAP
  !define IMAGE_BITMAP 0
!endif

!macro MP_UNPACK_ART
  InitPluginsDir
  File "/oname=$PLUGINSDIR\mp-next-es.bmp" "${__FILEDIR__}\btn-next-es.bmp"
  File "/oname=$PLUGINSDIR\mp-next-en.bmp" "${__FILEDIR__}\btn-next-en.bmp"
  File "/oname=$PLUGINSDIR\mp-install-es.bmp" "${__FILEDIR__}\btn-install-es.bmp"
  File "/oname=$PLUGINSDIR\mp-install-en.bmp" "${__FILEDIR__}\btn-install-en.bmp"
  File "/oname=$PLUGINSDIR\mp-finish-es.bmp" "${__FILEDIR__}\btn-finish-es.bmp"
  File "/oname=$PLUGINSDIR\mp-finish-en.bmp" "${__FILEDIR__}\btn-finish-en.bmp"
  File "/oname=$PLUGINSDIR\mp-back-es.bmp" "${__FILEDIR__}\btn-back-es.bmp"
  File "/oname=$PLUGINSDIR\mp-back-en.bmp" "${__FILEDIR__}\btn-back-en.bmp"
  File "/oname=$PLUGINSDIR\mp-cancel-es.bmp" "${__FILEDIR__}\btn-cancel-es.bmp"
  File "/oname=$PLUGINSDIR\mp-cancel-en.bmp" "${__FILEDIR__}\btn-cancel-en.bmp"
  File "/oname=$PLUGINSDIR\mp-uninstall-es.bmp" "${__FILEDIR__}\btn-uninstall-es.bmp"
  File "/oname=$PLUGINSDIR\mp-uninstall-en.bmp" "${__FILEDIR__}\btn-uninstall-en.bmp"
!macroend

; $0 hwnd, $R8 path, $R6 width px, $R7 height px
!macro MP_STAMP_ONE UN
Function ${UN}MpStampOne
  Push $3
  Push $4
  ${If} $0 != 0
    System::Call 'uxtheme::SetWindowTheme(p r0, w " ", w " ")'
    System::Call 'user32::GetWindowLongPtrW(p r0, i -16) p .r3'
    System::Int64Op $3 | 32896
    Pop $3
    System::Int64Op $3 & 0xFFFFFFFFFFFFFFFE
    Pop $3
    System::Call 'user32::SetWindowLongPtrW(p r0, i -16, p r3)'
    System::Call 'user32::GetWindowLongPtrW(p r0, i -20) p .r3'
    System::Int64Op $3 & 0xFFFFFFFFFFFDDCFF
    Pop $3
    System::Call 'user32::SetWindowLongPtrW(p r0, i -20, p r3)'
    System::Call 'user32::LoadImageW(p 0, w R8, i 0, i R6, i R7, i 0x10) p .r4'
    ${If} $4 != 0
      SendMessage $0 ${BM_SETIMAGE} ${IMAGE_BITMAP} $4
    ${EndIf}
  ${EndIf}
  Pop $4
  Pop $3
FunctionEnd
!macroend
!insertmacro MP_STAMP_ONE ""
!insertmacro MP_STAMP_ONE "un."

; Place Back | Cancel | Next from the right, DPI-scaled, then stamp.
!macro MP_LAYOUT_AND_STAMP UN
Function ${UN}MpLayoutAndStamp
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  Push $6
  Push $7
  Push $9

  System::Call 'user32::GetDpiForWindow(p $HWNDPARENT) i .r9'
  ${If} $9 < 96
    StrCpy $9 96
  ${EndIf}

  IntOp $R6 120 * $9
  IntOp $R6 $R6 / 96
  IntOp $R5 108 * $9
  IntOp $R5 $R5 / 96
  IntOp $R7 32 * $9
  IntOp $R7 $R7 / 96
  IntOp $R4 10 * $9
  IntOp $R4 $R4 / 96
  IntOp $R3 14 * $9
  IntOp $R3 $R3 / 96

  System::Alloc 16
  Pop $1
  System::Call 'user32::GetClientRect(p $HWNDPARENT, p r1)'
  System::Call '*$1(i, i, i .r2, i .r3)'
  System::Free $1
  IntOp $R3 14 * $9
  IntOp $R3 $R3 / 96

  IntOp $6 $3 - $R3
  IntOp $6 $6 - $R7

  GetDlgItem $0 $HWNDPARENT 1
  IntOp $1 $2 - $R3
  IntOp $1 $1 - $R6
  System::Call 'user32::MoveWindow(p r0, i r1, i r6, i R6, i R7, i 1)'
  StrCpy $R8 "$PLUGINSDIR\mp-$7-$8.bmp"
  Call ${UN}MpStampOne

  GetDlgItem $0 $HWNDPARENT 2
  IntOp $1 $1 - $R4
  IntOp $1 $1 - $R5
  System::Call 'user32::MoveWindow(p r0, i r1, i r6, i R5, i R7, i 1)'
  StrCpy $R6 $R5
  StrCpy $R8 "$PLUGINSDIR\mp-cancel-$8.bmp"
  Call ${UN}MpStampOne

  GetDlgItem $0 $HWNDPARENT 3
  IntOp $1 $1 - $R4
  IntOp $1 $1 - $R5
  System::Call 'user32::MoveWindow(p r0, i r1, i r6, i R5, i R7, i 1)'
  StrCpy $R8 "$PLUGINSDIR\mp-back-$8.bmp"
  Call ${UN}MpStampOne

  Pop $9
  Pop $7
  Pop $6
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
FunctionEnd
!macroend
!insertmacro MP_LAYOUT_AND_STAMP ""
!insertmacro MP_LAYOUT_AND_STAMP "un."

Function MpSkinInit
  !insertmacro MP_UNPACK_ART
FunctionEnd

Function un.MpSkinInit
  !insertmacro MP_UNPACK_ART
FunctionEnd

Function MpPickPrimary
  Push $0
  Push $1
  Push $6
  GetDlgItem $0 $HWNDPARENT 1
  System::Call 'user32::GetWindowTextW(p r0, w .r6, i 80)'
  StrCpy $7 "next"
  ${StrLoc} $1 $6 "esinst" ">"
  ${If} $1 != ""
    StrCpy $7 "uninstall"
  ${EndIf}
  ${StrLoc} $1 $6 "ninst" ">"
  ${If} $1 != ""
    StrCpy $7 "uninstall"
  ${EndIf}
  ${StrLoc} $1 $6 "nstal" ">"
  ${If} $7 == "next"
    ${If} $1 != ""
      StrCpy $7 "install"
    ${EndIf}
  ${EndIf}
  ${StrLoc} $1 $6 "inali" ">"
  ${If} $1 != ""
    StrCpy $7 "finish"
  ${EndIf}
  ${StrLoc} $1 $6 "inish" ">"
  ${If} $1 != ""
    StrCpy $7 "finish"
  ${EndIf}
  Pop $6
  Pop $1
  Pop $0
FunctionEnd

Function MpSkin
  Push $7
  Push $8
  StrCpy $8 "es"
  ${If} $LANGUAGE = 1033
    StrCpy $8 "en"
  ${EndIf}
  GetDlgItem $0 $HWNDPARENT 1035
  ${If} $0 != 0
    ShowWindow $0 0
  ${EndIf}
  GetDlgItem $0 $HWNDPARENT 1036
  ${If} $0 != 0
    ShowWindow $0 0
  ${EndIf}
  Call MpPickPrimary
  Call MpLayoutAndStamp
  Pop $8
  Pop $7
FunctionEnd

Function un.MpSkin
  Push $7
  Push $8
  StrCpy $8 "es"
  ${If} $LANGUAGE = 1033
    StrCpy $8 "en"
  ${EndIf}
  StrCpy $7 "uninstall"
  GetDlgItem $0 $HWNDPARENT 1035
  ${If} $0 != 0
    ShowWindow $0 0
  ${EndIf}
  GetDlgItem $0 $HWNDPARENT 1036
  ${If} $0 != 0
    ShowWindow $0 0
  ${EndIf}
  Call un.MpLayoutAndStamp
  Pop $8
  Pop $7
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Rename "$INSTDIR\uninstall.exe" "$INSTDIR\unins000.exe"
  File "/oname=$INSTDIR\uninstall.exe" "${__FILEDIR__}\..\..\..\..\windows\uninstall-ui.exe"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
