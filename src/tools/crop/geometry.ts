/** Géométrie du recadrage, en pixels de l'image source. */

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Size {
  w: number;
  h: number;
}

/** Poignée saisie : déplacement du cadre ou redimensionnement par un bord / un coin. */
export type Handle = "move" | "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

export const MIN_SIZE = 8;

const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);

/** Plus grand cadre de ratio `ratio` centré dans l'image (ou l'image entière si libre). */
export function fitRatio(img: Size, ratio: number | null): Rect {
  if (!ratio) return { x: 0, y: 0, w: img.w, h: img.h };
  let w = img.w;
  let h = w / ratio;
  if (h > img.h) {
    h = img.h;
    w = h * ratio;
  }
  return { x: (img.w - w) / 2, y: (img.h - h) / 2, w, h };
}

/** Nouveau cadre après un glisser de (dx, dy) depuis `start` avec la poignée `handle`. */
export function dragRect(start: Rect, handle: Handle, dx: number, dy: number, img: Size, ratio: number | null): Rect {
  if (handle === "move") {
    return {
      ...start,
      x: clamp(start.x + dx, 0, img.w - start.w),
      y: clamp(start.y + dy, 0, img.h - start.h),
    };
  }
  if (ratio && handle.length === 2) return dragCornerWithRatio(start, handle, dx, dy, img, ratio);

  let left = start.x;
  let top = start.y;
  let right = start.x + start.w;
  let bottom = start.y + start.h;
  if (handle.includes("w")) left = clamp(left + dx, 0, right - MIN_SIZE);
  if (handle.includes("e")) right = clamp(right + dx, left + MIN_SIZE, img.w);
  if (handle.includes("n")) top = clamp(top + dy, 0, bottom - MIN_SIZE);
  if (handle.includes("s")) bottom = clamp(bottom + dy, top + MIN_SIZE, img.h);
  return { x: left, y: top, w: right - left, h: bottom - top };
}

/** Coin déplacé à ratio fixe : le coin opposé reste ancré. */
function dragCornerWithRatio(start: Rect, handle: Handle, dx: number, dy: number, img: Size, ratio: number): Rect {
  const east = handle.includes("e");
  const south = handle.includes("s");
  const anchorX = east ? start.x : start.x + start.w;
  const anchorY = south ? start.y : start.y + start.h;
  const cornerX = (east ? start.x + start.w : start.x) + dx;
  const cornerY = (south ? start.y + start.h : start.y) + dy;

  let w = Math.max(Math.abs(cornerX - anchorX), Math.abs(cornerY - anchorY) * ratio, MIN_SIZE);
  let h = w / ratio;
  const maxW = east ? img.w - anchorX : anchorX;
  const maxH = south ? img.h - anchorY : anchorY;
  if (w > maxW) {
    w = maxW;
    h = w / ratio;
  }
  if (h > maxH) {
    h = maxH;
    w = h * ratio;
  }
  return { x: east ? anchorX : anchorX - w, y: south ? anchorY : anchorY - h, w, h };
}

/** Change la largeur ou la hauteur (saisie numérique) en gardant le centre et le ratio. */
export function resizeTo(rect: Rect, dim: "w" | "h", value: number, img: Size, ratio: number | null): Rect {
  let w = rect.w;
  let h = rect.h;
  if (dim === "w") {
    w = clamp(value, MIN_SIZE, img.w);
    if (ratio) h = w / ratio;
  } else {
    h = clamp(value, MIN_SIZE, img.h);
    if (ratio) w = h * ratio;
  }
  if (ratio && h > img.h) {
    h = img.h;
    w = h * ratio;
  }
  if (ratio && w > img.w) {
    w = img.w;
    h = w / ratio;
  }
  const cx = rect.x + rect.w / 2;
  const cy = rect.y + rect.h / 2;
  return { x: clamp(cx - w / 2, 0, img.w - w), y: clamp(cy - h / 2, 0, img.h - h), w, h };
}

/** Cadre en pixels entiers, prêt à être envoyé au moteur. */
export function toPixels(rect: Rect, img: Size) {
  const x = clamp(Math.round(rect.x), 0, img.w - 1);
  const y = clamp(Math.round(rect.y), 0, img.h - 1);
  return {
    x,
    y,
    width: clamp(Math.round(rect.w), 1, img.w - x),
    height: clamp(Math.round(rect.h), 1, img.h - y),
  };
}
