; Hooks de l'installeur NSIS de Morph (voir bundle.windows.nsis.installerHooks).

!macro NSIS_HOOK_PREUNINSTALL
  ; Morph se retire du démarrage de Windows (réglage « Lancer à l'ouverture de session »).
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Morph"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Morph"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Langue choisie dans l'installeur : Morph l'adopte à son premier lancement (le fichier
  ; est ensuite supprimé ; une mise à jour ne change pas la langue choisie dans l'app).
  Push $0
  Push $1
  StrCpy $0 "fr"
  ${If} $LANGUAGE == ${LANG_ENGLISH}
    StrCpy $0 "en"
  ${EndIf}
  CreateDirectory "$APPDATA\com.underskydev.morph"
  FileOpen $1 "$APPDATA\com.underskydev.morph\installer-language" w
  FileWrite $1 $0
  FileClose $1
  Pop $1
  Pop $0
!macroend
