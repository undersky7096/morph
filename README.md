# Morph

[![Morph](https://morph.underskydev.com/og-fr.png)](https://morph.underskydev.com)

**[Télécharger](https://morph.underskydev.com/download/)** ·
[Site](https://morph.underskydev.com) ·
[Formats et conversions](https://morph.underskydev.com/convert/) ·
[Avis](https://morph.underskydev.com/reviews/) ·
[Feuille de route](https://morph.underskydev.com/roadmap/) ·
[FAQ](https://morph.underskydev.com/faq/)

Convertir et retoucher ses images, sons, vidéos et documents en un geste, sans ouvrir d'application :
on glisse des fichiers en maintenant **Shift**, on les lâche n'importe où, et un menu radial
propose les formats et les outils. Windows et Linux. Site : [morph.underskydev.com](https://morph.underskydev.com).

## Ce que fait Morph

- **Convertir** : images (PNG, JPG, WebP, AVIF, GIF, BMP, TIFF, ICO, TGA ; HEIC et SVG en
  lecture), audio (MP3, WAV, FLAC, OGG, AAC, M4A, Opus, WMA, AIFF), vidéo (MP4, WebM, MKV,
  MOV, AVI, WMV, FLV, 3GP, GIF, sans réencodage quand c'est possible), extraction du son
  d'une vidéo ; documents (DOCX, ODT, RTF, Markdown, HTML, TXT, EPUB) entre eux, tableurs
  (XLSX, ODS, CSV) et présentations (PPTX, ODP), tout document en PDF, pages d'un PDF en
  images, images en un PDF. Formats favoris en premier, lots de fichiers mélangés.
- **PDF** : fusionner plusieurs PDF, compresser un PDF.
- **Archives** : créer une archive ZIP ou 7Z de fichiers et de dossiers ; extraire ZIP, 7Z,
  TAR (GZ, BZ2, XZ) et RAR ; changer le format d'une archive (RAR → ZIP…).
- **Recadrer** une image, **Détourer** (retirer le fond, en local), **Compresser** (images,
  audio, vidéo, taille cible), **Éditer** : retouches et annotations d'image, découpe et
  volume audio, montage léger de vidéo (découpe, cadrage, vitesse, effets, son).
- Traitements en arrière-plan avec mini-fenêtre de progression, annulation, doublons et
  erreurs expliqués ; notification à la fin.
- Thème clair / sombre / couleurs personnalisées, lancement à l'ouverture de session,
  mises à jour automatiques (proposées, jamais imposées).

Tout se passe sur l'ordinateur : aucun fichier n'est envoyé. Le modèle de détourage est
téléchargé une seule fois, au premier usage. Les formats bureautiques (DOCX ↔ ODT, XLSX,
PPTX…) et l'export PDF des documents utilisent [LibreOffice](https://www.libreoffice.org)
s'il est installé (gratuit) ; sans lui, ces formats apparaissent grisés dans le menu.

## Installation

Télécharger l'installeur de la [dernière version](https://github.com/undersky7096/morph/releases/latest) :

- **Windows** : `Morph_x.y.z_x64-setup.exe` (installation pour l'utilisateur, sans droits
  administrateur). L'installeur n'est pas signé : Windows peut afficher « Windows a protégé
  votre ordinateur » ; cliquer sur « Informations complémentaires » puis « Exécuter quand même ».
- **Linux** : `Morph_x.y.z_amd64.AppImage`, à rendre exécutable puis lancer ; « Ajouter Morph
  au système » (écran d'accueil ou Réglages) crée l'entrée de menu et les actions du clic
  droit (Nautilus, Dolphin, Nemo).

## Développement

Prérequis : Node.js 22.12+ (ou 24), Rust stable, et sous Windows les outils de build C++ de
Visual Studio (Build Tools).

```sh
npm install
npm run fetch:ffmpeg       # sidecar ffmpeg (src-tauri/binaries/, non versionné)
npm run fetch:onnxruntime  # ONNX Runtime pour le détourage (+ runtime Visual C++ sous Windows)
npm run fetch:pandoc       # sidecar pandoc (documents texte)
npm run fetch:pdfium       # pdfium (pages de PDF en images, fusion)
npm run tauri dev          # application en mode développement
```

Construire l'installeur demande la clé qui signe les mises à jour (voir plus bas) :

```sh
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/morph-updater.key)" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat ~/.tauri/morph-updater.password)" \
npm run tauri build        # NSIS sous Windows, AppImage sous Linux
```

Sans la clé : `npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}'`.

### Moteur en ligne de commande

```sh
cd src-tauri
cargo run --bin morph-cli -- targets photo.png clip.mov             # formats proposés (JSON)
cargo run --bin morph-cli -- webp photo.png                         # convertit à côté de l'original
cargo run --bin morph-cli -- compress medium --target 20 clip.mp4   # compresse
cargo run --bin morph-cli -- merge a.pdf b.pdf                      # fusionne des PDF
cargo run --bin morph-cli -- archive-zip Dossier photo.jpg           # archive la sélection
cargo run --bin morph-cli -- extract photos.tar.gz                  # extrait
cargo test                                                          # tests
```

### Logo et icônes

Le logo est `branding/morph-logo.png` ; `python scripts/icons.py` en tire toutes les icônes
(exe, installeur, Linux) et le masque de l'icône dynamique.

### Builds et versions

`.github/workflows/build.yml` construit l'installeur Windows et l'AppImage à chaque push sur
`main` (artefacts de l'onglet Actions). Pour une version :

1. `npm run bump -- x.y.z`, notes dans `.github/release-notes/vx.y.z.md`, commit ;
2. pousser le tag `vx.y.z` : la build crée une **release brouillon** avec les deux
   installeurs, leurs signatures et `latest.json` (manifeste des mises à jour) ;
3. relire puis publier la release : les Morph installés la proposent alors en mise à jour.

Les mises à jour sont signées : clé privée hors du dépôt (`~/.tauri/morph-updater.key` et
son mot de passe, **à sauvegarder** : sans elle, plus aucune mise à jour ne pourra être
publiée), en secrets du dépôt `TAURI_SIGNING_PRIVATE_KEY` et
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` ; clé publique dans `src-tauri/tauri.conf.json`.

## Licence

GPL-3.0-or-later (voir [LICENSE](LICENSE)). Composants : ffmpeg (sidecar, build GPL de
BtbN), ONNX Runtime (MIT, embarqué), modèle de détourage IS-Net (Apache-2.0, téléchargé au
premier usage) ; liste complète dans Réglages → À propos. Cadrage et historique du projet :
[projet.md](projet.md).
