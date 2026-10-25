# Mesures de référence — Morph 1.6.1 et site (25 octobre 2026)

Mesures prises **avant toute modification**, pour comparer après la refonte. Aucun fichier de code
ou de configuration du projet n'a été modifié, rien n'a été compilé ni installé, rien n'a été déployé.

## 1. Conditions

| | |
|---|---|
| Machine | Ryzen 7 7435HS (8 cœurs / 16 fils), 16 Go, RTX 3050 Laptop 4 Go, SSD |
| Système | Windows 11 Pro build 26300, 1920×1080 à 100 %, secteur, mode « Utilisation normale » |
| WebView2 | 155.0.4283.62 |
| Morph | 1.6.1 installée (`%LOCALAPPDATA%\Morph`), réglages de l'utilisateur (thème sombre, détourage « best », GPU activé) |
| Site | https://morph.underskydev.com, production, lecture seule ; Lighthouse 13.5.0, Chrome installé |

**Unités.** Mémoire = octets privés validés (« private ») de tout l'arbre de processus : morph.exe,
ses WebView2 et les outils lancés (ffmpeg, pandoc…). Processeur = % d'un cœur (100 = un cœur plein,
1600 = toute la machine). Échantillonnage toutes les 0,25 s.

**Fichiers de test** (synthétiques, générés par `scripts/gen_media.py` avec le ffmpeg de Morph, hors dépôt) :
A1 WAV 60 min (691 Mo), A2 MP3 60 min (86 Mo), A3 WMA 20 min (22,5 Mo), V1 MP4 1080p 20 min (1,5 Go),
V2 MP4 4K 3 min (684 Mo), V3 MP4 15 s, I1 = 30 JPEG 4000×3000 + 20 PNG 2000×2000.
Les images et vidéos sont bruitées exprès (entropie proche de vraies photos) : les tailles de sortie
sont plutôt pessimistes.

## 2. Application — résultats

### 2.1 Repos et fenêtres

| Mesure | Médiane | Passes / écart | Remarque |
|---|---|---|---|
| Repos, Réglages fermés (120 s) | **342 Mo** (WebView 263, morph 80), 9 processus | 1 × 120 s | après une session d'utilisation |
| Processeur au repos | **25,7 % d'un cœur** en moyenne, pics à 101 % | 1 × 120 s | aucune fenêtre visible : **anormal, à investiguer** |
| Démarrage à froid → Réglages visibles | **0,61 s** | 0,64 / 0,61 / 0,61 (écart 4 %) | |
| Mémoire 30 s après démarrage à froid | **239 Mo** au pic, 226 Mo ensuite | 3 passes, écart 2 % | processeur ~24 % d'un cœur, Réglages ouverts |
| Ouverture des Réglages (instance chaude) | 0,31 s | 0,41 / 0,31 / 0,31 (écart 32 %) | |
| Réglages ouverts (120 s) | 423 Mo au pic, 402 Mo en moyenne | 1 × 120 s | |
| Après fermeture des Réglages | 367 Mo | 1 × 120 s | **+25 Mo** retenus par rapport au repos |

Ouverture des outils (Recadrer, Compresser, éditeurs image / audio / vidéo, 3 passes chacun) : la
fenêtre native apparaît en 0,03 à 0,06 s, mais ce chiffre **ne mesure pas** le moment où l'outil est
utilisable. Les captures à +1 s et +3 s (`shots/tool-*`) le montrent : l'éditeur vidéo est encore noir
à 1 s et affiché à 3 s ; l'éditeur audio sur A2 (60 min) affiche encore « Analyse du fichier… » à 3 s.

### 2.2 Éditeur audio — problème confirmé

Ouverture de l'éditeur, 60 s d'observation, 3 passes par fichier :

| Fichier | Pic mémoire (médiane) | Écart | Pic WebView | Pic morph.exe | Encore ouvert à 60 s |
|---|---|---|---|---|---|
| A1 WAV 60 min | **7 059 Mo** | 9,3 % | 3 641 Mo | 6 700 Mo | ~1 072 Mo |
| A2 MP3 60 min | 1 114 Mo | 0,3 % | 602 Mo | 757 Mo | ~493 Mo |
| A3 WMA 20 min | 1 911 Mo | 23 % | 793 Mo | 1 564 Mo | ~559 Mo |

Après fermeture, le passage suivant repart d'environ 417 Mo (contre 342 Mo au repos).
Cause dans le code : `read_media` lit le fichier entier (`std::fs::read`) et l'envoie par IPC à la WebView,
qui le décode à son tour. Un WAV de 60 min passe donc plusieurs fois en mémoire.

Couper et exporter (A2, garder ~10 min, manipulation de l'utilisateur) : fichier de 14,1 Mo écrit en
**4,5 s**, pic de 769 Mo sur toute la manipulation (dominé par l'éditeur ouvert).

### 2.3 Moteur seul (morph-cli, sans interface)

Repos de 10 à 60 s entre deux passes ; sorties supprimées après chaque passe.

| Cas | Médiane | Passes / écart | Pic mémoire | Processeur moyen |
|---|---|---|---|---|
| V1 → MKV (copie sans réencodage) | 1,64 s | 2,94 / 1,64 / 1,64 (79 %, 1ʳᵉ passe à froid) | 35 Mo | ~1 cœur |
| V2 → MKV (copie) | 1,24 s | 3 passes, 10 % | 51 Mo | <1 cœur |
| V3 → WebM | 32,9 s | 3 passes, 2,8 % | 560 Mo | ~7,9 cœurs |
| V2 → AVI | 40,9 s | 3 passes, 4,9 % | 856 Mo | ~9,5 cœurs |
| V2 → Compresser « moyen » | 141,8 s | 3 passes, 5,6 % | **4 327 Mo** | ~14,7 cœurs |
| V1 → Compresser « moyen » | 279 s | 1 passe | 1 184 Mo | ~14,9 cœurs |
| V1 → Compresser à 20 Mo | 111 s | 1 passe | 369 Mo | ~12,9 cœurs |
| A1 WAV → MP3 | 18,3 s | 3 passes, 1,3 % | 24 Mo | ~1,1 cœur |
| I1 (50 images) → WebP | 6,1 s | 3 passes, 10 % | 115 Mo | **~0,9 cœur** |
| I1 → Compresser « moyen » | 53,1 s | 3 passes, 0,4 % | 123 Mo | **~1,4 cœur** |

Tous les codes de sortie = 0. Le pic de 4,3 Go de la compression 4K est celui du processus ffmpeg (x264).

### 2.4 Depuis l'interface (manipulations de l'utilisateur, 1 passe chacune)

| Mesure | Résultat |
|---|---|
| Annuler V1 → WebM après ~1 min 40 | ffmpeg disparaît à t = 111,3 s, le fichier partiel (26 Mo) est supprimé à t = 111,1 s ; aucun reste. L'utilisateur a vu l'annulation instantanée et la fenêtre se fermer quelques secondes après. |
| Lot I1 → WebP par glisser-déposer | ~6,4 s de la première à la dernière sortie (moteur seul : 6,1 s) ; pic 389 Mo |
| 3 dépôts quasi simultanés (V2 → AVI, A1 → MP3, I1 → WebP) | Les 3 tâches tournent **en même temps** (2 ffmpeg actifs) : AVI 46,5 s (seul : 40,9), MP3 35,5 s (seul : 18,3), WebP ~11 s (seul : 6,1). Fin de tout en ~51 s contre 65 s en série (délai humain entre les dépôts inclus). Pic 1 356 Mo. |

Remarques : le réglage « doublons : garder » a bien produit `capture_03 (2).webp` ; inclure un WebP dans un
lot vers WebP donne l'erreur « impossible de convertir du webp en webp » (formulation à revoir).

### 2.5 Progression réelle ou estimée (lecture du code, mesure 8)

| Traitement | Progression | Source |
|---|---|---|
| Audio / vidéo (ffmpeg), compression, découpe, montage vidéo | **réelle** (temps encodé / durée) | `convert/ffmpeg.rs:298` |
| Compression en deux passes (taille cible) | réelle, 0–50 % puis 50–100 % | `edit/compress.rs:312` |
| Archives (extraire, créer, reconditionner) | réelle (octets ou entrées) | `convert/archive.rs` |
| PDF → images, fusion, images → PDF, compression PDF | réelle (par page / fichier) | `convert/pdf.rs` |
| Téléchargement d'un modèle de détourage | réelle (octets reçus) | `edit/cutout.rs:263` |
| Détourage vidéo | réelle (images traitées) | `edit/cutout.rs:614` |
| Documents via pandoc | **estimée** (2 s supposées, plafond 95 %) | `convert/pandoc.rs:74` |
| Documents via LibreOffice | **estimée** (6 s supposées, plafond 95 %) | `convert/office.rs:136` |
| Détourage image : chargement image, modèle, inférence | **estimée** (durées supposées) | `edit/cutout.rs:333,418,440` |
| Image → image (une seule image) | 0 % puis 100 % | `convert/mod.rs` |
| Lot de fichiers | par fichier terminé, fichiers traités **l'un après l'autre** | `jobs.rs` |

### 2.6 Constat sur le WebP

JPG → WebP produit des fichiers **~8 fois plus lourds** que la source (photo de 2,9 Mo → WebP de 22 Mo ;
lot I1 : 770 Mo de WebP). Cause : `convert/raster.rs:46` enregistre via la bibliothèque `image`, dont
l'encodeur WebP est **sans perte uniquement**. Les images de test bruitées accentuent l'écart, mais le
mode sans perte est confirmé par le code.

### 2.7 Captures de l'interface

`shots/ui/` : Réglages, Recadrer, Compresser, éditeurs image / audio / vidéo, anneau et fenêtre de
progression, en **sombre 100 %** et **clair 100 %**. L'anneau étant transparent, l'Explorateur visible
derrière lui a été flouté sur la bande de gauche (volet de navigation). Le halo n'a pas pu être capturé (il disparaît en
moins d'une seconde). L'échelle 150 % a été abandonnée (voir § 5). Les captures anciennes de chaque
mesure sont dans `shots/` (Réglages, outils à +1 s / +3 s, éditeur audio, démarrages à froid).

## 3. Site — résultats

### 3.1 Lighthouse (production)

**4 pages × 3 passes**, 75 s entre deux chargements (`site-lighthouse.json`). Médianes, écart entre passes entre parenthèses :

| Page | Profil | Perf. | Access. | LCP | CLS | Poids | JS |
|---|---|---|---|---|---|---|---|
| / | mobile | 88 (10 %) | 100 | 3 296 ms (60 %) | 0,098 | 893 Ko | 15 Ko |
| / | ordinateur | **75** (1 %) | 100 | 972 ms | **0,711** | 893 Ko | 15 Ko |
| /download/ | mobile | 100 | 95 | 1 352 ms | 0 | 103 Ko | 3 Ko |
| /download/ | ordinateur | 100 | 95 | 342 ms | 0,001 | 103 Ko | 3 Ko |
| /convert/png-to-webp/ | mobile | 98 | 100 | 1 230 ms | 0 | 69 Ko | 6 Ko |
| /convert/png-to-webp/ | ordinateur | 100 | 100 | 270 ms | 0,001 | 69 Ko | 6 Ko |
| /formats/ | mobile | 98 | 95 | 1 230 ms | 0 | 71 Ko | 3 Ko |
| /formats/ | ordinateur | 100 | 95 | 330 ms | 0,001 | 71 Ko | 3 Ko |

Bonnes pratiques et SEO : 100 partout. TBT : 0 ms partout.

**Passe unique sur les 10 pages** (`site-lighthouse-pass1-10pages.json`, voir § 5) : mêmes ordres de
grandeur ; /en/ = accueil (perf. 73 ordinateur, CLS 0,764) ; /account/ CLS 0,211 en ordinateur ;
/reviews/ accessibilité 89 ; pages de conversion 67–69 Ko et 3–6 Ko de JS.

**Causes identifiées sur l'accueil :**
- **CLS 0,71–0,76 (ordinateur)** : décalages provoqués par le mur de captures 3D (`div.wall img.wall-shot`,
  images en `loading="lazy"` sans dimensions réservées). C'est l'un des effets visuels à retirer selon le brief.
- **893 Ko, dont 801 Ko d'images PNG** : image-editor.png 272 Ko, video-editor.png 230 Ko, radial-menu.png 116 Ko…
- Accessibilité (Lighthouse, toutes pages) : `label-content-name-mismatch` (accueil, avis),
  `color-contrast` (téléchargement, avis), `aria-required-children` (formats), `heading-order` (compte, avis),
  `select-name` (avis).

### 3.2 Mise en page et clavier (copie locale du site)

Captures à 375 / 768 / 1024 / 1440 / 1920 px, sombre et clair, sur /, /download/, /formats/,
/convert/png-to-webp/, /en/ (`shots/site-local/`, `site-shots-local.json`) :
- **aucun défilement horizontal**, à aucune largeur ;
- le thème du site est **sombre par défaut et ne suit pas la préférence du système** (`data-theme="dark"`
  forcé, seul le bouton change de thème) ;
- clavier : pas de lien d'évitement (« aller au contenu ») ; le focus est visible partout, **sauf deux
  éléments sans texte ni focus visible** sur /formats/ (10ᵉ et 11ᵉ Tab).

Captures de production prises par Lighthouse (sombre, 412 et 1350 px) : `shots/site-lighthouse/`.
Elles confirment que la production a la même apparence que la copie locale.

### 3.3 Contenu et promesses (HTML de production)

| Point | Constat | Statut |
|---|---|---|
| Badge de version | « Nouveau · v1.4 » sur l'accueil FR et EN, alors que l'app est en 1.6.1 | **confirmé** |
| macOS | « macOS bientôt » (accueil), « macOS Bientôt disponible » (téléchargement), « Une version macOS est prévue » (FAQ), en FR et EN ; aucun projet de version macOS | **confirmé, trompeur** |
| « Convertis des dizaines de fichiers en parallèle » | les fichiers d'un même lot sont traités l'un après l'autre (~1 cœur mesuré) ; seuls des dépôts séparés tournent en parallèle | **confirmé, inexact** |
| « Des centaines de formats » | la même page affiche « 71 extensions reconnues » | **confirmé, contradictoire** |
| Démo « Flouter une zone » | l'éditeur d'image de l'app n'a pas d'outil de floutage de zone (Texte, Pinceau, Surligneur, Gomme, Flèche, Trait, Rectangle, Ellipse) | **confirmé** |
| Page de téléchargement anglaise | notes de version 1.6.0 en **français** (« Détourage jusqu'à 10 fois plus rapide… ») | **nouveau constat** |
| Temps de chargement des pages de conversion | 67–69 Ko, 3–6 Ko de JS, perf. 98–100 | **pas de problème** |

## 4. Synthèse : problèmes confirmés ou réfutés

**Confirmés par la mesure**
1. Éditeur audio : 7 Go de mémoire pour un WAV de 60 min, ~1,1 Go pour un MP3 de 60 min.
2. Lots traités fichier par fichier (~1 cœur sur 16 pour 50 images) : la promesse « en parallèle » est inexacte.
3. WebP sans perte, ~8 fois plus lourd que le JPG source.
4. Processeur à ~25 % d'un cœur au repos, sans fenêtre visible (cause non identifiée).
5. Mémoire non rendue après fermeture : +25 Mo après les Réglages, ~+75 Mo après l'éditeur audio.
6. Site : CLS 0,71–0,76 sur l'accueil (mur de captures 3D) ; 801 Ko d'images PNG sur l'accueil.
7. Site : badge v1.4, mentions macOS, « centaines de formats », démo « Flouter une zone », texte FR sur la page EN.

**Réfutés ou sans problème**
- Annulation : arrête bien ffmpeg et supprime le fichier partiel.
- Des dépôts séparés tournent vraiment en parallèle.
- L'interface ne ralentit pas la conversion (lot WebP : 6,4 s contre 6,1 s en moteur seul).
- Démarrage à froid rapide (0,61 s), stable d'une passe à l'autre.
- Pages de conversion du site légères et rapides ; aucun défilement horizontal.

**Observés, sans jugement** : compression vidéo 4K → 4,3 Go de mémoire pour ffmpeg ; progression
estimée (non réelle) pour pandoc, LibreOffice et le détourage d'image.

## 5. Limites et incidents

- **Une passe** pour : le repos (120 s), les Réglages ouverts / fermés, les encodages longs de V1, chaque
  manipulation de l'interface. Les écarts entre passes ne sont donnés que là où il y a 3 passes.
- **Temps d'ouverture des outils** : la mesure (apparition de la fenêtre native) ne reflète pas la
  disponibilité ; seules les captures à +1 s / +3 s l'indiquent.
- **Manipulations de l'interface** : l'instant exact des clics de l'utilisateur n'est pas connu ;
  les durées sont prises sur l'apparition / la croissance des fichiers. Lot WebP : un premier dépôt d'un seul
  fichier a précédé le lot ; seul le lot est compté.
- **Échelle 150 % abandonnée.** En passant par « Échelle personnalisée », Windows a enregistré une échelle
  de 150 % à appliquer à la prochaine connexion (`LogPixels = 144`, `Win8DpiScaling = 1`). Ces deux valeurs
  n'avaient pas été sauvegardées avant ; elles ont été remises à l'état par défaut de Windows par
  l'utilisateur (`Win8DpiScaling = 0`, `LogPixels` supprimé). `DpiValue` de l'écran est resté à 100 % tout du long.
- **Restauration vérifiée** : `settings.json` de Morph identique octet pour octet à la sauvegarde
  (SHA-256 `44d0b2f4…e1dd`), thème sombre ; échelle à 100 %.
- **Site — protection de l'hébergeur.** Le navigateur de captures automatisé (« HeadlessChrome ») a reçu
  HTTP 429 (o2switch Tiger Protect), puis des pages sans feuille de style. Ces captures ont été supprimées,
  et la protection n'a pas été contournée. Les captures à toutes les largeurs viennent donc de la **copie
  locale** du site (`dist` du 22 octobre : même apparence, mais version et notes 1.6.x absentes).
  Lighthouse n'a jamais été bloqué.
- **Site — passe non prévue.** Après le premier arrêt, une passe Lighthouse complète sur les 10 pages a
  continué à pleine vitesse (~57 chargements, 23:05–23:09) avant d'être arrêtée. Toutes les réponses
  étaient en HTTP 200. Sa première passe est conservée à part (`site-lighthouse-pass1-10pages.json`).
- Lighthouse mobile = simulation (Moto G, réseau 4G lent) : les temps absolus dépendent du modèle
  de simulation ; ils servent à comparer avant / après, pas à décrire un appareil réel.
- Les rapports Lighthouse complets (~0,5 Mo chacun) ne sont pas archivés, seulement leurs résumés.

## 6. Contenu du dossier

```
scripts/   gen_media.py (fichiers de test), bench.py (échantillonnage, fenêtres, captures),
           phases.py (phases automatiques), manual.py (manipulations de l'utilisateur),
           captures.py (captures d'interface), site.mjs (Lighthouse, captures, contenu),
           serve_dist.mjs (sert la copie locale du site)
results/   *.json (résumés), csv/ (relevés toutes les 0,25 s), shots/ (captures)
```

Les journaux du moteur (`csv/*.log`, simple affichage de la progression) restent en local : le
`.gitignore` du projet exclut les `*.log`. Les codes de sortie sont dans `cli.json`. Pour limiter le
poids, seules les captures de la 1ʳᵉ passe sont gardées pour les Réglages, les outils et l'éditeur audio.

Commandes : voir l'en-tête de chaque script. Les fichiers de test se régénèrent avec
`python -I scripts/gen_media.py <dossier>`.
