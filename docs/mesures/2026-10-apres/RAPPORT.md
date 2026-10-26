# Mesures après refonte — 26 octobre 2026

Comparaison avec la référence du 25 octobre (`../2026-10-reference/RAPPORT.md`, Morph 1.6.1 installée).
Mêmes scripts (`scripts/verif.py` réutilise `../2026-10-reference/scripts/phases.py` et `bench.py`),
même machine, mêmes fichiers de test synthétiques (sous-ensemble régénéré par `scripts/gen_media_light.py`).

## Conditions

| | |
|---|---|
| Machine | Ryzen 7 7435HS, 16 Go, RTX 3050 Laptop, Windows 11 build 26300, 100 %, secteur |
| Version mesurée | compilation locale de `main` (commit `9c9a7cf`), `src-tauri/target/release/morph.exe`, **non installée** (numéro encore 1.6.1) |
| Témoin | Morph 1.6.1 installée (`%LOCALAPPDATA%\Morph`) |
| PC | inactif pendant les mesures (aucune saisie depuis plus de 30 min) |
| Réglages | ceux de l'utilisateur (sombre, FR, détourage « best ») ; thème passé en clair le temps d'une capture, fichier restauré à l'identique ensuite (SHA-256 `44d0b2f4…` avant et après) |

## Résultats

### Application

| Mesure | 1.6.1 (référence) | Après | Passes |
|---|---|---|---|
| Processeur au repos, aucune fenêtre (120 s) | **25,7 %** d'un cœur | **0,3 %** d'un cœur | 1 × 120 s chacune |
| Mémoire au repos (moyenne) | 342 Mo | 178 Mo | 1 × 120 s ; voir limite 1 |
| Éditeur audio, WAV 60 min — pic | **7 059 Mo** (écart 9,3 %) | **309 Mo** (écart 7,6 %) | 3 |
| Éditeur audio, MP3 60 min — pic | 1 114 Mo (0,3 %) | 314 Mo (2,5 %) | 3 |
| Éditeur audio, WMA 20 min — pic | 1 911 Mo (23 %) | 333 Mo (0,2 %) | 3 |
| Éditeur audio, encore ouvert à 60 s (WAV) | ~1 072 Mo | ~282 Mo | 3 |
| Fenêtre des Réglages visible (instance chaude) | 0,31 s | 0,26 s | 1 (captures) |
| Fenêtres d'outils visibles (5 outils) | 0,03–0,06 s | 0,03–0,04 s | 3 ; même limite que la référence (visible ≠ prêt) |

La forme d'onde complète des 60 min est bien affichée à 60 s (`results/shots/audio-A1.wav-*.png`) : la baisse
de mémoire ne vient pas d'un chargement interrompu. À 3 s, le MP3 de 60 min affiche encore « Analyse de la
forme d'onde… », comme en 1.6.1.

### Moteur (morph-cli)

| Cas | 1.6.1 | Après | Passes |
|---|---|---|---|
| 24 images (12 PNG 2000×2000, 12 JPEG 3000×2000) → BMP | 1,65 s | **0,44 s** | 2 chacune (1,67/1,62 ; 0,46/0,41) |
| Photo JPEG 4000×3000 de 2,87 Mo → WebP | 22,2 Mo (sans perte) | **0,58 Mo** (avec perte, qualité 82) | 1 |

Deux fichiers qui donnent le même nom (`same.png` et `same.jpg` → BMP) convertis en même temps : `same.bmp`
et `same (2).bmp`, aucune écriture concurrente.

### Site (copie locale construite, `dist`, servie sans compression)

| Page | Référence (production) | Après (local) |
|---|---|---|
| Accueil, décalage de mise en page (CLS) | 0,71 | **0** |
| /account/, CLS | 0,21 | **0** |
| Accueil, ressources chargées | 22 requêtes, 893 Ko | 12 requêtes, 154 Ko (sans la démo chargée à la demande) |

Accessibilité : les causes relevées par Lighthouse en référence sont corrigées dans le code (filtres de
/formats/ en boutons pressés au lieu d'un `tablist` incomplet, nom des votes égal au texte visible, liste de
tri nommée, titres du pied de page en `h2`). Lighthouse n'a **pas** été relancé (voir limites).

## Limites

1. La mémoire au repos de la référence était prise après une session d'utilisation ; ici, 60 s après un
   démarrage. La comparaison exacte (même historique) reste à faire ; la baisse du processeur, elle, ne
   dépend pas de l'historique (cause identifiée : le halo dessiné en boucle alors qu'il était masqué).
2. Version mesurée **compilée localement, non installée** : sans ONNX Runtime ni pdfium à côté de
   l'exécutable, le détourage et le PDF n'ont pas été exercés ici (non modifiés par la refonte).
3. Les lots d'images en parallèle sont mesurés via morph-cli ; dans l'application (Maj + glisser d'un lot), la
   même fonction (`convert::parallelism`) est utilisée, mais le glisser-déposer n'a pas été rejoué (pas de
   clic simulé).
4. Site : mesures locales sans compression HTTP ni hébergeur, pas de Lighthouse (outil non réinstallé).
   À refaire sur la production après déploiement, comme en référence.
5. Liaison au compte : l'API `app.php` n'est pas déployée et aucun PHP n'est disponible sur cette machine ;
   seuls le chiffrement du jeton (test DPAPI), la sélection des préférences synchronisées et la construction
   des pages ont été testés. Le parcours complet (code → approbation → jeton → synchronisation) reste à
   vérifier après la mise en ligne (DEPLOIEMENT.md §11 du site).
6. Les captures du menu radial réel ne sont pas archivées : la fenêtre est transparente et montrait le
   bureau de l'utilisateur derrière.
