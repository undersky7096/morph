# Morph

Convertisseur et éditeur de fichiers déclenché par un geste drag + Shift, avec menus radiaux.
Cadrage complet : voir [projet.md](projet.md).

## Stack

- Tauri v2 (Rust) + React + TypeScript (Vite)

## Développement

Prérequis : Node.js 22.12+ (ou 24), Rust (stable), outils de build C++ Visual Studio (Windows).

```sh
npm install
npm run fetch:ffmpeg   # télécharge le sidecar ffmpeg (src-tauri/binaries/, non versionné)
npm run tauri dev      # lance l'application en mode dev
npm run tauri build    # produit l'installeur (NSIS sous Windows, AppImage sous Linux)
```

### Tester le moteur de conversion en CLI

```sh
cd src-tauri
cargo run --bin morph-cli -- targets photo.png clip.mov   # formats proposés (JSON)
cargo run --bin morph-cli -- webp photo.png               # convertit à côté de l'original
cargo test                                                 # tests unitaires
```

### Builds (GitHub Actions)

`.github/workflows/build.yml` construit l'installeur Windows (NSIS) et l'AppImage Linux à chaque push sur `main` (téléchargeables dans l'onglet Actions, artefacts `morph-nsis` / `morph-appimage`). Pousser un tag `vX.Y.Z` crée en plus une release brouillon avec les deux fichiers.

## Licence

GPL-3.0-or-later (voir [LICENSE](LICENSE)). ffmpeg est livré à part, en sidecar (build GPL de BtbN).

## Statut

Version actuelle : **0.4.0** (`0.<milestone>.0` ; changer avec `npm run bump -- x.y.z`).

- M4 : menu d'actions (après conversion, ou pétale « Éditer ») + outil Recadrer dans une popup, bouton « Outil suivant ».

- M0 : squelette — build Windows (installeur NSIS) OK ; build Linux (AppImage) via GitHub Actions. Logo temporaire (`assets/logo-temp.svg`).
- M1 : moteur de conversion — images (crate `image` : png, jpg, webp, bmp, gif, tiff, ico, tga) et audio/vidéo (ffmpeg sidecar : mp4, webm, mkv, mov, avi, gif, mp3, wav, ogg, flac, m4a, opus), progression, noms sans écrasement.
- M3 : déclencheur Shift + glisser depuis l'explorateur/le bureau (Windows), halo de survol, dépôt toujours en copie.
- M2 : menu radial des formats (fenêtre transparente sous le curseur). Test : glisser des fichiers dans la fenêtre Morph, ou `morph.exe fichier1 fichier2`. Voir les milestones dans [projet.md](projet.md).
