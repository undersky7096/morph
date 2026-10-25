"""Génère les fichiers de test des mesures de référence (synthétiques, reproductibles).

    python -I gen_media.py <dossier_media> [--ffmpeg chemin\\ffmpeg.exe]

Utilise le ffmpeg livré avec Morph (par défaut %LOCALAPPDATA%\\Morph\\ffmpeg.exe).
Les fichiers existants sont conservés (relancer reprend là où on s'était arrêté).
"""

import argparse
import os
import subprocess
import sys
import time
from pathlib import Path

DEFAULT_FFMPEG = Path(os.environ.get("LOCALAPPDATA", "")) / "Morph" / "ffmpeg.exe"


def ff(ffmpeg: Path, *args: str) -> None:
    cmd = [str(ffmpeg), "-hide_banner", "-loglevel", "error", "-y", *args]
    print(" ".join(cmd), flush=True)
    subprocess.run(cmd, check=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("media", type=Path)
    ap.add_argument("--ffmpeg", type=Path, default=DEFAULT_FFMPEG)
    a = ap.parse_args()
    m: Path = a.media
    m.mkdir(parents=True, exist_ok=True)
    f = a.ffmpeg

    jobs = []

    # A1 : WAV 60 min, stéréo 48 kHz 16 bits (sinus + bruit rose, contenu non nul partout).
    jobs.append(("A1.wav", lambda o: ff(f,
        "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000:duration=3600",
        "-f", "lavfi", "-i", "anoisesrc=color=pink:sample_rate=48000:amplitude=0.15:duration=3600:seed=1",
        "-filter_complex", "[0][1]amix=inputs=2:normalize=0,aformat=channel_layouts=stereo",
        "-c:a", "pcm_s16le", o)))
    # A2 : MP3 60 min 192 kb/s (dérivé de A1).
    jobs.append(("A2.mp3", lambda o: ff(f, "-i", str(m / "A1.wav"), "-c:a", "libmp3lame", "-b:a", "192k", o)))
    # A3 : WMA 20 min (chemin « aperçu réencodé » de l'éditeur audio).
    jobs.append(("A3.wma", lambda o: ff(f, "-i", str(m / "A1.wav"), "-t", "1200", "-c:a", "wmav2", "-b:a", "128k", o)))
    # V1 : MP4 H.264 1080p30, 20 min, mire animée + bruit temporel (entropie réaliste) + son.
    jobs.append(("V1.mp4", lambda o: ff(f,
        "-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=30:duration=1200",
        "-f", "lavfi", "-i", "sine=frequency=330:sample_rate=48000:duration=1200",
        "-vf", "noise=alls=10:allf=t", "-c:v", "libx264", "-preset", "veryfast", "-b:v", "10M",
        "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k", "-shortest", o)))
    # V2 : MP4 H.264 4K30, 3 min.
    jobs.append(("V2.mp4", lambda o: ff(f,
        "-f", "lavfi", "-i", "testsrc2=size=3840x2160:rate=30:duration=180",
        "-f", "lavfi", "-i", "sine=frequency=330:sample_rate=48000:duration=180",
        "-vf", "noise=alls=10:allf=t", "-c:v", "libx264", "-preset", "veryfast", "-b:v", "30M",
        "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k", "-shortest", o)))
    # V3 : MP4 1080p 15 s (observation de la progression du détourage vidéo).
    jobs.append(("V3.mp4", lambda o: ff(f, "-i", str(m / "V1.mp4"), "-t", "15", "-c", "copy", o)))

    for name, make in jobs:
        out = m / name
        if out.exists() and out.stat().st_size > 0:
            print(f"déjà là : {name}")
            continue
        t = time.perf_counter()
        make(str(out))
        print(f"{name} : {out.stat().st_size / 1e6:.1f} Mo en {time.perf_counter() - t:.0f} s", flush=True)

    # I1 : 30 JPEG 4000×3000 (dégradés + bruit, proches d'une photo) + 20 PNG 2000×2000.
    lot = m / "I1"
    lot.mkdir(exist_ok=True)
    if len(list(lot.glob("*.jpg"))) < 30:
        ff(f, "-f", "lavfi", "-i", "gradients=size=4000x3000:nb_colors=5:speed=0.08:seed=7:rate=1",
           "-vf", "noise=alls=14:allf=u", "-frames:v", "30", "-q:v", "3", str(lot / "photo_%02d.jpg"))
    if len(list(lot.glob("*.png"))) < 20:
        ff(f, "-f", "lavfi", "-i", "testsrc2=size=2000x2000:rate=1",
           "-vf", "noise=alls=6:allf=u", "-frames:v", "20", str(lot / "capture_%02d.png"))

    total = sum(p.stat().st_size for p in m.rglob("*") if p.is_file())
    print(f"total : {total / 1e9:.2f} Go")
    return 0


if __name__ == "__main__":
    sys.exit(main())
