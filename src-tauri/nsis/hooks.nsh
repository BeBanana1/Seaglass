; Hooks de l'installeur NSIS de Tauri (bundle.windows.nsis.installerHooks).

; Windows garde l'icône d'un exe dans son cache, rangée par chemin : après une
; mise à jour qui change le logo, la barre des tâches et le Gestionnaire des
; tâches montraient encore l'ancienne (#139). On signale le fichier modifié,
; puis un changement d'associations, qui fait relire les icônes à l'Explorateur.
!macro NSIS_HOOK_POSTINSTALL
  ; SHCNE_UPDATEITEM, SHCNF_PATHW
  System::Call 'shell32::SHChangeNotify(i 0x00002000, i 0x0005, w "$INSTDIR\${MAINBINARYNAME}.exe", p 0)'
  ; SHCNE_ASSOCCHANGED, SHCNF_IDLIST
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
