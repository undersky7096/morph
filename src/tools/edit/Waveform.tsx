import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { clamp, formatTime, type Segment } from "./segments";

/** Pics de la forme d'onde par seconde (voir `edit::trim::PEAKS_PER_SEC`). */
export const PEAKS_PER_SEC = 100;
/** Plus petite durée affichée (zoom maximal). */
export const MIN_SPAN = 0.5;

export interface View {
  start: number;
  end: number;
}

export interface Range {
  a: number;
  b: number;
}

const RULER = 22;
const EDGE_HIT = 6;
const STEPS = [0.01, 0.05, 0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600];

interface Props {
  peaks: Uint8Array;
  duration: number;
  segments: Segment[];
  selection: Range | null;
  view: View;
  time: number;
  onViewChange: (view: View) => void;
  onSeek: (t: number) => void;
  onSelect: (range: Range | null) => void;
  /** Glisser un bord de passage ; `done` au relâchement. */
  onEdge: (index: number, edge: "start" | "end", t: number, done: boolean) => void;
}

type Drag =
  | { mode: "edge"; index: number; edge: "start" | "end" }
  | { mode: "select"; anchor: number; x: number; moved: boolean };

/** Zoom autour de `at` (facteur < 1 : rapprocher). */
export function zoomView(view: View, factor: number, at: number, duration: number): View {
  const span = clamp((view.end - view.start) * factor, Math.min(MIN_SPAN, duration), duration);
  const ratio = (at - view.start) / (view.end - view.start);
  return panView({ start: at - span * ratio, end: at - span * ratio + span }, 0, duration);
}

/** Décale la vue de `dt` secondes, sans sortir du fichier. */
export function panView(view: View, dt: number, duration: number): View {
  const span = view.end - view.start;
  const start = clamp(view.start + dt, 0, duration - span);
  return { start, end: start + span };
}

/** Couleurs du thème actif, lues sur l'élément (variables CSS). */
function palette(el: Element) {
  const css = getComputedStyle(el);
  const v = (name: string) => css.getPropertyValue(name).trim();
  return { audio: v("--kind-audio"), text: v("--text"), muted: v("--muted"), line: v("--line"), bg: v("--bg") };
}

/** Couleur CSS (#rgb, #rrggbb ou rgb[a]()) avec une opacité : le canvas 2D ne lit pas toujours `color-mix()`. */
function alpha(color: string, a: number): string {
  let rgb: number[] = [];
  if (color.startsWith("#")) {
    const hex = color.length === 4 ? [...color.slice(1)].map((d) => d + d).join("") : color.slice(1, 7);
    rgb = [0, 2, 4].map((i) => parseInt(hex.slice(i, i + 2), 16));
  } else {
    const m = color.match(/rgba?\(([^)]+)\)/);
    if (m) rgb = m[1].split(/[\s,/]+/).slice(0, 3).map(Number);
  }
  return rgb.length === 3 ? `rgba(${rgb.join(", ")}, ${a})` : color;
}

/** Plus grand pic de chaque colonne de pixels entre `start` et `end`. */
function columns(peaks: Uint8Array, start: number, end: number, width: number): Uint8Array {
  const out = new Uint8Array(width);
  const perPx = ((end - start) * PEAKS_PER_SEC) / width;
  for (let x = 0; x < width; x++) {
    const from = Math.floor(start * PEAKS_PER_SEC + x * perPx);
    const to = Math.max(from + 1, Math.floor(start * PEAKS_PER_SEC + (x + 1) * perPx));
    let max = 0;
    for (let i = Math.max(0, from); i < Math.min(to, peaks.length); i++) if (peaks[i] > max) max = peaks[i];
    out[x] = max;
  }
  return out;
}

function useSize(ref: React.RefObject<HTMLElement | null>) {
  const [size, setSize] = useState({ w: 0, h: 0 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () => setSize({ w: el.clientWidth, h: el.clientHeight });
    update();
    const observer = new ResizeObserver(update);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return size;
}

function setupCanvas(canvas: HTMLCanvasElement, w: number, h: number) {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(w * dpr);
  canvas.height = Math.round(h * dpr);
  const ctx = canvas.getContext("2d")!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  return ctx;
}

/** Forme d'onde détaillée : règle, passages gardés (couleur) et retirés (grisés), sélection, curseur de lecture. */
export default function Waveform(props: Props) {
  const { peaks, duration, segments, selection, view, time } = props;
  const wrapRef = useRef<HTMLDivElement>(null);
  const baseRef = useRef<HTMLCanvasElement>(null);
  const overlayRef = useRef<HTMLCanvasElement>(null);
  const drag = useRef<Drag | null>(null);
  const [hover, setHover] = useState<number | null>(null);
  const [cursor, setCursor] = useState("text");
  const { w, h } = useSize(wrapRef);
  const span = view.end - view.start;
  const toX = (t: number) => ((t - view.start) / span) * w;
  const toT = (x: number) => clamp(view.start + (x / w) * span, 0, duration);

  // Couche de fond : ne change qu'avec les passages, la sélection ou la vue.
  useEffect(() => {
    const canvas = baseRef.current;
    if (!canvas || !w || !h) return;
    const ctx = setupCanvas(canvas, w, h);
    const c = palette(canvas);
    const mid = RULER + (h - RULER) / 2;
    const amp = (h - RULER) / 2 - 10;

    // Passages gardés : fond teinté ; le reste est barré de hachures.
    ctx.fillStyle = alpha(c.text, 0.06);
    ctx.fillRect(0, RULER, w, h - RULER);
    for (const s of segments) {
      const x0 = Math.max(0, toX(s.start));
      const x1 = Math.min(w, toX(s.end));
      if (x1 <= 0 || x0 >= w) continue;
      ctx.clearRect(x0, RULER, x1 - x0, h - RULER);
      ctx.fillStyle = alpha(c.audio, 0.09);
      ctx.fillRect(x0, RULER, x1 - x0, h - RULER);
    }

    // Forme d'onde, symétrique, colonne par colonne.
    const cols = columns(peaks, view.start, view.end, Math.max(1, Math.floor(w)));
    const kept = c.audio;
    const removed = alpha(c.muted, 0.45);
    let si = 0;
    for (let x = 0; x < cols.length; x++) {
      const t = view.start + ((x + 0.5) / cols.length) * span;
      while (si < segments.length && segments[si].end <= t) si++;
      const inside = si < segments.length && t >= segments[si].start;
      const half = Math.max(0.5, (cols[x] / 255) * amp);
      ctx.fillStyle = inside ? kept : removed;
      ctx.fillRect(x, mid - half, 1, half * 2);
    }
    ctx.fillStyle = alpha(c.text, 0.18);
    ctx.fillRect(0, mid - 0.5, w, 1);

    // Sélection.
    if (selection) {
      const x0 = toX(selection.a);
      const x1 = toX(selection.b);
      ctx.fillStyle = alpha(c.text, 0.16);
      ctx.fillRect(x0, RULER, x1 - x0, h - RULER);
      ctx.fillStyle = c.text;
      ctx.fillRect(x0 - 0.5, RULER, 1, h - RULER);
      ctx.fillRect(x1 - 0.5, RULER, 1, h - RULER);
    }

    // Bords des passages, avec une poignée en haut.
    ctx.fillStyle = c.audio;
    for (const s of segments) {
      for (const t of [s.start, s.end]) {
        const x = toX(t);
        if (x < -4 || x > w + 4) continue;
        ctx.fillRect(x - 1, RULER, 2, h - RULER);
        ctx.beginPath();
        ctx.roundRect(x - 4, RULER, 8, 14, 3);
        ctx.fill();
      }
    }

    // Règle.
    ctx.fillStyle = c.bg;
    ctx.fillRect(0, 0, w, RULER);
    ctx.fillStyle = c.line;
    ctx.fillRect(0, RULER - 1, w, 1);
    const pxPerSec = w / span;
    const step = STEPS.find((s) => s * pxPerSec >= 80) ?? 3600;
    ctx.font = "11px system-ui, sans-serif";
    ctx.textBaseline = "middle";
    for (let t = Math.ceil(view.start / step) * step; t <= view.end; t += step) {
      const x = toX(t);
      ctx.fillStyle = c.line;
      ctx.fillRect(x, RULER - 7, 1, 6);
      ctx.fillStyle = c.muted;
      ctx.fillText(formatTime(t, step < 1).replace(/,(\d)0$/, ",$1"), x + 4, RULER / 2 - 1);
    }
  }, [peaks, segments, selection, view.start, view.end, w, h]);

  // Couche du dessus : curseur de lecture et survol.
  useEffect(() => {
    const canvas = overlayRef.current;
    if (!canvas || !w || !h) return;
    const ctx = setupCanvas(canvas, w, h);
    const c = palette(canvas);
    if (hover !== null && !drag.current) {
      ctx.fillStyle = alpha(c.text, 0.35);
      ctx.fillRect(Math.round(toX(hover)), RULER, 1, h - RULER);
    }
    const x = toX(time);
    if (x >= -1 && x <= w + 1) {
      ctx.fillStyle = c.text;
      ctx.fillRect(x - 0.75, 0, 1.5, h);
      ctx.beginPath();
      ctx.moveTo(x - 5, 0);
      ctx.lineTo(x + 5, 0);
      ctx.lineTo(x, 7);
      ctx.fill();
    }
  }, [time, hover, view.start, view.end, w, h]);

  // Molette : défilement ; Ctrl + molette : zoom autour du pointeur.
  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const x = e.clientX - el.getBoundingClientRect().left;
      if (e.ctrlKey || e.metaKey) {
        props.onViewChange(zoomView(view, Math.exp(e.deltaY * 0.002), toT(x), duration));
      } else {
        const delta = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
        props.onViewChange(panView(view, (delta / w) * span, duration));
      }
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  /** Bord de passage sous le pointeur ; sur une coupe (deux passages collés), celui du côté du pointeur. */
  function edgeAt(x: number): { index: number; edge: "start" | "end" } | null {
    let best: { index: number; edge: "start" | "end"; d: number } | null = null;
    segments.forEach((s, index) => {
      for (const edge of ["start", "end"] as const) {
        const ex = toX(s[edge]);
        const d = Math.abs(ex - x);
        const sideOk = edge === "start" ? x >= ex - 1 : x <= ex + 1;
        if (d <= EDGE_HIT && (!best || d < best.d || (d === best.d && sideOk))) best = { index, edge, d };
      }
    });
    return best;
  }

  const localX = (e: React.PointerEvent) => e.clientX - wrapRef.current!.getBoundingClientRect().left;

  function onPointerDown(e: React.PointerEvent) {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const x = localX(e);
    const edge = edgeAt(x);
    drag.current = edge ? { mode: "edge", ...edge } : { mode: "select", anchor: toT(x), x, moved: false };
  }

  function onPointerMove(e: React.PointerEvent) {
    const x = localX(e);
    const d = drag.current;
    setHover(toT(x));
    if (!d) {
      setCursor(edgeAt(x) ? "ew-resize" : "text");
    } else if (d.mode === "edge") {
      props.onEdge(d.index, d.edge, toT(x), false);
    } else if (d.moved || Math.abs(x - d.x) > 3) {
      d.moved = true;
      const t = toT(x);
      props.onSelect({ a: Math.min(d.anchor, t), b: Math.max(d.anchor, t) });
    }
  }

  function onPointerUp(e: React.PointerEvent) {
    const d = drag.current;
    drag.current = null;
    if (!d) return;
    if (d.mode === "edge") {
      props.onEdge(d.index, d.edge, toT(localX(e)), true);
    } else if (!d.moved) {
      props.onSelect(null);
      props.onSeek(d.anchor);
    }
  }

  /** Double-clic : sélectionne le passage (ou le trou) sous le pointeur. */
  function onDoubleClick(e: React.MouseEvent) {
    const t = toT(e.clientX - wrapRef.current!.getBoundingClientRect().left);
    const seg = segments.find((s) => t >= s.start && t < s.end);
    if (seg) {
      props.onSelect({ a: seg.start, b: seg.end });
      return;
    }
    const prev = [...segments].reverse().find((s) => s.end <= t);
    const next = segments.find((s) => s.start > t);
    props.onSelect({ a: prev?.end ?? 0, b: next?.start ?? duration });
  }

  return (
    <div
      ref={wrapRef}
      className="waveform"
      style={{ cursor }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={() => setHover(null)}
      onDoubleClick={onDoubleClick}
    >
      <canvas ref={baseRef} style={{ width: w, height: h }} />
      <canvas ref={overlayRef} style={{ width: w, height: h }} />
    </div>
  );
}

/** Vue d'ensemble du fichier : la fenêtre visible y est déplaçable. */
export function Overview(props: {
  peaks: Uint8Array;
  duration: number;
  segments: Segment[];
  view: View;
  time: number;
  onViewChange: (view: View) => void;
}) {
  const { peaks, duration, segments, view, time } = props;
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const grab = useRef<number | null>(null);
  const { w, h } = useSize(wrapRef);
  const toX = (t: number) => (t / duration) * w;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !w || !h) return;
    const ctx = setupCanvas(canvas, w, h);
    const c = palette(canvas);
    const cols = columns(peaks, 0, duration, Math.max(1, Math.floor(w)));
    const removed = alpha(c.muted, 0.4);
    let si = 0;
    for (let x = 0; x < cols.length; x++) {
      const t = ((x + 0.5) / cols.length) * duration;
      while (si < segments.length && segments[si].end <= t) si++;
      const half = Math.max(0.5, (cols[x] / 255) * (h / 2 - 3));
      ctx.fillStyle = si < segments.length && t >= segments[si].start ? c.audio : removed;
      ctx.fillRect(x, h / 2 - half, 1, half * 2);
    }
    const x0 = toX(view.start);
    const x1 = Math.max(x0 + 4, toX(view.end));
    ctx.fillStyle = alpha(c.text, 0.12);
    ctx.fillRect(x0, 0, x1 - x0, h);
    ctx.strokeStyle = alpha(c.text, 0.55);
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.roundRect(x0 + 0.5, 0.5, x1 - x0 - 1, h - 1, 4);
    ctx.stroke();
    ctx.fillStyle = c.text;
    ctx.fillRect(toX(time) - 0.5, 0, 1, h);
  }, [peaks, segments, view.start, view.end, time, w, h]);

  function moveTo(clientX: number, offset: number) {
    const t = ((clientX - wrapRef.current!.getBoundingClientRect().left) / w) * duration;
    props.onViewChange(panView(view, t - offset - view.start, duration));
  }

  return (
    <div
      ref={wrapRef}
      className="overview"
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        const t = ((e.clientX - wrapRef.current!.getBoundingClientRect().left) / w) * duration;
        // Saisie de la fenêtre là où on l'attrape ; ailleurs, elle se centre sous le pointeur.
        grab.current = t >= view.start && t <= view.end ? t - view.start : (view.end - view.start) / 2;
        moveTo(e.clientX, grab.current);
      }}
      onPointerMove={(e) => grab.current !== null && moveTo(e.clientX, grab.current)}
      onPointerUp={() => (grab.current = null)}
    >
      <canvas ref={canvasRef} style={{ width: w, height: h }} />
    </div>
  );
}
