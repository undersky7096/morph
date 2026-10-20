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
- **Reprendre à M9 (installeurs : NSIS + onboarding, AppImage)**.
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