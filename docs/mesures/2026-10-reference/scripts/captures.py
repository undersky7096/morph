"""Captures de référence de l'interface, pour une configuration donnée (thème × échelle).

    python -I captures.py auto  <tag> --media <dossier_media> --results <dossier_résultats>
    python -I captures.py watch <tag> --results <dossier_résultats> [--timeout 180]

auto  : ouvre les Réglages et les outils (Recadrer, Compresser, éditeurs image / audio / vidéo),
        capture chaque fenêtre à +3 s puis la referme (WM_CLOSE).
watch : attend les fenêtres ouvertes par un glisser-déposer de l'utilisateur (halo, anneau des formats,
        puis progression) et capture chacune 1,2 s après son apparition.
Ne change aucun réglage : thème et échelle sont changés à la main par l'utilisateur.
"""

import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import bench  # noqa: E402
import phases  # noqa: E402

SHOTS = [  # (nom, arguments après morph.exe, fichier relatif au dossier media)
    ("settings", [], None),
    ("crop", ["--crop"], "I1/photo_01.jpg"),
    ("compress", ["--compress"], "I1/photo_01.jpg"),
    ("edit-image", ["--edit"], "I1/photo_01.jpg"),
    ("edit-audio", ["--edit"], "A3.wma"),
    ("edit-video", ["--edit"], "V3.mp4"),
]


def auto(a):
    for name, args, rel in SHOTS:
        cmd = [phases.MORPH, *args] + ([str(a.media / rel)] if rel else [])
        _, wins = phases.open_window(cmd)
        if not wins:
            print(f"{name} : aucune fenêtre (déjà ouverte ?)")
            continue
        time.sleep(3)
        bench.shot(a.results / "shots" / "ui" / f"{a.tag}-{name}.png", wins)
        phases.close(wins)
        time.sleep(4)


def watch(a):
    # L'anneau est une fenêtre persistante qu'on masque et réaffiche : on guette les fenêtres
    # qui *deviennent* visibles, pas seulement les nouvelles.
    # La fenêtre de progression est basse : seuil de taille réduit à 40 px.
    pids = bench.morph_pids()
    prev = {w["hwnd"] for w in bench.visible_windows(pids, 40)}
    t0, n = time.perf_counter(), 0
    while time.perf_counter() - t0 < a.timeout and n < 4:
        cur = bench.visible_windows(pids, 40)
        for w in cur:
            if w["hwnd"] in prev or n >= 4:
                continue
            print(f"+{time.perf_counter() - t0:.1f} s : {w['rect']}", flush=True)
            time.sleep(1.2)
            now = [x for x in bench.visible_windows(pids, 40) if x["hwnd"] == w["hwnd"]]
            n += 1
            bench.shot(a.results / "shots" / "ui" / f"{a.tag}-drop-{n}.png", now)
        prev = {w["hwnd"] for w in cur}
        time.sleep(0.03)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["auto", "watch"])
    ap.add_argument("tag")
    ap.add_argument("--media", type=Path)
    ap.add_argument("--results", type=Path, required=True)
    ap.add_argument("--timeout", type=float, default=180)
    a = ap.parse_args()
    {"auto": auto, "watch": watch}[a.mode](a)


if __name__ == "__main__":
    main()
