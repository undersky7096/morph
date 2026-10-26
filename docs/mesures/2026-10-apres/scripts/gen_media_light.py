"""Fichiers de test des mesures « après refonte », sous-ensemble de la référence
(mêmes paramètres que ../2026-10-reference/scripts/gen_media.py) : A1 WAV 60 min, A2 MP3,
A3 WMA 20 min, V3 MP4 15 s, une photo JPEG 4000×3000.

    python -I gen_media_light.py <dossier_media> [--ffmpeg chemin\\ffmpeg.exe]
"""

import argparse
import os
import subprocess
import sys
from pathlib import Path

DEFAULT_FFMPEG = Path(os.environ.get("LOCALAPPDATA", "")) / "Morph" / "ffmpeg.exe"


def ff(ffmpeg: Path, *args: str) -> None:
    subprocess.run([str(ffmpeg), "-hide_banner", "-loglevel", "error", "-y", *args], check=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("media", type=Path)
    ap.add_argument("--ffmpeg", type=Path, default=DEFAULT_FFMPEG)
    a = ap.parse_args()
    m, f = a.media, a.ffmpeg
    (m / "I1").mkdir(parents=True, exist_ok=True)
    steps = [
        ("A1.wav", lambda o: ff(f,
            "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000:duration=3600",
            "-f", "lavfi", "-i", "anoisesrc=color=pink:sample_rate=48000:amplitude=0.15:duration=3600:seed=1",
            "-filter_complex", "[0][1]amix=inputs=2:normalize=0,aformat=channel_layouts=stereo",
            "-c:a", "pcm_s16le", o)),
        ("A2.mp3", lambda o: ff(f, "-i", str(m / "A1.wav"), "-c:a", "libmp3lame", "-b:a", "192k", o)),
        ("A3.wma", lambda o: ff(f, "-i", str(m / "A1.wav"), "-t", "1200", "-c:a", "wmav2", "-b:a", "128k", o)),
        ("V3.mp4", lambda o: ff(f,
            "-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=30:duration=15",
            "-f", "lavfi", "-i", "sine=frequency=330:sample_rate=48000:duration=15",
            "-vf", "noise=alls=10:allf=t", "-c:v", "libx264", "-preset", "veryfast", "-b:v", "10M",
            "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k", "-shortest", o)),
        ("I1/photo_01.jpg", lambda o: ff(f,
            "-f", "lavfi", "-i", "gradients=size=4000x3000:nb_colors=5:speed=0.08:seed=7:rate=1",
            "-vf", "noise=alls=14:allf=u", "-frames:v", "1", "-q:v", "3", o)),
    ]
    for name, make in steps:
        out = m / name
        if not (out.exists() and out.stat().st_size > 0):
            make(str(out))
        print(f"{name} : {out.stat().st_size / 1e6:.1f} Mo", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
