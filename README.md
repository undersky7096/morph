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
cargo run --bin morph-cli -- compress medium --target 20 clip.mp4   # compresse (light|medium|strong, --colors pour les PNG)
cargo test                                                 # tests unitaires
```

### Builds (GitHub Actions)

`.github/workflows/build.yml` construit l'installeur Windows (NSIS) et l'AppImage Linux à chaque push sur `main` (téléchargeables dans l'onglet Actions, artefacts `morph-nsis` / `morph-appimage`). Pousser un tag `vX.Y.Z` crée en plus une release brouillon avec les deux fichiers.

### Logo et icônes

Le logo est `branding/morph-logo.png` ; `python scripts/icons.py` en tire toutes les icônes (exe, installeur, Linux) et le masque utilisé par l'icône dynamique.

## Licence

GPL-3.0-or-later (voir [LICENSE](LICENSE)). ffmpeg est livré à part, en sidecar (build GPL de BtbN).

## Statut

Version actuelle : **0.5.0** (`0.<milestone>.0` ; changer avec `npm run bump -- x.y.z`).

- M6a : éditeur image (réglages, filtres, rotation/miroir/redressement, vignettage, grain, inversion ; texte, pinceau, gomme, surligneur, formes ; annuler/rétablir). Après chaque enregistrement : retour aux pétales, ou fichier suivant d'un lot.
- M5 : outil Compresser — PNG optimisé sans perte (oxipng) + option « réduire les couleurs » (libimagequant), JPEG/WebP/GIF par niveau, audio/vidéo par niveau ou par taille cible (2 passes, préréglage Discord 20 Mo), aperçu du poids, lots. Résultat `nom Compressed.ext`, rien d'enregistré si pas plus léger.
- M4 : menu d'actions (après conversion, ou pétale « Éditer ») + outil Recadrer dans une popup, bouton « Outil suivant ».

- M0 : squelette — build Windows (installeur NSIS) OK ; build Linux (AppImage) via GitHub Actions.
- M1 : moteur de conversion — images (crate `image` : png, jpg, webp, bmp, gif, tiff, ico, tga) et audio/vidéo (ffmpeg sidecar : mp4, webm, mkv, mov, avi, gif, mp3, wav, ogg, flac, m4a, opus), progression, noms sans écrasement.
- M3 : déclencheur Shift + glisser depuis l'explorateur/le bureau (Windows), halo de survol, dépôt toujours en copie.
- M2 : menu radial des formats (fenêtre transparente sous le curseur). Test : glisser des fichiers dans la fenêtre Morph, ou `morph.exe fichier1 fichier2`. Voir les milestones dans [projet.md](projet.md).
