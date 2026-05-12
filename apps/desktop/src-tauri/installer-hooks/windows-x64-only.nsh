!include LogicLib.nsh
!include x64.nsh

!macro NSIS_HOOK_PREINSTALL
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP|MB_OK "Liberty only supports 64-bit Windows. Please install it on a 64-bit version of Windows."
    Abort
  ${EndIf}
!macroend
