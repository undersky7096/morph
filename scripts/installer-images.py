"""Images de l'installeur Windows (NSIS), générées depuis l'icône de l'application.

À relancer après `npx tauri icon <logo.png>` :  python scripts/installer-images.py
(nécessite Pillow : pip install pillow)
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
ICON = ROOT / "src-tauri" / "icons" / "icon.png"
OUT = ROOT / "src-tauri" / "installer"

BG_TOP = (20, 22, 34)
BG_BOTTOM = (7, 8, 13)
MINT = (94, 242, 194)
BLUE = (47, 107, 255)


def font(size: int) -> ImageFont.ImageFont:
    for name in ("segoeuib.ttf", "seguisb.ttf", "arialbd.ttf", "DejaVuSans-Bold.ttf"):
        for folder in (Path("C:/Windows/Fonts"), Path("/usr/share/fonts/truetype/dejavu")):
            if (folder / name).exists():
                return ImageFont.truetype(str(folder / name), size)
    return ImageFont.load_default()


def background(w: int, h: int) -> Image.Image:
    """Fond sombre en dégradé vertical, avec deux halos flous (menthe et bleu)."""
    img = Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        t = y / max(1, h - 1)
        c = tuple(round(a + (b - a) * t) for a, b in zip(BG_TOP, BG_BOTTOM))
        for x in range(w):
            px[x, y] = c
    glow = Image.new("RGB", (w, h), (0, 0, 0))
    d = ImageDraw.Draw(glow)
    r = max(w, h) // 3
    d.ellipse((-r // 2, h // 4 - r // 2, r, h // 4 + r), fill=tuple(c // 3 for c in MINT))
    d.ellipse((w - r, h - r * 3 // 2, w + r // 2, h - r // 4), fill=tuple(c // 3 for c in BLUE))
    glow = glow.filter(ImageFilter.GaussianBlur(r // 2))
    return Image.blend(img, glow, 0.5)


def paste_icon(img: Image.Image, size: int, center: tuple[int, int]) -> None:
    icon = Image.open(ICON).convert("RGBA").resize((size, size), Image.LANCZOS)
    img.paste(icon, (center[0] - size // 2, center[1] - size // 2), icon)


def sidebar() -> None:
    """164 × 314 : pages d'accueil et de fin de l'installeur."""
    w, h = 164, 314
    img = background(w, h)
    paste_icon(img, 96, (w // 2, 118))
    d = ImageDraw.Draw(img)
    f = font(26)
    text = "Morph"
    tw = d.textlength(text, font=f)
    d.text(((w - tw) / 2, 182), text, font=f, fill=(238, 241, 247))
    small = font(11)
    sub = "convertir · retoucher"
    sw = d.textlength(sub, font=small)
    d.text(((w - sw) / 2, 218), sub, font=small, fill=(154, 163, 181))
    img.save(OUT / "sidebar.bmp")


def header() -> None:
    """150 × 57 : en-tête des pages intermédiaires."""
    w, h = 150, 57
    img = background(w, h)
    paste_icon(img, 40, (w - 30, h // 2))
    d = ImageDraw.Draw(img)
    d.text((12, 15), "Morph", font=font(20), fill=(238, 241, 247))
    img.save(OUT / "header.bmp")


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    sidebar()
    header()
    print(f"Images écrites dans {OUT}")
