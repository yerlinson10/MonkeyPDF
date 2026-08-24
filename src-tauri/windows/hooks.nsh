; Hallmark · component: nsis-installer · genre: playful-técnico · theme: press-shop banana
; Rounded pill bitmaps on cream footer (mock: Cancelar outline + Siguiente banana + monkey).

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

!macro MP_STAMP hwnd file bw bh
  Push $3
  Push $4
  ${If} ${hwnd} != 0
    System::Call 'uxtheme::SetWindowTheme(p ${hwnd}, w " ", w " ")'
    System::Call 'user32::GetWindowLongW(p ${hwnd}, i -16) i .r3'
    IntOp $3 $3 | 0x80
    System::Call 'user32::SetWindowLongW(p ${hwnd}, i -16, i r3)'
    System::Call 'user32::SetWindowPos(p ${hwnd}, p 0, i 0, i 0, i ${bw}, i ${bh}, i 0x16)'
    System::Call 'user32::LoadImageW(p 0, w "$PLUGINSDIR\${file}", i 0, i 0, i 0, i 0x10) p .r4'
    ${If} $4 != 0
      SendMessage ${hwnd} ${BM_SETIMAGE} ${IMAGE_BITMAP} $4
    ${EndIf}
  ${EndIf}
  Pop $4
  Pop $3
!macroend

Function MpSkinInit
  !insertmacro MP_UNPACK_ART
FunctionEnd

Function un.MpSkinInit
  !insertmacro MP_UNPACK_ART
FunctionEnd

Function MpSkin
  Push $0
  Push $1
  Push $6
  Push $7
  Push $8

  StrCpy $8 "es"
  ${If} $LANGUAGE = 1033
    StrCpy $8 "en"
  ${EndIf}

  GetDlgItem $0 $HWNDPARENT 1036
  ${If} $0 != 0
    ShowWindow $0 0
  ${EndIf}

  GetDlgItem $0 $HWNDPARENT 1028
  ${If} $0 != 0
    SetCtlColors $0 0x00141A11 0x00F2F8FA
  ${EndIf}

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

  ${If} $7 == "install"
    !insertmacro MP_STAMP $0 "mp-install-$8.bmp" 168 42
  ${ElseIf} $7 == "finish"
    !insertmacro MP_STAMP $0 "mp-finish-$8.bmp" 168 42
  ${ElseIf} $7 == "uninstall"
    !insertmacro MP_STAMP $0 "mp-uninstall-$8.bmp" 128 42
  ${Else}
    !insertmacro MP_STAMP $0 "mp-next-$8.bmp" 168 42
  ${EndIf}

  GetDlgItem $0 $HWNDPARENT 3
  !insertmacro MP_STAMP $0 "mp-back-$8.bmp" 128 42

  GetDlgItem $0 $HWNDPARENT 2
  !insertmacro MP_STAMP $0 "mp-cancel-$8.bmp" 128 42

  Pop $8
  Pop $7
  Pop $6
  Pop $1
  Pop $0
FunctionEnd

Function un.MpSkin
  Push $0
  Push $8
  StrCpy $8 "es"
  ${If} $LANGUAGE = 1033
    StrCpy $8 "en"
  ${EndIf}
  GetDlgItem $0 $HWNDPARENT 1
  !insertmacro MP_STAMP $0 "mp-uninstall-$8.bmp" 128 42
  GetDlgItem $0 $HWNDPARENT 3
  !insertmacro MP_STAMP $0 "mp-back-$8.bmp" 128 42
  GetDlgItem $0 $HWNDPARENT 2
  !insertmacro MP_STAMP $0 "mp-cancel-$8.bmp" 128 42
  Pop $8
  Pop $0
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_POSTINSTALL
!macroend

!macro NSIS_HOOK_PREUNINSTALL
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
!macroend
