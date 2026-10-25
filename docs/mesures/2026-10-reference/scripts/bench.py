"""Outils de mesure de Morph (Windows) : échantillonnage mémoire/processeur, chronométrage
de commandes et d'ouverture de fenêtres, captures d'écran.

    python -I bench.py sample  --out x.csv (--seconds N | --stop-file F) [--roots morph.exe,morph-cli.exe]
    python -I bench.py run     --out x.csv [--roots morph-cli.exe] -- commande args…
    python -I bench.py window  --out x.json -- commande args…   (temps jusqu'à une nouvelle fenêtre Morph visible)
    python -I bench.py shot    --out x.png [--all]               (fenêtres Morph visibles, ou tout l'écran)
    python -I bench.py windows                                    (liste les fenêtres Morph visibles)

Arbre mesuré : chaque processus dont le nom est dans --roots, plus tous ses descendants
(WebView2, ffmpeg, pandoc…). Mémoire : « private » = octets privés validés (commit),
« ws » = jeu de travail. Processeur : % d'un cœur (100 = un cœur plein).
Ne modifie rien : lecture des processus et de l'écran seulement.
"""

import argparse
import csv
import ctypes
import ctypes.wintypes as wt
import json
import os
import subprocess
import sys
import threading
import time
from pathlib import Path

import psutil

user32 = ctypes.windll.user32
try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)  # coordonnées physiques, quelle que soit l'échelle
except OSError:
    pass

DEFAULT_ROOTS = ("morph.exe", "morph-cli.exe")


# --------------------------------------------------------------------------- processus

def tree(roots):
    procs = {}
    for p in psutil.process_iter(["name"]):
        if (p.info["name"] or "").lower() in roots:
            procs[p.pid] = p
            try:
                for c in p.children(recursive=True):
                    procs[c.pid] = c
            except psutil.Error:
                pass
    return procs


def group(name):
    n = name.lower()
    if n.startswith("msedgewebview2"):
        return "webview"
    if n in ("ffmpeg.exe", "pandoc.exe", "soffice.exe", "soffice.bin"):
        return "worker"
    return "morph"


class Sampler:
    """Relève l'arbre toutes les `interval` secondes dans un fil à part."""

    FIELDS = ["t", "procs", "private_mb", "ws_mb", "cpu_pct", "morph_private_mb",
              "webview_private_mb", "worker_private_mb", "workers"]

    def __init__(self, out, roots, interval=0.25):
        self.out, self.roots, self.interval = Path(out), roots, interval
        self.rows, self._stop = [], threading.Event()
        self._cpu = {}
        self._thread = threading.Thread(target=self._loop, daemon=True)

    def start(self):
        self.t0 = time.perf_counter()
        self._thread.start()
        return self

    def stop(self):
        self._stop.set()
        self._thread.join()
        self._write()
        return self.summary()

    def _loop(self):
        last = time.perf_counter()
        while not self._stop.is_set():
            now = time.perf_counter()
            row = {"t": round(now - self.t0, 3), "procs": 0, "private_mb": 0.0, "ws_mb": 0.0,
                   "cpu_pct": 0.0, "morph_private_mb": 0.0, "webview_private_mb": 0.0,
                   "worker_private_mb": 0.0, "workers": 0}
            dt = max(now - last, 1e-3)
            seen = {}
            for pid, p in tree(self.roots).items():
                try:
                    with p.oneshot():
                        mem = p.memory_info()
                        ct = p.cpu_times()
                        name = p.name()
                except psutil.Error:
                    continue
                cpu = ct.user + ct.system
                seen[pid] = cpu
                prev = self._cpu.get(pid)
                if prev is not None:
                    row["cpu_pct"] += (cpu - prev) / dt * 100
                g = group(name)
                priv = mem.private / 2**20
                row["procs"] += 1
                row["private_mb"] += priv
                row["ws_mb"] += mem.rss / 2**20
                row[f"{g}_private_mb"] += priv
                if g == "worker":
                    row["workers"] += 1
            self._cpu, last = seen, now
            for k in ("private_mb", "ws_mb", "cpu_pct", "morph_private_mb", "webview_private_mb", "worker_private_mb"):
                row[k] = round(row[k], 1)
            self.rows.append(row)
            self._stop.wait(max(0.0, self.interval - (time.perf_counter() - now)))

    def _write(self):
        self.out.parent.mkdir(parents=True, exist_ok=True)
        with self.out.open("w", newline="") as fh:
            w = csv.DictWriter(fh, fieldnames=self.FIELDS)
            w.writeheader()
            w.writerows(self.rows)

    def summary(self):
        r = self.rows[1:] or self.rows  # le premier relevé n'a pas de processeur
        if not r:
            return {}
        n = len(r)
        return {
            "samples": n,
            "duration_s": round(r[-1]["t"] - r[0]["t"], 2),
            "private_mb_peak": max(x["private_mb"] for x in r),
            "private_mb_mean": round(sum(x["private_mb"] for x in r) / n, 1),
            "private_mb_last": r[-1]["private_mb"],
            "ws_mb_peak": max(x["ws_mb"] for x in r),
            "webview_private_mb_peak": max(x["webview_private_mb"] for x in r),
            "morph_private_mb_peak": max(x["morph_private_mb"] for x in r),
            "worker_private_mb_peak": max(x["worker_private_mb"] for x in r),
            "cpu_pct_mean": round(sum(x["cpu_pct"] for x in r) / n, 1),
            "cpu_pct_peak": round(max(x["cpu_pct"] for x in r), 1),
            "workers_peak": max(x["workers"] for x in r),
            "procs_peak": max(x["procs"] for x in r),
        }


def write_summary(out, data):
    p = Path(out).with_suffix(".json")
    p.write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(data, ensure_ascii=False))


# --------------------------------------------------------------------------- fenêtres

def morph_pids():
    return {p.pid for p in psutil.process_iter(["name"]) if (p.info["name"] or "").lower() == "morph.exe"}


def visible_windows(pids=None, min_size=100):
    pids = morph_pids() if pids is None else pids
    found = []

    @ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
    def cb(hwnd, _):
        if not user32.IsWindowVisible(hwnd):
            return True
        pid = wt.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        if pid.value in pids:
            # Cadre visible (sans la bordure invisible de redimensionnement, qui laisserait
            # passer ce qu'il y a derrière dans les captures).
            r = wt.RECT()
            if ctypes.windll.dwmapi.DwmGetWindowAttribute(hwnd, 9, ctypes.byref(r), ctypes.sizeof(r)) != 0:
                user32.GetWindowRect(hwnd, ctypes.byref(r))
            n = user32.GetWindowTextLengthW(hwnd)
            buf = ctypes.create_unicode_buffer(n + 1)
            user32.GetWindowTextW(hwnd, buf, n + 1)
            cls = ctypes.create_unicode_buffer(256)
            user32.GetClassNameW(hwnd, cls, 256)
            w, h = r.right - r.left, r.bottom - r.top
            # Seulement les vraies fenêtres de l'interface : Morph garde aussi de petites
            # fenêtres techniques « visibles » (16×16, zone de notification, boucle d'événements).
            if cls.value == "Tauri Window" and w >= min_size and h >= min_size:
                found.append({"hwnd": hwnd, "pid": pid.value, "title": buf.value, "class": cls.value,
                              "rect": [r.left, r.top, r.right, r.bottom]})
        return True

    user32.EnumWindows(cb, 0)
    return found


def launch_detached(cmd):
    """Lance sans attendre (Morph relaie à l'instance déjà ouverte puis se ferme)."""
    flags = subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
    subprocess.Popen(cmd, creationflags=flags, close_fds=True,
                     stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


# --------------------------------------------------------------------------- commandes

def cmd_sample(a):
    s = Sampler(a.out, a.roots, a.interval).start()
    if a.seconds:
        time.sleep(a.seconds)
    else:
        while not Path(a.stop_file).exists():
            time.sleep(0.2)
    write_summary(a.out, {"label": Path(a.out).stem, **s.stop()})


def cmd_run(a):
    s = Sampler(a.out, a.roots, a.interval).start()
    t = time.perf_counter()
    log = Path(a.out).with_suffix(".log")
    with log.open("w", encoding="utf-8", errors="replace") as fh:
        code = subprocess.run(a.cmd, stdout=fh, stderr=subprocess.STDOUT).returncode
    wall = time.perf_counter() - t
    time.sleep(0.5)
    write_summary(a.out, {"label": Path(a.out).stem, "cmd": a.cmd, "exit": code, "wall_s": round(wall, 2), **s.stop()})


def cmd_window(a):
    before = {w["hwnd"] for w in visible_windows()}
    t = time.perf_counter()
    launch_detached(a.cmd)
    new, deadline = None, t + a.timeout
    while time.perf_counter() < deadline:
        cur = [w for w in visible_windows() if w["hwnd"] not in before]
        if cur:
            new = cur
            break
        time.sleep(0.02)
    elapsed = time.perf_counter() - t
    data = {"label": Path(a.out).stem, "cmd": a.cmd, "visible_s": round(elapsed, 3) if new else None,
            "windows": [{k: v for k, v in w.items() if k != "hwnd"} for w in (new or [])]}
    if new and a.shot_after:
        time.sleep(a.shot_after)
        shot(Path(a.out).with_suffix(".png"), new)
    Path(a.out).write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(data, ensure_ascii=False))


def shot(out, wins=None):
    from PIL import ImageGrab
    if wins is not None and not wins:
        print(f"{out} : aucune fenêtre à capturer")
        return
    if wins is None:
        img = ImageGrab.grab(all_screens=True)
    else:
        l = min(w["rect"][0] for w in wins); t = min(w["rect"][1] for w in wins)
        r = max(w["rect"][2] for w in wins); b = max(w["rect"][3] for w in wins)
        img = ImageGrab.grab(bbox=(l, t, r, b), all_screens=True)
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    img.save(out, optimize=True)
    print(f"{out} {img.size}")


def cmd_shot(a):
    shot(a.out, None if a.all else [w for w in visible_windows() if not a.title or a.title in w["title"]])


def cmd_windows(_):
    for w in visible_windows():
        print(w)


def main():
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="c", required=True)
    roots = lambda s: tuple(x.strip().lower() for x in s.split(","))
    for name in ("sample", "run"):
        p = sub.add_parser(name)
        p.add_argument("--out", required=True)
        p.add_argument("--roots", type=roots, default=DEFAULT_ROOTS)
        p.add_argument("--interval", type=float, default=0.25)
        if name == "sample":
            p.add_argument("--seconds", type=float)
            p.add_argument("--stop-file")
        else:
            p.add_argument("cmd", nargs=argparse.REMAINDER)
    p = sub.add_parser("window")
    p.add_argument("--out", required=True)
    p.add_argument("--timeout", type=float, default=30)
    p.add_argument("--shot-after", type=float, default=0)
    p.add_argument("cmd", nargs=argparse.REMAINDER)
    p = sub.add_parser("shot")
    p.add_argument("--out", required=True)
    p.add_argument("--all", action="store_true")
    p.add_argument("--title")
    sub.add_parser("windows")
    a = ap.parse_args()
    if hasattr(a, "cmd") and a.cmd and a.cmd[0] == "--":
        a.cmd = a.cmd[1:]
    {"sample": cmd_sample, "run": cmd_run, "window": cmd_window, "shot": cmd_shot, "windows": cmd_windows}[a.c](a)


if __name__ == "__main__":
    main()
