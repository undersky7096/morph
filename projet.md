# Projet "Morph" — convertisseur/éditeur de fichiers (Windows + Linux)

## Contexte
Application de bureau inspirée de "Tangerine" (app Mac vue dans un reel Instagram) :
un convertisseur de fichiers ET un mini-éditeur avancé, déclenchés par un geste
drag + Shift n'importe où sur le bureau/l'explorateur, via des menus radiaux
(pie menus) façon éventail.

Nom retenu : **Morph**
Site de distribution prévu : sous-domaine de underskydev.com → morph.underskydev.com

## État d'avancement
- **M0 en cours** : squelette Tauri v2 + React 19 + TS, installeur NSIS Windows fonctionnel. Reste : icônes (logo) et build Linux via GitHub Actions.
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
- Repo git local initialisé (pas encore de remote GitHub).

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
- Reprendre directement à **M0** sauf indication contraire de
  l'utilisateur.
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