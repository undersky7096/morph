; Hooks de l'installeur NSIS de Morph (voir bundle.windows.nsis.installerHooks).

!macro NSIS_HOOK_PREUNINSTALL
  ; Morph se retire du démarrage de Windows (réglage « Lancer à l'ouverture de session »).
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Morph"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Morph"
!macroend
