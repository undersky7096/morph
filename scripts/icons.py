"""Icônes de Morph, générées depuis le logo (`branding/morph-logo.png`, M sombre sur
fond transparent).

    python scripts/icons.py      (nécessite Pillow : pip install pillow)

Produit :
- `src-tauri/icons/mark.png` et `src/assets/mark.png` : le M seul, blanc sur
  transparent (masque recoloré par l'application : icône dynamique, interface) ;
- `branding/app-icon.png` : M noir sur carré blanc arrondi, d'où `npx tauri icon`
  tire toutes les icônes fixes (exe, installeur, raccourcis, Linux) ;
- les images de l'installeur (`scripts/installer-images.py`).
"""

import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
LOGO = ROOT / "branding" / "morph-logo.png"

# Géométrie partagée avec `src-tauri/src/icon.rs` : carré arrondi qui occupe
# TILE de l'image, coins de RADIUS × le côté du carré, M sur MARK du côté du carré.
TILE = 0.90
RADIUS = 0.23
MARK = 0.60


def mark(size: int) -> Image.Image:
    """Le M blanc sur transparent, centré dans un carré de `size` px."""
    logo = Image.open(LOGO).convert("RGBA")
    alpha = logo.getchannel("A").crop(logo.getchannel("A").getbbox())
    side = max(alpha.size)
    square = Image.new("L", (side, side))
    square.paste(alpha, ((side - alpha.width) // 2, (side - alpha.height) // 2))
    square = square.resize((size, size), Image.LANCZOS)
    out = Image.new("RGBA", (size, size), (255, 255, 255, 0))
    out.putalpha(square)
    return out


def app_icon(size: int, tile=(255, 255, 255), ink=(24, 24, 24)) -> Image.Image:
    scale = 4  # suréchantillonnage pour des coins lisses
    big = size * scale
    t = round(big * TILE)
    off = (big - t) // 2
    mask = Image.new("L", (big, big))
    ImageDraw.Draw(mask).rounded_rectangle((off, off, off + t - 1, off + t - 1), radius=round(t * RADIUS), fill=255)
    img = Image.new("RGBA", (big, big), tile + (0,))
    img.putalpha(mask)
    img = img.resize((size, size), Image.LANCZOS)
    m = round(size * TILE * MARK)
    glyph = mark(m)
    ink_layer = Image.new("RGBA", (m, m), ink + (255,))
    ink_layer.putalpha(glyph.getchannel("A"))
    img.alpha_composite(ink_layer, ((size - m) // 2, (size - m) // 2))
    return img


def main() -> None:
    m = mark(512)
    m.save(ROOT / "src-tauri" / "icons" / "mark.png")
    m.save(ROOT / "src" / "assets" / "mark.png")
    source = ROOT / "branding" / "app-icon.png"
    app_icon(1024).save(source)
    npx = shutil.which("npx") or "npx"
    subprocess.run([npx, "tauri", "icon", str(source)], cwd=ROOT, check=True)
    for mobile in ("android", "ios"):  # applications de bureau seulement
        shutil.rmtree(ROOT / "src-tauri" / "icons" / mobile, ignore_errors=True)
    subprocess.run([sys.executable, str(ROOT / "scripts" / "installer-images.py")], cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
