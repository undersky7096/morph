"""Mesures « après refonte » : mêmes protocoles que la référence (../2026-10-reference),
appliqués à une version compilée localement (non installée).

    python -I verif.py <phase> --morph <chemin\\morph.exe> --media <dossier> --results <dossier>

Phases : idle, tools, audio (identiques à la référence) et shots (Réglages visibles,
pour les captures du thème courant).

Aucun clic ni frappe simulés : fenêtres ouvertes par la ligne de commande de Morph,
fermées par WM_CLOSE (comme le bouton ×).
"""

import argparse
import sys
import time
from pathlib import Path

REF = Path(__file__).resolve().parents[2] / "2026-10-reference" / "scripts"
sys.path.insert(0, str(REF))
import bench  # noqa: E402
import phases  # noqa: E402


def shots(a):
    name = a.label or "settings"
    secs, wins = phases.open_window([phases.MORPH])
    time.sleep(2)
    bench.shot(a.results / "shots" / f"{name}.png", wins)
    phases.close(wins)
    phases.save(a.results, f"shots-{name}", {"visible_s": secs})


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("phase", choices=["idle", "tools", "audio", "shots"])
    ap.add_argument("--morph", type=Path, required=True)
    ap.add_argument("--media", type=Path, required=True)
    ap.add_argument("--results", type=Path, required=True)
    ap.add_argument("--label")
    a = ap.parse_args()
    phases.MORPH = str(a.morph)
    (a.results / "csv").mkdir(parents=True, exist_ok=True)
    (a.results / "shots").mkdir(parents=True, exist_ok=True)
    {"idle": phases.idle, "tools": phases.tools, "audio": phases.audio, "shots": shots}[a.phase](a)


if __name__ == "__main__":
    main()
