"""Mesure d'une action manuelle (faite par l'utilisateur dans Morph) : mémoire/processeur de
l'arbre morph.exe + apparition et croissance des fichiers de sortie dans le dossier media.

    python -I manual.py <nom> --media <dossier_media> --results <dossier_résultats> --stop-file F

S'arrête quand le fichier F existe. Écrit <résultats>/manual-<nom>.json et csv/manual-<nom>.csv.
Ne supprime rien : le nettoyage des sorties se fait ensuite, après vérification.
"""

import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import bench  # noqa: E402


def files(media: Path):
    out = {}
    for p in media.rglob("*"):
        try:
            if p.is_file():
                out[p] = p.stat().st_size
        except OSError:
            pass
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("name")
    ap.add_argument("--media", type=Path, required=True)
    ap.add_argument("--results", type=Path, required=True)
    ap.add_argument("--stop-file", type=Path, required=True)
    a = ap.parse_args()

    before = files(a.media)
    s = bench.Sampler(a.results / "csv" / f"manual-{a.name}.csv", ("morph.exe",)).start()
    t0 = time.perf_counter()
    seen = {}  # chemin → {first_s, last_change_s, size, gone_s}
    while not a.stop_file.exists():
        now = round(time.perf_counter() - t0, 2)
        cur = files(a.media)
        for p, size in cur.items():
            if p in before:
                continue
            e = seen.setdefault(p, {"first_s": now, "last_change_s": now, "size": size, "gone_s": None})
            if size != e["size"]:
                e["size"], e["last_change_s"] = size, now
            e["gone_s"] = None
        for p, e in seen.items():
            if p not in cur and e["gone_s"] is None:
                e["gone_s"] = now
        time.sleep(0.1)
    mem = s.stop()
    out = {"label": a.name, "duration_s": round(time.perf_counter() - t0, 2), "memory": mem,
           "outputs": {str(p.relative_to(a.media)): e for p, e in sorted(seen.items())}}
    (a.results / f"manual-{a.name}.json").write_text(json.dumps(out, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(out, ensure_ascii=False)[:1500])


if __name__ == "__main__":
    main()
