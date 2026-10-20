// Passages gardés d'un fichier audio : triés, sans chevauchement, en secondes.

export interface Segment {
  start: number;
  end: number;
}

/** Plus court passage gardé ; une coupe plus près d'un bord est ignorée. */
export const MIN_SEG = 0.05;

export const totalLength = (segs: Segment[]) => segs.reduce((n, s) => n + s.end - s.start, 0);

/** Index du passage qui contient `t`, ou -1 (t dans un trou). */
export const segmentAt = (segs: Segment[], t: number) => segs.findIndex((s) => t >= s.start && t < s.end);

/** Retire l'intervalle [a, b] des passages gardés (garder A + C, enlever B). */
export function subtract(segs: Segment[], a: number, b: number): Segment[] {
  const out: Segment[] = [];
  for (const s of segs) {
    if (b <= s.start || a >= s.end) {
      out.push(s);
      continue;
    }
    if (a - s.start >= MIN_SEG) out.push({ start: s.start, end: a });
    if (s.end - b >= MIN_SEG) out.push({ start: b, end: s.end });
  }
  return out;
}

/** Ne garde que l'intervalle [a, b]. */
export function intersect(segs: Segment[], a: number, b: number): Segment[] {
  return segs
    .map((s) => ({ start: Math.max(s.start, a), end: Math.min(s.end, b) }))
    .filter((s) => s.end - s.start >= MIN_SEG);
}

/** Coupe le passage qui contient `t` en deux (chacun peut ensuite être retiré). */
export function split(segs: Segment[], t: number): Segment[] {
  const i = segmentAt(segs, t);
  if (i < 0) return segs;
  const s = segs[i];
  if (t - s.start < MIN_SEG || s.end - t < MIN_SEG) return segs;
  return [...segs.slice(0, i), { start: s.start, end: t }, { start: t, end: s.end }, ...segs.slice(i + 1)];
}

/** Déplace un bord d'un passage, sans dépasser ses voisins ni le fichier. */
export function moveEdge(segs: Segment[], i: number, edge: "start" | "end", t: number, duration: number): Segment[] {
  const s = segs[i];
  const next =
    edge === "start"
      ? { ...s, start: clamp(t, i > 0 ? segs[i - 1].end : 0, s.end - MIN_SEG) }
      : { ...s, end: clamp(t, s.start + MIN_SEG, i < segs.length - 1 ? segs[i + 1].start : duration) };
  return segs.map((x, j) => (j === i ? next : x));
}

export const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v));

/** `1:05,25` (minutes:secondes,centièmes), `1:02:03,40` au-delà d'une heure. */
export function formatTime(t: number, precise = true): string {
  const cs = Math.round(Math.max(0, t) * 100);
  const h = Math.floor(cs / 360000);
  const m = Math.floor(cs / 6000) % 60;
  const s = Math.floor(cs / 100) % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  const base = h ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
  return precise ? `${base},${pad(cs % 100)}` : base;
}
