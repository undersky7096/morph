# Projet "Morph" — convertisseur/éditeur de fichiers (Windows + Linux)

## Contexte
Application de bureau inspirée de "Tangerine" (app Mac vue dans un reel Instagram) :
un convertisseur de fichiers ET un mini-éditeur avancé, déclenchés par un geste
drag + Shift n'importe où sur le bureau/l'explorateur, via des menus radiaux
(pie menus) façon éventail.

Nom retenu : **Morph**
Site de distribution prévu : sous-domaine de underskydev.com → morph.underskydev.com

## État d'avancement
- **M0 fait** (Windows) : squelette Tauri v2 + React 19 + TS, installeur NSIS fonctionnel, logo temporaire `assets/logo-temp.svg` (à remplacer par le vrai PNG 1024 via `npx tauri icon`). Reste : build Linux via GitHub Actions (repo GitHub à créer).
- **M1 fait** : moteur de conversion dans `src-tauri/src/convert/` (formats, raster, ffmpeg), commandes Tauri `get_targets` / `convert_files` (événement `convert-progress`), CLI `morph-cli`. ffmpeg = build BtbN GPL stable en sidecar (`npm run fetch:ffmpeg`), installeur ≈ 47 Mo à cause de lui.
- **M2 fait** : menu radial des formats dans une fenêtre `radial` dédiée (transparente, sans bordure, au-dessus de tout, centrée sous le curseur, cachée/réaffichée). Pétales animés (scale + fondu, motion), sous-menus (lots mixtes : un sous-menu par type ; vidéo : formats audio dans un sous-pétale « Audio »), fermeture au clic ailleurs / Échap, conversion avec progression dans le moyeu. Déclencheurs de dev : glisser-déposer dans la fenêtre principale, ou `morph.exe fichier1 fichier2…` (réutilisable pour « Ouvrir avec Morph » sous Linux).
- **M3 fait** (Windows) : `src-tauri/src/shift_drag.rs`. Hook souris bas niveau (thread dédié) ; un glisser parti de l'explorateur/bureau (processus explorer.exe) avec Shift enfoncé à n'importe quel moment fait apparaître une zone de dépôt native invisible (alpha 1/255, plein écran virtuel, sans focus). Elle a son propre `IDropTarget` qui répond **toujours DROPEFFECT_COPY** : Shift = « déplacer » pour l'explorateur, et wry laissait l'effet proposé au Drop → risque de suppression des originaux. Halo « M » (fenêtre `halo`, traversée par la souris) qui suit le curseur pendant le survol ; menu radial ouvert au point de dépôt. Testé en simulant de vrais glisser depuis l'explorateur (1 et 2 fichiers, avec/sans Shift) : originaux intacts.
- **M4 fait** — **v0.4.0** (convention : version `0.<milestone>.0`, `npm run bump -- x.y.z`). Menu d'actions : pétale « Éditer » au 1er niveau (fichier seul) et anneau d'actions automatique après une conversion réussie (✓ au centre). Actions : image → Recadrer/Compresser/Éditer, audio/vidéo → Compresser/Éditer, lot → Compresser seulement (Compress = M5, Edit = M6, grisés d'ici là). Fenêtre `tool` créée à la demande (sans décorations, barre de titre maison) ; outil Crop : cadre déplaçable, 8 poignées (4 en ratio fixe), grille des tiers, ratios Libre/1:1/16:9/9:16/4:3/3:4, saisie L×H, Entrée = Appliquer ; résultat `nom Cropped.ext` à côté de l'original, puis « Outil suivant » enchaîne sur ce fichier. Orientation EXIF désormais appliquée (conversions et crop). Erreurs JS non gérées renvoyées dans la console de l'app (`log_frontend_error`).
- **M5 fait** — **v0.5.0**. `src-tauri/src/edit/compress.rs` + `src/tools/compress/`. Niveaux Léger/Moyen/Fort. PNG : oxipng (sans perte, toujours) + « Réduire les couleurs » (imagequant, désactivé par défaut ; si la qualité minimale est inatteignable on garde l'original). JPEG : qualité 85/75/60 ; WebP : libwebp via ffmpeg ; GIF : palette 192/128/64 via ffmpeg. Audio : même format, débit par niveau (FLAC : sans perte niveau 12). Vidéo : x264 CRF 24/28/32 (VP9 pour webm, mpeg4 pour avi) ; wmv/flv/mpeg/3gp/ts → MP4. Taille cible (Mo = 10⁶ octets, par fichier) : 2 passes, marge 4 %, résolution plafonnée à 720p/480p si débit faible, erreur si cible impossible, rien à faire si déjà sous la cible. Non compressibles (à convertir d'abord) : BMP, TIFF, ICO, TGA, WAV. Rien n'est enregistré si le résultat n'est pas plus léger. Aperçu (image seule) : rendu + poids recalculés à chaque réglage, bouton « maintenir pour voir l'original ». La fenêtre d'outil reçoit désormais une liste de fichiers (`open_tool { tool, paths }`).
- Vision fonctionnelle et UX entièrement définie (voir ci-dessous).
- Identité visuelle en cours : logo "M" avec deux variantes réalisées en
  artifact Design (canvas.json + artboards .dc.html) :
  - Variante 1 : dégradé indigo/violet, style doux/arrondi.
  - Variante 2 (retenue, "premium/rapide") : fond quasi noir, panneau
    squircle façon glassmorphism (bordure en dégradé lumineux, reflet
    diagonal, ombre portée), halos de lumière mint/bleu flous en fond,
    "M" en dégradé mint→bleu électrique avec glow, traits de vitesse.
    Le "M" a été agrandi dans le format carré pour servir d'**icône de
    raccourci bureau**.
  - Formats produits : photo de profil (cercle 512x512) + bannière
    (1500x500). Artifact Claude.ai créé, non exporté en fichiers statiques
    pour l'instant (à faire : exporter en PNG/ICO/SVG pour l'intégration
    réelle dans l'installeur et les shortcuts).
- Repo GitHub privé `undersky7096/morph` ; CI GitHub Actions (`.github/workflows/build.yml`) : installeur NSIS + AppImage Linux à chaque push, release brouillon sur tag `v*`. Licence **GPL-3.0-or-later** (décidée le 2026-10-20) → `imagequant` autorisé pour M5.

## Fonctionnalités — spécification détaillée

### Déclenchement
- Shift + glisser-déposer un ou plusieurs fichiers vers un point vide de
  l'écran, depuis le **bureau ou l'explorateur Windows uniquement** (pas
  besoin de couvrir d'autres applications/navigateurs).
- Support multi-fichiers dès la V1.

### Menu radial 1 — Formats de conversion
- Pétales = formats cibles pertinents selon le(s) fichier(s) sélectionné(s).
- Les formats favoris (configurables) apparaissent en premier.
- Comportement par défaut pour un lot de fichiers : convertir tous les
  fichiers vers le **même format cible**.
- Si le lot contient des types réellement incompatibles entre eux (ex :
  image + audio vers un format qui n'existe que pour l'un des deux) :
  le menu doit proposer des **sous-menus par groupe de type détecté**
  (ex: un sous-menu "images → ?", un sous-menu "audio → ?"). Ce
  comportement est modifiable dans les paramètres.
- Formats visés à terme : le maximum possible (images, audio, vidéo en
  priorité pour le MVP ; documents, archives etc. ensuite), y compris les
  conversions croisées/inattendues (png→ico, txt→pdf, mp4→gif, vidéo→son
  comme mp4→wav...).
- Le menu n'est **pas modal** : un clic ailleurs le ferme sans action.
- Animation d'ouverture/fermeture : **scale + fondu en douceur**.

### Menu radial 2 — Actions d'édition
Apparaît juste après la conversion (ou sur un fichier déjà dans le bon
format). Outils prioritaires pour le MVP : **Compress, Edit, Crop**.
Outils reportés en V2 : Annotate, Redact, Metadata, Add BG.

- **Crop** : zone de recadrage interactive, ratios prédéfinis (1:1, 16:9,
  9:16, 4:3, 3:4) + dimensions libres W/H, bouton Apply.
- **Compress** : réglage qualité/taille cible, aperçu du poids avant/après.
- **Edit** (le plus gros morceau, un vrai mini-éditeur) :
  - Images : tous les effets d'image usuels (luminosité, contraste,
    saturation, filtres, rotation...) **+ ajout de texte et de dessins**.
  - Audio : découpe uniquement (trim).
  - Vidéo : découpe, réglage du volume, effets/couleurs, et changement
    de format.
- Chaque outil s'ouvre dans une **popup dédiée**, dans le thème actif de
  l'appli (couleurs + transparence personnalisables).
- **Enchaînement d'outils** : un bouton "outil suivant" dans la popup
  permet de passer directement d'un outil à l'autre (ex: Crop → Compress)
  sans repasser par le menu radial.
- Résultat enregistré par défaut **au même endroit que l'original, avec
  un suffixe** (ex: `Page 001 Cropped.jpg`). Option dans les paramètres
  pour demander l'emplacement à chaque fois (désactivée par défaut).

### Conversions longues
Pour les traitements lourds (ex: vidéo volumineuse), affichage de la
progression dans une **mini-fenêtre de progression séparée** (pas de
blocage de l'UI principale).

### Personnalisation
Fenêtre de réglages **classique** (pas une popup), accessible via une
**icône dans le system tray**, qui respecte elle-même le thème actif :
- Thèmes : clair/sombre (suivant Windows par défaut) + thème coloré/custom.
- Transparence des menus radiaux.
- Formats de fichiers affichés en premier dans le menu.
- Lancement au démarrage de l'OS (on/off).
- Emplacement de sauvegarde des fichiers convertis (on/off + dossier).
- Comportement sur les lots de types incompatibles (sous-menus vs autre).

### Installation
- **Windows** : vrai `.exe` avec assistant d'installation graphique
  complet (branding, dossier d'installation, raccourcis) qui pose des
  questions de personnalisation **avant le premier lancement** (thème,
  lancement au démarrage...).
- **Linux** : rendu équivalent — AppImage envisagé comme format le plus
  proche d'un "tout-en-un" sans gestion de paquet complexe.
- Le déclencheur Shift+drag global est spécifique à Windows pour la V1 ;
  sous Linux, un menu clic-droit via le gestionnaire de fichiers est
  envisagé en attendant une solution V2 (Wayland complique fortement un
  hook global équivalent).

### Usage / distribution
- Pour l'instant projet **perso**.
- Intention de distribuer à d'autres personnes à partir de la V2/V3, via
  un site de téléchargement hébergé sur underskydev.com.
- L'utilisateur possède déjà un **serveur** dont l'usage n'est pas encore
  défini précisément — pistes évoquées : synchronisation des réglages
  entre machines, vérification de mises à jour automatiques, traitement
  distant pour les conversions trop lourdes (vidéo 4K par ex.). À
  trancher plus tard, pas bloquant pour le MVP.

## Stack technique retenue

**Tauri (Rust + webview React)** — choix validé pour :
- binaire léger + vrai installeur natif (bundler Tauri → NSIS pour
  Windows, AppImage pour Linux) ;
- accès bas niveau Windows nécessaire pour le hook drag+Shift global
  (via la crate `windows-rs`) ;
- UI web (React + Framer Motion pour les animations) adaptée aux menus
  radiaux et popups d'édition basées sur canvas.

Architecture en 3 couches :

UI (webview React + Framer Motion)

Menu radial formats / actions
Popups Crop / Compress / Edit
Fenêtre Réglages
Mini-fenêtre de progression
│ IPC Tauri
Cœur applicatif (Rust)
Détection drag+Shift global (Windows, windows-rs)
System tray + démarrage auto
Gestionnaire de conversions
Gestion des réglages (fichier config)
│ appels process / lib
Moteur de conversion
ffmpeg (sidecar binaire) → audio/vidéo (conversion + trim + volume
effets/couleurs vidéo)
crate Rust image ou libvips → images
Konva.js (dans le webview) → édition interactive image (effets +
texte/dessin), export PNG/JPEG
(V2) LibreOffice headless → documents


Choix d'implémentation pour l'onboarding/installeur : installeur NSIS
classique et sobre (dossier, raccourcis, case "lancer au démarrage")
**puis** un écran d'onboarding stylé **dans l'appli elle-même** au
premier lancement, pour le thème et les formats favoris (rendu plus
premium qu'un assistant NSIS custom, et identique entre Windows/Linux).

## Découpage en tâches (milestones)

| # | Tâche | Contenu |
|---|---|---|
| M0 | Squelette | Projet Tauri + React, build produisant un .exe Windows et un AppImage Linux fonctionnels (fenêtre vide) |
| M1 | Moteur de conversion | Intégration ffmpeg + image crate, conversions image/audio/vidéo de base, testable en CLI |
| M2 | Menu radial (formats) | UI + animation scale/fondu, déclenché temporairement par un bouton de dev |
| M3 | Déclencheur réel | Hook Shift+drag Windows, multi-fichiers, sous-menus par groupe de type |
| M4 | Menu actions + Crop | 2e menu radial, popup Crop avec bouton "outil suivant" |
| M5 | Compress | Popup Compress |
| M6 | Edit | Le plus gros morceau : effets image + texte/dessin (Konva), trim audio, trim/volume/effets/format vidéo |
| M7 | Réglages + tray | Fenêtre de réglages complète, icône system tray, démarrage auto |
| M8 | Progression & erreurs | Mini-fenêtre de progression, gestion doublons/erreurs |
| M9 | Installeurs | NSIS Windows + onboarding, AppImage Linux équivalent |
| M10 | Polish | Tests sur machines propres, finitions |

## Ce qu'il reste à faire
1. Exporter le logo (variante premium validée) en fichiers statiques
   utilisables : PNG haute résolution, .ico (multi-tailles pour Windows),
   SVG pour les usages web (site underskydev.com).
2. Trancher les derniers détails UX mineurs non encore abordés si besoin
   (ex: comportement exact en cas d'erreur de conversion, gestion des
   doublons de nom de fichier).
3. Démarrer M0 : initialiser le projet Tauri, configurer le build
   cross-plateforme (Windows .exe + Linux AppImage).
4. Suivre les milestones M1 → M10 dans l'ordre, chacun livrant un
   incrément testable indépendamment.

## Chemin à suivre pour la suite (instructions pour Claude)
- **M6a fait** (2026-10-20, version toujours 0.5.0 jusqu'à 6c) : éditeur image
  `src/tools/edit/` — rendu WebGL (`renderer.ts` : flou 2 passes, netteté,
  couleurs, filtres, vignettage, grain, quarts de tour/miroirs/redressement
  avec agrandissement) + annotations Konva (`Canvas.tsx`, coordonnées en
  pixels de sortie), annuler/rétablir (`history.ts`, curseurs fusionnés),
  export PNG pleine résolution → `save_edited` (corps brut + en-tête
  `x-path`) réencode au format d'origine en `nom Edited.ext`. Éditeur chargé
  à la demande (Konva hors du bundle du radial).
- **Nouvel enchaînement (décision du 2026-10-20, remplace « Outil
  suivant »)** : après « Enregistrer », un fichier seul → la fenêtre se ferme
  et l'anneau d'actions réapparaît sous le curseur sur le résultat
  (`open_actions`, ✓ « Enregistré ») ; un lot → Recadrer/Éditer passent au
  fichier suivant (compteur 2/5, bouton « Passer »), puis pétales sur tous
  les résultats. Compresser (lot d'un coup) : bilan ~1,5 s puis pétales ; une
  erreur garde la fenêtre ouverte.
- **M6b fait** (2026-10-20, M6a validée par l'utilisateur) : éditeur audio
  `src/tools/edit/AudioEditor.tsx` + `Waveform.tsx` (forme d'onde canvas,
  règle, zoom Ctrl+molette, vue d'ensemble déplaçable) + `segments.ts`
  (passages gardés : retirer/garder la sélection, couper au curseur, bords
  déplaçables, annuler/rétablir). La lecture saute les passages retirés.
  Rust `src-tauri/src/edit/trim.rs` : `audio_peaks` (1 pic / 10 ms),
  `audio_preview` (WAV pour le WMA, illisible par le webview), `trim_audio`
  (atrim + concat, fondus de 8 ms aux coupes, débit/profondeur de la source,
  pochette gardée en MP3/FLAC/M4A) → `nom Edited.ext`. CLI :
  `morph-cli trim 0-12.5,20-31 fichier`.
- **M6c fait** (2026-10-20) — **v0.6.0** : éditeur vidéo
  `src/tools/edit/VideoEditor.tsx`. Découpe partagée avec l'audio
  (`Timeline.tsx` : lecture qui saute les trous, sélection, raccourcis, liste
  des passages). Aperçu = même rendu WebGL que l'image (`renderer.updateImage`
  à chaque image) ; lecture en continu via le protocole `asset` (fichier
  autorisé à la demande par `allow_media`, en-têtes CORS → WebGL OK), copie
  H.264 720p (`video_preview`) si le conteneur n'est pas lisible (AVI, WMV…).
  Onglets Découpe / Réglages / Filtres / Cadre (rotation, miroir, redresser,
  ratio Recadrer ou Ajuster avec fond flou/couleur, résolution par petit côté,
  sans agrandissement) / Son (vitesse 0,25–4× via atempo, volume 0–200 %,
  couper, EBU R128, fondus). Rust `src-tauri/src/edit/video.rs` : couleurs par
  LUT 3D `.cube` calculée avec les formules du shader, vignettage par masque
  PNG exact, netteté par `convolution`, flou `gblur`, grain `noise` ; même
  conteneur en sortie (x264 CRF 18, VP9, mpeg4, wmv2, mpeg2) → `nom Edited.ext`.
  `morph --edit|--crop|--compress fichiers` ouvre directement un outil.
- Après M6c (validé) : curseur de volume 0–200 % dans l'éditeur audio ; aperçu
  au-delà de 100 % via un gain Web Audio (audio et vidéo) ; export avec
  limiteur au-delà de 100 %.
- **Décisions M7** (validées par l'utilisateur le 2026-10-20) :
  - La fenêtre principale **devient les Réglages** : Morph démarre dans la
    zone de notification ; clic sur l'icône = Réglages ; fermer la fenêtre
    laisse Morph tourner ; « Quitter » dans le menu de l'icône.
  - Thème : Système / Clair / Sombre **+ couleur d'accent + teinte de fond**
    personnalisables.
  - Lots mixtes : sous-menus par type (défaut) **ou** « formats communs
    seulement » (option).
  - Enregistrement : **à côté de l'original** (défaut) / **dossier fixe** /
    **demander à chaque fois**.
- **M7 fait** (2026-10-20) — **v0.7.0**. `src-tauri/src/settings.rs` (JSON
  dans le dossier de config, événement `settings-changed`), `tray.rs` (clic =
  Réglages, menu Réglages/Quitter), plugins single-instance (une seconde
  instance relaie ses arguments : fichiers → menu, `--edit`… → outil, rien →
  Réglages), autostart (`morph --hidden` dans la clé Run), dialog. La fenêtre
  `main` (cachée au démarrage, masquée à la fermeture) est la fenêtre des
  Réglages (`src/App.tsx`) : Apparence (thème, accent, fond, opacité des
  menus), Menu des formats (favoris ordonnés, lots mixtes), Enregistrement (à
  côté / dossier fixe / demander : `chooseOutputDir` puis `outDir` transmis
  aux commandes, appliqué par `convert::with_output_dir`), Système (démarrage,
  Quitter). Thème : `src/settings.ts` pose `data-theme` et les variables
  `--accent`, `--accent-2`, `--bg`, `--surface-base`, `--radial-opacity` dans
  chaque fenêtre. Corrige aussi le menu radial vide de `morph fichier`.
- Retours sur M7 (2026-10-20, appliqués) : lancement à l'ouverture de session
  **activé par défaut** (au premier lancement, sans fichier de réglages) ;
  choisir Système/Clair/Sombre **efface l'accent et le fond personnalisés** ;
  transitions fluides (anneau du menu qui tourne selon le sens de navigation,
  moyeu en fondu, indicateurs glissants dans les Réglages et les onglets des
  éditeurs, fondu de thème, apparition des sections) en CSS/motion légers,
  désactivées si « réduire les animations » ; pétales d'outils centrés. Le menu
  ne se ferme plus sur une perte de focus qu'il n'a jamais eu (Windows refuse
  parfois le premier plan), et une seconde instance cède le premier plan à la
  première (`AllowSetForegroundWindow`). Réglages lus à la première demande
  (une fenêtre pouvait recevoir les valeurs par défaut au démarrage).
- À faire en M9 : l'installeur doit retirer l'entrée « Morph » de la clé Run à
  la désinstallation.
- **Décisions M8** (validées par l'utilisateur le 2026-10-20) :
  - Progression : le moyeu du menu d'abord ; si le traitement dépasse ~2 s
    (ou en cas d'erreur / de doublon à trancher), le menu se ferme et une
    **mini-fenêtre** prend le relais (fichier en cours, % par fichier, temps
    restant).
  - **Annulation** : tout le lot ou un seul fichier ; les fichiers terminés
    sont gardés, le fichier partiel est supprimé.
  - **Doublons** : nouveau réglage — garder les deux « (2) » (défaut) /
    remplacer / demander (Remplacer, Garder les deux, Ignorer, pour tous).
  - **Erreurs** en français clair (fichier illisible, disque plein, accès
    refusé…) avec « Copier le détail » et « Réessayer » ; **notification
    Windows** à la fin des traitements longs, clic = ouvrir le dossier.
- **M8 fait** (2026-10-20) — **v0.8.0**. `src-tauri/src/jobs.rs` : les
  conversions du menu tournent en arrière-plan (`start_conversion`, événement
  `job-update`), annulables (tout / un fichier), réessayables ; au-delà de 2 s,
  en cas d'erreur ou de doublon, ou si le menu est fermé, elles passent dans la
  mini-fenêtre `progress` (`src/progress/`, en bas à droite : % par fichier,
  temps restant, Réessayer / Copier le détail, Remplacer / Garder les deux /
  Ignorer pour un fichier ou tous, Ouvrir le dossier, Éditer…, fermeture auto
  5 s après un succès). Notification à la fin (`notify.rs` : toast Windows,
  clic = dossier ; notify-rust sous Linux). `convert/output.rs` : contexte
  d'une opération (dossier, doublons, annulation) posé par `output::scoped` ;
  « remplacer » met l'ancien fichier de côté et le restaure en cas d'échec ;
  ffmpeg est tué à l'annulation et le fichier partiel supprimé. Erreurs
  structurées (`ErrorInfo` : code, message clair, détail). Outils : bouton
  Annuler, bandeau de doublon, « Copier le détail » (`src/tools/save.tsx`).
  Réglage « Si le fichier existe déjà » (défaut : garder les deux).
- **Décisions M9** (validées par l'utilisateur le 2026-10-20) : logo
  temporaire conservé (logo définitif plus tard : `npx tauri icon logo.png` puis
  `python scripts/installer-images.py`) ; écran d'accueil en 4 étapes ;
  installeur sobre en français, par utilisateur ; Linux : intégration au
  bureau proposée depuis l'accueil et les Réglages.
- **M9 fait** (2026-10-20) — **v0.9.0**. Installeur NSIS (`bundle.windows.nsis`) :
  français, sans droits admin, images `src-tauri/installer/{sidebar,header}.bmp`
  générées par `scripts/installer-images.py`, raccourci Bureau en option et
  « Lancer Morph » en fin d'installation (modèle Tauri), « Supprimer les données
  de l'application » à la désinstallation ; hook `installer/hooks.nsh` qui
  retire Morph du démarrage à la désinstallation (rétabli au lancement suivant
  si les réglages l'avaient activé). Écran d'accueil `src/onboarding/` (réglage
  `onboarded`) : Bienvenue + démo animée du geste, Thème/accent, Favoris,
  Démarrage (+ intégration Linux) ; « Revoir l'écran d'accueil » dans Système.
  Linux (`src-tauri/src/linux.rs`) : entrée de menu avec « Ouvrir avec » (types
  MIME), actions Convertir / Éditer / Compresser dans Nautilus (scripts),
  Dolphin (service menu) et Nemo ; AppImage avec GStreamer embarqué
  (`bundleMediaFramework`). Réglages illisibles conservés en `settings.json.bak`.
- Non vérifié : AppImage et intégration Linux sur une vraie machine Linux (CI
  seulement) ; premier lancement après une mise à jour 0.8 → 0.9 a une fois
  affiché l'accueil avec les réglages par défaut (non reproduit).
- **Retouches 0.9.1–0.9.2** (2026-10-20, demandées par l'utilisateur) : un outil fermé
  avant la fin (bouton, Alt+F4) rouvre l'anneau des outils sur les mêmes fichiers
  (`open_tools`, `Ring::Tools`), tel qu'il était (niveau, moyeu) et là où il était
  (pas sous le curseur) ; appui + glisser sur le moyeu
  déplace tout le menu (`startDragging`, seuil 5 px). Le menu ne se ferme plus que
  si la perte de focus dure plus de 150 ms : déplacer la fenêtre la fait perdre et
  regagner aussitôt.
- **Décisions M10** (validées par l'utilisateur le 2026-10-20) :
  - Découpage : **10a** logo + icône dynamique, page « À propos », revue d'ensemble
    de l'interface, performance ; **10b** retirer le fond ; **10c** mises à jour
    automatiques, tests sur machine propre, release brouillon **v1.0.0**.
  - Logo définitif : `branding/morph-logo.png` (M #181818 sur fond transparent).
    Icône = M sur un carré arrondi : par défaut carré blanc + M noir (exe, installeur,
    raccourcis) ; en fonctionnement, l'icône (zone de notification, fenêtres) suit
    l'apparence : sombre → carré noir + M blanc ; fond personnalisé → carré de la
    couleur de fond, M blanc si elle est sombre, noir si elle est claire.
  - « À propos » : version, contact@underskydev.com, morph.underskydev.com, GitHub,
    licences des composants.
  - Retirer le fond : modèle IS-Net (libre, ~170 Mo) exécuté en local, téléchargé et
    vérifié à la première utilisation ; pétale « Détourer » dans l'anneau des outils
    + bouton dans l'éditeur d'image.
  - Mises à jour : plugin updater de Tauri via GitHub Releases, signées ; clé privée
    hors du dépôt, ajoutée en secret GitHub par Claude avec `gh secret set` (après
    accord au moment de le faire).
  - Tests machine propre : Windows Sandbox (activé par l'utilisateur) + AppImage
    dans WSL.
- **10a fait** (2026-10-20) — v0.10.0. Logo : `scripts/icons.py` génère depuis
  `branding/morph-logo.png` l'icône fixe (M noir sur carré blanc, `npx tauri icon`),
  les images de l'installeur et `icons/mark.png` ; `src-tauri/src/icon.rs` redessine
  l'icône (zone de notification, fenêtres) à chaque changement d'apparence ou de
  thème du système ; composant `Logo` dans l'interface (variables `--logo-tile` /
  `--logo-ink`). Page « À propos » (version, site, contact, licence, composants
  libres ; liens ouverts par `open_link`, liste blanche). Performance : fenêtre des
  Réglages créée à l'ouverture et détruite à la fermeture (−1 page web au repos :
  ~219 → ~183 Mo pour WebView2), code chargé par fenêtre (`main.tsx`, imports
  dynamiques). Le dépôt GitHub est privé : lien GitHub absent de « À propos », et
  à régler avant les mises à jour automatiques (10c).
- **10b fait** (2026-10-20) — v0.11.0. Détourage `src-tauri/src/edit/cutout.rs` :
  IS-Net (`isnet-general-use.onnx`, 178 648 008 octets, SHA-256 60920e99…964a,
  adresse des releases de rembg) téléchargé au premier détourage dans
  `%LOCALAPPDATA%/com.underskydev.morph/models`, vérifié, gardé en mémoire 90 s
  après usage. Prétraitement de rembg (1024², valeurs/max − 0,5). ONNX Runtime
  1.31.0 officiel (`npm run fetch:onnxruntime`, empreinte figée) chargé à la
  demande (`ort` en `load-dynamic`) : sous Windows `onnxruntime.dll` + runtime
  Visual C++ (msvcp140, msvcp140_1, vcruntime140, vcruntime140_1, pris dans les
  Build Tools) à côté de l'exe (`tauri.windows.conf.json`) ; sous Linux
  `onnxruntime/libonnxruntime.so` (`tauri.linux.conf.json`). Pourquoi : les binaires
  liés statiquement par `ort` imposent DirectML.dll et MSVCP140.dll à l'exe (Morph
  ne démarrerait plus sur un Windows sans runtime Visual C++) ; `tract` (Rust pur)
  prend 11,7 s par image contre 0,84 s. Pétale « Détourer » (outil `direct`) : tâche
  de fond `jobs::CUTOUT` → `nom Cutout.png`, note « téléchargement du modèle » dans
  la mini-fenêtre ; éditeur d'image : « Retirer le fond » / « Remettre le fond »
  (annulable, damier sous l'image), enregistrement en PNG si le format d'origine
  n'a pas de transparence (JPEG, BMP).
- **Décisions 10c** (validées par l'utilisateur le 2026-10-20) : le dépôt GitHub
  passe en public (fait par l'utilisateur ; règle aussi l'accès au code source exigé
  par la GPL), releases et mises à jour via GitHub ; Morph vérifie au démarrage puis
  toutes les 6 h, prévient (notification + Réglages) et n'installe que sur clic,
  jamais pendant un traitement ; Linux : build GitHub seulement (pas de WSL) ;
  pas de certificat de signature pour la v1.0.0 (avertissement SmartScreen accepté) ;
  tests sur machine propre (Windows Sandbox, à lancer en administrateur) quand
  l'utilisateur le dira.
- **10c en cours** (2026-10-20) — v0.12.0. Mises à jour : `tauri-plugin-updater`,
  `src-tauri/src/updates.rs` (vérification 30 s après le démarrage puis toutes les
  6 h, notification une fois par version → Réglages « À propos », installation sur
  clic refusée si un traitement tourne ou si un outil est ouvert, puis redémarrage),
  manifeste `…/releases/latest/download/latest.json` (`scripts/updater-json.mjs`,
  job `updater` de la CI sur un tag). Clé de signature : `~/.tauri/morph-updater.key`
  + `.password` (hors dépôt, à sauvegarder), secrets GitHub
  `TAURI_SIGNING_PRIVATE_KEY` / `_PASSWORD` ; sans secret, la CI construit sans
  fichiers de mise à jour. Notes de version : `.github/release-notes/vX.Y.Z.md`
  (release et manifeste). Test machine propre : `tests/sandbox/run.ps1` (Sandbox en
  administrateur, script `inside.ps1` automatique : installation silencieuse, premier
  lancement, conversions et détourage via `morph-cli`, désinstallation, rapport et
  captures). `morph-cli cutout`. README réécrit pour le dépôt public.
- **10c fait** (2026-10-21) : secrets de signature ajoutés, dépôt public. Test
  Windows Sandbox (Windows 11 sans runtime Visual C++) : installation, démarrage,
  conversions image et audio, détourage et désinstallation OK ; l'interface n'a pas pu
  s'afficher car WebView2 est inutilisable dans le Sandbox (dossier présent, donc
  l'installeur ne l'installe pas ; l'installeur officiel y échoue avec 0x80040902) —
  particularité du Sandbox, WebView2 étant présent sur Windows 10/11 à jour.
- **v1.0.0** : tag `v1.0.0` → release brouillon (installeurs, signatures,
  latest.json), à relire et publier ; puis vérifier que la 0.12.0 propose la mise à jour.
- **M11 — formats v2** (cahier des charges `integration-v2.md`, plan validé par
  l'utilisateur le 2026-10-21). Version `1.<étape>.0` : **11a** table centrale
  (`convert/catalog.rs`, exposée au webview par `format_catalog`), audio/vidéo
  (AAC, WMA, AIFF ; WMV, FLV, 3GP ; copie des flux sans réencodage quand le
  conteneur l'accepte), images (AVIF via ffmpeg, HEIC/HEIF en lecture, SVG via
  resvg, ICO multi-résolution), pétale « Plus › » au-delà de 8 formats ;
  **11b** documents (pandoc embarqué ; LibreOffice **détecté s'il est installé**,
  sinon pétales grisés + lien libreoffice.org ; PDF : pages → images (pdfium),
  fusion et compression (lopdf), **images → PDF**) ; **11c** archives (ZIP/7Z en
  création pour toute sélection, **dossiers acceptés** ; extraction ZIP/7Z/TAR/GZ/
  **TAR.XZ/TAR.BZ2** ; RAR en lecture seule via `bsdtar` du système — toujours
  présent sous Windows, sous Linux bsdtar/unrar/7z sinon message avec le paquet à
  installer). Pas de PDF → DOCX (résultat inutilisable). Moteurs : disponibilité
  vérifiée au démarrage, route absente du menu ou pétale grisé avec la raison.
- **11a fait** (2026-10-21) — v1.1.0. `convert/catalog.rs` (une ligne par format :
  extension, alias, type, moteurs lecture/écriture, sorties particulières, compressible,
  éditable, MIME Linux, codecs recopiables), `convert/engines.rs` (ffmpeg vérifié au
  démarrage ; route impossible → pétale grisé avec la raison, erreur `engine-missing`),
  `formats.rs` interroge la table ; `format_catalog` alimente `src/formats.ts` (chargé
  avant chaque fenêtre), les favoris des Réglages et les MIME de `linux.rs`. Ajouts : AAC,
  WMA, AIFF ; WMV, FLV, 3GP ; copie des flux sans réencodage (repli sur le réencodage si
  ffmpeg refuse) ; AVIF (libaom, transparence en second flux, relue par alphamerge) ;
  HEIC/HEIF en lecture (ffmpeg 9 : tuiles et rotation OK, profil ICC ignoré) ; SVG via
  resvg (agrandi à 1024 px sur le grand côté, au plus 8192) ; ICO 16→256 px. Menu : 9
  pétales au plus par anneau, le reste dans « Plus › » ; sorties particulières (vidéo →
  GIF) au 5e rang. HEIC/SVG : pas de Recadrer/Éditer (à convertir d'abord), Détourer oui.
  Mise à jour auto 0.12.0 → 1.0.0 vérifiée (Réglages affichaient 1.0.0).
- **Détourage « Qualité »** (2026-10-21, demandé après 11a : IS-Net jugé pas assez bon) :
  réglage Outils → Détourer « Efficacité » (IS-Net, 170 Mo, ~1-2 s) / « Qualité »
  (BiRefNet-general, MIT, 970 Mo, ~20 s ; prétraitement ImageNet, sortie en logits).
  Défaut : Efficacité. Un seul modèle en mémoire. RMBG-2.0 écarté (licence non
  commerciale). `morph-cli cutout --best`.
  Progression continue pendant les étapes muettes (`convert::ticking` : avance au rythme
  de la durée attendue/mesurée sans jamais atteindre la fin avant le résultat).
- **11b fait** (2026-10-21) — v1.2.0. Type `document` (ambre), familles Texte / Tableur /
  Présentation / PDF (`catalog::Family` : une conversion reste dans sa famille, plus le
  PDF). Routes (`formats::route`) : texte ↔ texte par pandoc (sidecar 3.12.1, GPL) sauf
  entre formats bureautiques où LibreOffice est préféré s'il est là ; DOC/XLS/PPT,
  tableurs, présentations et tout → PDF par LibreOffice (`convert/office.rs` : détecté par
  le registre / Program Files / PATH, recherché de nouveau tant qu'il manque, profil à
  part dans %TEMP%\morph-libreoffice, une conversion à la fois, délai 3 min) ; MD/HTML/
  TXT/EPUB → PDF via un ODT provisoire. Sans LibreOffice : pétale grisé « Nécessite
  LibreOffice », clic = page de téléchargement. PDF (`convert/pdf.rs`) : pages → PNG/JPG
  par pdfium chromium/7881 (200 dpi ; plusieurs pages → dossier `nom/nom 01.png`),
  fusion par pdfium (outil « Fusionner », au premier niveau pour plusieurs PDF →
  `nom Merged.pdf`), images → PDF par lopdf (une page A4 par image ; plusieurs images =
  un seul PDF, cible « pdf » traitée en lot), compression PDF dans Compresser (images
  réduites à 2400/1600/1100 px, JPEG 85/72/55). Installeur 83 Mo (pandoc). Non testé :
  menu sans LibreOffice installé, AppImage.
- **11c fait** (2026-10-21) — v1.3.0. Type `archive` (rose). `convert/archive.rs` :
  extraction ZIP (zip), 7Z (sevenz-rust2), TAR/TAR.GZ/TGZ/TAR.BZ2/TAR.XZ (tar + flate2,
  bzip2, lzma-rust2), fichier seul compressé (`notes.txt.gz` → `notes.txt`), RAR en lecture
  par bsdtar (System32	ar.exe sous Windows ; Linux : bsdtar, unrar ou 7z, sinon pétale
  grisé avec le paquet à installer). Chemins qui sortiraient du dossier refusés. Passage
  par un dossier provisoire : un seul élément dans l'archive → sorti tel quel, sinon
  dossier `nom/`. Outil « Extraire » (direct, au premier niveau pour une archive). Archive
  → ZIP/7Z (`photos.tar.gz` → `photos.zip`). « Archiver › » (ZIP, 7Z solide) pour toute
  sélection sauf des archives, dossiers acceptés (Shift + glisser et ligne de commande) :
  une archive pour le lot, nommée d'après le premier élément. Dossiers seuls → ZIP/7Z au
  premier niveau. Archives créées relues par le tar de Windows. Non testé : clic sur
  Extraire depuis le menu (refusé pendant que l'utilisateur utilisait le PC), AppImage.
- **Décisions M6** (validées par l'utilisateur le 2026-10-20) :
  - Découpage en sous-étapes, un commit + validation chacune :
    **6a** éditeur image, **6b** découpe audio, **6c** éditeur vidéo.
    Version 0.6.0 à la fin de 6c.
  - **Image — réglages** : luminosité, contraste, saturation, exposition,
    température, netteté, flou ; filtres prédéfinis en vignettes (N&B,
    sépia, vintage, froid, chaud, dramatique…) ; rotation 90°, miroir H/V,
    redressement fin (−45° à +45°) ; vignettage et grain ; **inverser les
    couleurs**.
  - **Image — annotation** : texte (police, taille, couleur, gras,
    contour/ombre ; déplaçable/redimensionnable/rotatif après coup),
    pinceau libre (+ gomme), formes (flèche, rectangle, ellipse, trait),
    surligneur.
  - **Découpe (audio et vidéo)** : **plusieurs segments** — on peut retirer
    des morceaux au milieu (garder A + C, enlever B), avec forme d'onde.
  - **Vidéo — effets** : mêmes réglages et mêmes filtres que l'image,
    vitesse (0,25× à 4×, son sans effet chipmunk), rotation et miroir,
    inverser les couleurs.
  - **Vidéo — format** : ratio (9:16, 1:1, 16:9…) avec les deux méthodes au
    choix « Recadrer » (cadre déplaçable) / « Ajuster » (bandes floues ou de
    couleur) **et** résolution (1080p, 720p, 480p…).
  - **Vidéo — son** : curseur de volume (0–200 %), couper le son, normaliser
    (EBU R128), fondus entrant/sortant.
  - Choix par défaut (non débattus) : édition non destructive avec
    annuler/rétablir, export pleine résolution dans le format d'origine,
    résultat `nom Edited.ext`.
- **Décisions prises pour M5 (réalisées)** (validées par l'utilisateur le 2026-10-19) :
  - **PNG** : optimisation sans perte toujours appliquée, **plus** une option
    « réduire les couleurs » (quantification façon TinyPNG, gain 60-80 %).
    Licence tranchée : Morph est GPL-3.0-or-later → utiliser `imagequant`.
  - **Vidéo** : les deux modes — curseur de qualité (léger / moyen / fort)
    **et** champ « taille cible en Mo » qui calcule le débit
    automatiquement. Préréglage Discord = **20 Mo** (valeur donnée par
    l'utilisateur, ne pas mettre 25).
  - Rappels spec : aperçu du poids avant/après, applicable à un lot,
    résultat `nom Compressed.ext` à côté de l'original, s'intègre dans
    l'enchaînement « Outil suivant » (retirer `comingIn: "M5"` dans
    `src/tools/registry.tsx`).
  - Pistes techniques : WebP avec perte via ffmpeg (la crate `image`
    n'encode le WebP qu'en sans perte), JPEG via la qualité de l'encodeur.
- Méthode de travail : une étape (milestone) à la fois, s'arrêter et
  attendre le feu vert, un commit par étape, poser les questions
  avant de coder.
- Build Linux : fait via GitHub Actions (repo `undersky7096/morph`). Logo définitif en attente : PNG
  1024×1024 fourni par l'utilisateur → `npx tauri icon <png>`.
- Respecter strictement les choix déjà validés ci-dessus (stack Tauri,
  ffmpeg + Konva, enchaînement d'outils via bouton "outil suivant",
  sous-menus par groupe de type pour les lots mixtes, etc.) — ne pas
  rouvrir ces décisions sans raison explicite de l'utilisateur.
- Avant de coder une nouvelle fonctionnalité, vérifier qu'elle correspond
  bien à un des milestones listés ; si l'utilisateur demande quelque
  chose hors scope V1, confirmer si c'est voulu pour la V1 ou à repousser
  en V2.
- Le logo "Morph" premium existe déjà en artifact Design sur claude.ai —
  demander son URL à l'utilisateur si besoin de le modifier à nouveau,
  plutôt que d'en recréer un depuis zéro.