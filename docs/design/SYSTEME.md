# Système visuel de Morph

Référence commune à l'application (`src/App.css`) et au site (`morph.underskydev.com/src/styles/global.css`).
Les deux dépôts n'ont pas de bibliothèque partagée : ils reprennent **les mêmes noms et valeurs de tokens**.
Toute modification se fait des deux côtés.

## Principes

- **Encre et papier.** Encre `#151515`, blanc cassé `#F1F0EB`, blanc `#FFFFFF`, gris chauds. Le contraste vient
  de la taille et de l'espace, pas du gras ni des couleurs.
- **Un seul accent**, bleu outremer, réservé aux états : élément actif, focus clavier, progression, sélection,
  quelques repères de marque. Les actions principales sont **pleines, couleur du texte** (encre en clair,
  blanc cassé en sombre).
- **Couleurs de type de fichier** conservées (reconnaissance dans le menu radial), accordées à la palette.
- **Chasse fixe pour les extensions** (`.HEIC → .WEBP`) et les nombres : détail propre à Morph.
- **Interdits** : orbes et halos flous, dégradés décoratifs, verre dépoli généralisé, néons, animations en boucle
  sans but, faux chiffres.

## Couleurs

| Token | Clair | Sombre | Usage |
|---|---|---|---|
| `--bg` | `#F1F0EB` | `#151515` | fond de fenêtre / page |
| `--surface-1` | `#F7F6F2` | `#1B1B1A` | barres latérales, barres d'outils |
| `--surface-2` (site : `--bg-soft`) | `#FFFFFF` | `#232321` (site `#1C1C1B`) | cartes, champs, listes |
| `--surface-sunken` (site : `--sunken`) | `#E8E7E1` | `#101010` | pistes, choix segmentés |
| `--text` (site : `--fg`) | `#151515` | `#F1F0EB` | texte |
| `--text-2` (site : `--fg-2`) | `#3D3C39` | `#CFCDC6` | texte secondaire long |
| `--muted` | `#6B6964` (site `#615F5A`) | `#9B9992` (site `#A3A19A`) | légendes, aides |
| `--line` / `--line-strong` | encre 12 % / 24 % | blanc cassé 10 % / 20 % | filets, bordures |
| `--accent` | `#2D45E0` | `#7B8CFF` | états actifs, focus, progression |
| `--on-accent` | `#FFFFFF` | `#151515` | texte sur l'accent |
| `--primary` / `--on-primary` | `#151515` / `#F1F0EB` | `#F1F0EB` / `#151515` | bouton principal |
| `--success` | `#2B7349` | `#5FBF84` | réussite |
| `--warning` | `#8F5B00` | `#E0A63A` | doublon, attention |
| `--danger` | `#C2362B` | `#FF6B5E` | erreur |
| `--kind-image` | `#2E8F5E` | `#5CC791` | images |
| `--kind-video` | `#1F7FB8` | `#4FB0E6` | vidéo |
| `--kind-audio` | `#7656D6` | `#A58CF5` | audio |
| `--kind-document` | `#B97D12` | `#E3AB45` | documents |
| `--kind-archive` | `#C4466F` | `#EE7AA0` | archives |

Contrastes vérifiés (WCAG 2.1) : texte et `--muted` ≥ 4,5:1 sur `--bg` et `--surface-2` dans les deux thèmes ;
`--accent` ≥ 4,5:1 sur le fond (6,1:1 en clair comme en sombre) et avec `--on-accent`.

L'application garde les **réglages personnels** : couleur d'accent et teinte du fond libres
(`settings.ts` dérive alors `--surface-*` du fond choisi et le texte sur l'accent de sa luminance).

## Typographie

- **Geist** (variable) pour l'interface et les textes, **Geist Mono** pour extensions, versions, nombres,
  libellés de section. Licence SIL OFL 1.1. Application : sous-ensembles latin et latin étendu embarqués
  (`src/assets/fonts`, 84 Ko). Site : paquets `@fontsource-variable/geist` et `geist-mono`.
- Titres en poids 600, approche négative (−0,025 à −0,045 em). Pas de 800.
- Site : `.display` (hero), `.headline` (sections), `.lede` (chapeau), `.eyebrow` (libellé mono « 01 — Formats »).

## Formes

| Token | Valeur | Usage |
|---|---|---|
| `--r-xs` | 4 px | touches, petits repères |
| `--r-sm` | 6 px | boutons, champs, puces de format |
| `--r-md` | 10 px | listes d'options, menus |
| `--r-lg` | 14 px | cartes, panneaux |

Ombres seulement pour ce qui flotte (`--shadow-float` : menu radial, halo, captures du site). En sombre, les
bordures portent la structure plutôt que les ombres.

## Mouvement

| Token | Valeur | Usage |
|---|---|---|
| `--ease` | `cubic-bezier(.3, 0, .2, 1)` | changements d'état |
| `--ease-out` | `cubic-bezier(.2, .8, .2, 1)` | entrées |
| `--t-fast` / `--t-base` / `--t-slow` | 120 / 200 / 320 ms | survol / panneaux / barres |

- Application : sobre. Indicateurs de sélection qui glissent (ressort `motion`), fondu des sections, anneau du
  menu qui tourne selon le sens de navigation, fondu de thème (View Transitions).
- Site : plus expressif mais joué **une fois** (scène du geste dans le hero, apparition au défilement), avec
  bouton « Rejouer ».
- `prefers-reduced-motion` : animations coupées ; la scène du hero affiche directement son état final.

## Composants de référence

- Choix segmenté (langue, thème) : piste `--surface-sunken`, pastille `--surface-2` qui glisse.
- Options (radio) : liste groupée dans un cadre, ligne active sur `--accent-soft`.
- Pétale du menu radial : surface translucide réglable, anneau intérieur à la couleur du type, extension en
  mono ; au survol, anneau plein et teinte, jamais de lueur.
- Barre de progression : 4 px, accent ; vert à la fin, ambre si doublon.
