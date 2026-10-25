"""Phases automatiques des mesures de référence (sans clic ni frappe simulés).

    python -I phases.py <phase> --media <dossier_media> --results <dossier_résultats>

Phases :
  idle      mémoire au repos de l'instance déjà lancée (120 s)
  settings  ouverture des Réglages ×3 (instance chaude) + mémoire Réglages ouverts / refermés
  cold      un démarrage à froid (Morph doit être quitté avant) → Réglages visibles
  tools     ouverture des outils ×3 (Recadrer, Compresser, éditeurs image / audio / vidéo)
  audio     mémoire de l'éditeur audio sur A1 (WAV 60 min), A2 (MP3 60 min), A3 (WMA 20 min) ×3
  cli       moteur seul (morph-cli) : vidéo, audio, lot d'images, ×3 sauf encodages > 10 min

Fermeture des fenêtres ouvertes par la mesure : message WM_CLOSE (comme le bouton ×),
jamais de souris ni de clavier simulés. Les sorties créées dans le dossier media sont
supprimées après chaque passe (seulement les fichiers absents avant la passe).
"""

import argparse
import ctypes
import json
import os
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import bench  # noqa: E402

MORPH_DIR = Path(os.environ["LOCALAPPDATA"]) / "Morph"
MORPH = str(MORPH_DIR / "morph.exe")
CLI = str(MORPH_DIR / "morph-cli.exe")
WM_CLOSE = 0x0010


def close(wins):
    for w in wins:
        ctypes.windll.user32.PostMessageW(w["hwnd"], WM_CLOSE, 0, 0)


def snapshot(media: Path):
    return {p for p in media.rglob("*")}


def cleanup(media: Path, before):
    """Supprime ce qui est apparu dans `media` depuis `before` (sorties d'une passe)."""
    for p in sorted(snapshot(media) - before, key=lambda p: len(p.parts), reverse=True):
        if p.is_dir():
            shutil.rmtree(p, ignore_errors=True)
        else:
            p.unlink(missing_ok=True)


def open_window(cmd, timeout=30):
    """Lance `cmd` et attend une nouvelle fenêtre Morph visible. → (secondes, fenêtres)."""
    before = {w["hwnd"] for w in bench.visible_windows()}
    t = time.perf_counter()
    bench.launch_detached(cmd)
    while time.perf_counter() - t < timeout:
        new = [w for w in bench.visible_windows() if w["hwnd"] not in before]
        if new:
            # La plus grande : l'anneau des outils peut réapparaître en même temps.
            area = lambda w: (w["rect"][2] - w["rect"][0]) * (w["rect"][3] - w["rect"][1])
            return time.perf_counter() - t, [max(new, key=area)]
        time.sleep(0.02)
    return None, []


def stats(values):
    v = [x for x in values if x is not None]
    if not v:
        return {}
    med = statistics.median(v)
    return {"values": [round(x, 3) for x in v], "median": round(med, 3),
            "spread_pct": round((max(v) - min(v)) / med * 100, 1) if med else 0.0}


def save(results: Path, name, data):
    results.mkdir(parents=True, exist_ok=True)
    (results / f"{name}.json").write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")
    print(name, json.dumps(data, ensure_ascii=False)[:600], flush=True)


# --------------------------------------------------------------------------- phases

def idle(a):
    s = bench.Sampler(a.results / "csv" / "idle.csv", ("morph.exe",)).start()
    time.sleep(120)
    save(a.results, "idle", s.stop())


def settings(a):
    times, out = [], {}
    for i in range(3):
        secs, wins = open_window([MORPH])
        times.append(secs)
        time.sleep(1)
        bench.shot(a.results / "shots" / f"settings-open-{i + 1}.png", wins)
        if i == 0:  # mémoire Réglages ouverts
            s = bench.Sampler(a.results / "csv" / "settings-open.csv", ("morph.exe",)).start()
            time.sleep(120)
            out["memory_open"] = s.stop()
        close(wins)
        time.sleep(5)
        if i == 0:  # mémoire après fermeture (fenêtre détruite)
            s = bench.Sampler(a.results / "csv" / "settings-closed.csv", ("morph.exe",)).start()
            time.sleep(120)
            out["memory_after_close"] = s.stop()
    out["visible_s"] = stats(times)
    save(a.results, "settings", out)


def cold(a):
    if bench.morph_pids():
        sys.exit("Morph tourne encore : le quitter d'abord (icône → Quitter).")
    n = len(list((a.results).glob("cold-*.json"))) + 1
    secs, wins = open_window([MORPH], timeout=60)
    time.sleep(1)
    bench.shot(a.results / "shots" / f"cold-{n}.png", wins)
    s = bench.Sampler(a.results / "csv" / f"cold-{n}.csv", ("morph.exe",)).start()
    time.sleep(30)
    mem = s.stop()
    close(wins)
    save(a.results, f"cold-{n}", {"visible_s": secs, "memory_first_30s": mem})


TOOLS = [  # (nom, argument, fichier relatif au dossier media)
    ("crop-jpg", "--crop", "I1/photo_01.jpg"),
    ("compress-jpg", "--compress", "I1/photo_01.jpg"),
    ("edit-jpg", "--edit", "I1/photo_01.jpg"),
    ("edit-mp3", "--edit", "A2.mp3"),
    ("edit-mp4", "--edit", "V3.mp4"),
]


def tools(a):
    out = {}
    for name, flag, rel in TOOLS:
        times = []
        for i in range(3):
            secs, wins = open_window([MORPH, flag, str(a.media / rel)])
            times.append(secs)
            # « Visible » n'est pas « prêt » : captures à +1 s et +3 s pour voir le contenu.
            time.sleep(1)
            bench.shot(a.results / "shots" / f"tool-{name}-{i + 1}-1s.png", wins)
            time.sleep(2)
            bench.shot(a.results / "shots" / f"tool-{name}-{i + 1}-3s.png", wins)
            close(wins)
            time.sleep(4)  # l'anneau des outils réapparaît ; il se ferme au prochain focus
        out[name] = stats(times)
    save(a.results, "tools", out)


def audio(a):
    out = {}
    for name in ("A1.wav", "A2.mp3", "A3.wma"):
        runs = []
        for i in range(3):
            s = bench.Sampler(a.results / "csv" / f"audio-{name}-{i + 1}.csv", ("morph.exe",)).start()
            secs, wins = open_window([MORPH, "--edit", str(a.media / name)])
            time.sleep(60)  # décodage et forme d'onde (pic mémoire attendu ici)
            bench.shot(a.results / "shots" / f"audio-{name}-{i + 1}.png", wins)
            mem = s.stop()
            close(wins)
            time.sleep(10)
            runs.append({"visible_s": secs, **mem})
        out[name] = {
            "runs": runs,
            "private_mb_peak": stats([r["private_mb_peak"] for r in runs]),
            "webview_private_mb_peak": stats([r["webview_private_mb_peak"] for r in runs]),
        }
    save(a.results, "audio", out)


def cli_cases(m: Path):
    imgs = sorted(str(p) for p in (m / "I1").iterdir())
    # (nom, arguments, passes) ; 1 passe pour les encodages de plus de 10 min.
    return [
        ("v1-mkv-copy", ["mkv", str(m / "V1.mp4")], 3),
        ("v2-mkv-copy", ["mkv", str(m / "V2.mp4")], 3),
        ("v3-webm", ["webm", str(m / "V3.mp4")], 3),
        ("v2-avi", ["avi", str(m / "V2.mp4")], 3),
        ("v2-compress-medium", ["compress", "medium", str(m / "V2.mp4")], 3),
        ("v1-compress-medium", ["compress", "medium", str(m / "V1.mp4")], 1),
        ("v1-compress-target20", ["compress", "medium", "--target", "20", str(m / "V1.mp4")], 1),
        ("a1-mp3", ["mp3", str(m / "A1.wav")], 3),
        ("i1-webp", ["webp", *imgs], 3),
        ("i1-compress-medium", ["compress", "medium", *imgs], 3),
    ]


def cli(a):
    only = set(a.only.split(",")) if a.only else None
    path = a.results / "cli.json"
    out = json.loads(path.read_text(encoding="utf-8")) if path.exists() else {}
    for name, args, passes in cli_cases(a.media):
        if only and name not in only:
            continue
        runs = []
        for i in range(passes):
            before = snapshot(a.media)
            csvp = a.results / "csv" / f"cli-{name}-{i + 1}.csv"
            s = bench.Sampler(csvp, ("morph-cli.exe",)).start()
            t = time.perf_counter()
            log = csvp.with_suffix(".log")
            with log.open("w", encoding="utf-8", errors="replace") as fh:
                code = subprocess.run([CLI, *args], stdout=fh, stderr=subprocess.STDOUT).returncode
            wall = time.perf_counter() - t
            mem = s.stop()
            produced = sorted(snapshot(a.media) - before)
            sizes = {p.name: p.stat().st_size for p in produced if p.is_file()}
            runs.append({"exit": code, "wall_s": round(wall, 2), "outputs": len(sizes),
                         "output_bytes": sum(sizes.values()), **mem})
            cleanup(a.media, before)
            time.sleep(60 if wall > 60 else 10)  # refroidissement entre deux passes
        out[name] = {"args": [x if not os.path.isabs(x) else Path(x).name for x in args][:6],
                     "passes": passes, "runs": runs, "wall_s": stats([r["wall_s"] for r in runs]),
                     "private_mb_peak": stats([r["private_mb_peak"] for r in runs])}
        save(a.results, "cli", out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("phase", choices=["idle", "settings", "cold", "tools", "audio", "cli"])
    ap.add_argument("--media", type=Path, required=True)
    ap.add_argument("--results", type=Path, required=True)
    ap.add_argument("--only", help="cli : liste de cas séparés par des virgules")
    a = ap.parse_args()
    globals()[a.phase](a)


if __name__ == "__main__":
    main()
