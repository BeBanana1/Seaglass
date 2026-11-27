; Hooks of Tauri's NSIS installer (bundle.windows.nsis.installerHooks).

; Windows keeps an exe's icon in its cache, keyed by path: after an update
; that changes the logo, the taskbar and the Task Manager still showed the
; old one (#139). We signal the modified file, then an association change,
; which makes Explorer reread the icons.
!macro NSIS_HOOK_POSTINSTALL
  ; SHCNE_UPDATEITEM, SHCNF_PATHW
  System::Call 'shell32::SHChangeNotify(i 0x00002000, i 0x0005, w "$INSTDIR\${MAINBINARYNAME}.exe", p 0)'
  ; SHCNE_ASSOCCHANGED, SHCNF_IDLIST
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
