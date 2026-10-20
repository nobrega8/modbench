; Gives Modbench's file types their own icons in Explorer: Tauri's file
; associations use the app's icon. The classes are the associations' names in
; tauri.conf.json; the icons are installed as resources (tauri.windows.conf.json).
; Uninstalling removes the classes, icons included.

; Windows will not replace a program that is running. An update (or an uninstall)
; fails with "Error opening file for writing" when the installed modbench-cli.exe
; is still running, for example as a simulator, a loop or an MCP server. Stop the
; copy in the install folder first; copies run from elsewhere are left alone.
!macro ModbenchStopCli
  nsExec::Exec `powershell -NoProfile -NonInteractive -Command "Get-Process modbench-cli -ErrorAction SilentlyContinue | Where-Object { $$_.Path -eq '$INSTDIR\modbench-cli.exe' } | Stop-Process -Force"`
  Pop $0
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro ModbenchStopCli
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro ModbenchStopCli
!macroend

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "Software\Classes\Modbus Map\DefaultIcon" "" "$INSTDIR\file-icons\mbmap.ico"
  WriteRegStr SHCTX "Software\Classes\Modbench Simulator Setup\DefaultIcon" "" "$INSTDIR\file-icons\mbsim.ico"
  ; SHCNE_ASSOCCHANGED: Explorer shows the new icons without a restart
  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)"
!macroend
