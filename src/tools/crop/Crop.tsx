import { type ReactNode, useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { dragRect, fitRatio, type Handle, type Rect, resizeTo, type Size, toPixels } from "./geometry";
import "./Crop.css";

const RATIOS: { label: string; value: number | null }[] = [
  { label: "Libre", value: null },
  { label: "1:1", value: 1 },
  { label: "16:9", value: 16 / 9 },
  { label: "9:16", value: 9 / 16 },
  { label: "4:3", value: 4 / 3 },
  { label: "3:4", value: 3 / 4 },
];

const EDGE_HANDLES: Handle[] = ["n", "s", "e", "w"];
const CORNER_HANDLES: Handle[] = ["nw", "ne", "sw", "se"];

interface Props {
  path: string;
  /** Appelé avec le chemin du fichier recadré une fois enregistré. */
  onSaved: (output: string) => void;
  /** Boutons supplémentaires du pied de fenêtre (ex : « Passer » dans un lot). */
  footerExtra?: ReactNode;
}

export default function Crop({ path, onSaved, footerExtra }: Props) {
  const [src, setSrc] = useState<string | null>(null);
  const [img, setImg] = useState<Size | null>(null);
  const [rect, setRect] = useState<Rect | null>(null);
  const [ratio, setRatio] = useState<number | null>(null);
  const [scale, setScale] = useState(1);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const canvasRef = useRef<HTMLDivElement>(null);
  const drag = useRef<{ handle: Handle; x: number; y: number; start: Rect } | null>(null);

  // Chargement de l'image (octets bruts via IPC → URL locale).
  useEffect(() => {
    let url: string | null = null;
    setError(null);
    invoke<ArrayBuffer>("read_image", { path })
      .then((bytes) => {
        url = URL.createObjectURL(new Blob([bytes]));
        setSrc(url);
      })
      .catch((e) => setError(String(e)));
    return () => {
      if (url) URL.revokeObjectURL(url);
    };
  }, [path]);

  // Échelle d'affichage : l'image tient entièrement dans la zone disponible.
  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !img) return;
    const update = () => {
      const pad = 48;
      setScale(Math.min((canvas.clientWidth - pad) / img.w, (canvas.clientHeight - pad) / img.h));
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [img]);

  function onImageLoad(e: React.SyntheticEvent<HTMLImageElement>) {
    const size = { w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight };
    setImg(size);
    setRect(fitRatio(size, ratio));
  }

  function chooseRatio(value: number | null) {
    setRatio(value);
    if (img && value) setRect(fitRatio(img, value));
  }

  function onPointerDown(e: React.PointerEvent, handle: Handle) {
    if (!rect) return;
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { handle, x: e.clientX, y: e.clientY, start: rect };
  }

  function onPointerMove(e: React.PointerEvent) {
    const d = drag.current;
    if (!d || !img) return;
    setRect(dragRect(d.start, d.handle, (e.clientX - d.x) / scale, (e.clientY - d.y) / scale, img, ratio));
  }

  function onPointerUp() {
    drag.current = null;
  }

  function onDimension(dim: "w" | "h", value: string) {
    const n = Number(value);
    if (!rect || !img || !Number.isFinite(n) || n <= 0) return;
    setRect(resizeTo(rect, dim, n, img, ratio));
  }

  async function apply() {
    if (!rect || !img || busy) return;
    setBusy(true);
    setError(null);
    try {
      const output = await invoke<string>("crop_image", { path, rect: toPixels(rect, img) });
      onSaved(output);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter" && !(e.target instanceof HTMLInputElement)) apply();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const px = rect && img ? toPixels(rect, img) : null;
  const handles = ratio ? CORNER_HANDLES : [...CORNER_HANDLES, ...EDGE_HANDLES];

  return (
    <div className="crop">
      <div className="crop-canvas" ref={canvasRef}>
        {src && (
          <div
            className="crop-stage"
            style={img ? { width: img.w * scale, height: img.h * scale } : undefined}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
          >
            <img src={src} alt="" draggable={false} onLoad={onImageLoad} />
            {rect && (
              <div
                className="crop-box"
                style={{ left: rect.x * scale, top: rect.y * scale, width: rect.w * scale, height: rect.h * scale }}
                onPointerDown={(e) => onPointerDown(e, "move")}
              >
                <div className="crop-grid" />
                {handles.map((h) => (
                  <div key={h} className={`crop-handle h-${h}`} onPointerDown={(e) => onPointerDown(e, h)} />
                ))}
              </div>
            )}
          </div>
        )}
        {!src && !error && <p className="tool-hint">Chargement…</p>}
      </div>

      <aside className="tool-panel">
        <section>
          <h2>Format</h2>
          <div className="chips">
            {RATIOS.map((r) => (
              <button
                key={r.label}
                className={`chip ${ratio === r.value ? "is-active" : ""}`}
                onClick={() => chooseRatio(r.value)}
              >
                {r.label}
              </button>
            ))}
          </div>
        </section>

        <section>
          <h2>Dimensions</h2>
          <div className="dims">
            <label>
              <span>L</span>
              <input type="number" min={1} value={px?.width ?? ""} onChange={(e) => onDimension("w", e.target.value)} />
            </label>
            <span className="dims-x">×</span>
            <label>
              <span>H</span>
              <input type="number" min={1} value={px?.height ?? ""} onChange={(e) => onDimension("h", e.target.value)} />
            </label>
          </div>
          {img && (
            <p className="tool-hint">
              Image d'origine : {img.w} × {img.h} px
            </p>
          )}
        </section>

        <button className="link" onClick={() => img && setRect(fitRatio(img, ratio))}>
          Réinitialiser
        </button>
      </aside>

      <footer className="tool-footer">
        <span className={`tool-status ${error ? "is-error" : ""}`}>{error ?? (px ? `${px.width} × ${px.height} px` : "")}</span>
        {footerExtra}
        <button className="primary" onClick={apply} disabled={!rect || busy}>
          {busy ? "Enregistrement…" : "Appliquer"}
        </button>
      </footer>
    </div>
  );
}
