# Morph

Convertisseur et éditeur de fichiers déclenché par un geste drag + Shift, avec menus radiaux.
Cadrage complet : voir [projet.md](projet.md).

## Stack

- Tauri v2 (Rust) + React + TypeScript (Vite)

## Développement

Prérequis : Node.js 22.12+ (ou 24), Rust (stable), outils de build C++ Visual Studio (Windows).

```sh
npm install
npm run tauri dev      # lance l'application en mode dev
npm run tauri build    # produit l'installeur (NSIS sous Windows, AppImage sous Linux)
```

## Statut

- M0 : squelette — build Windows (installeur NSIS) OK ; build Linux (AppImage) via GitHub Actions à venir. Voir les milestones dans [projet.md](projet.md).
