; Gives Modbench's file types their own icons in Explorer: Tauri's file
; associations use the app's icon. The classes are the associations' names in
; tauri.conf.json; the icons are installed as resources (tauri.windows.conf.json).
; Uninstalling removes the classes, icons included.

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "Software\Classes\Modbus Map\DefaultIcon" "" "$INSTDIR\file-icons\mbmap.ico"
  WriteRegStr SHCTX "Software\Classes\Modbench Simulator Setup\DefaultIcon" "" "$INSTDIR\file-icons\mbsim.ico"
  ; SHCNE_ASSOCCHANGED: Explorer shows the new icons without a restart
  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)"
!macroend
