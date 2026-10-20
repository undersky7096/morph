import { type ReactNode, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { chooseOutputDir } from "../../settings";
import {
  ArrowIcon,
  BrushIcon,
  EllipseIcon,
  EraserIcon,
  FlipHIcon,
  FlipVIcon,
  HighlighterIcon,
  LineIcon,
  PointerIcon,
  RectIcon,
  RedoIcon,
  RotateLeftIcon,
  RotateRightIcon,
  TextIcon,
  TrashIcon,
  UndoIcon,
} from "../../icons";
import Canvas, { type CanvasHandle } from "./Canvas";
import { presetThumbs, Slider } from "./controls";
import { useHistory } from "./history";
import {
  ADJUST_DEFAULT,
  type AnnotateTool,
  COLORS,
  combined,
  type DrawStyle,
  EDIT_DEFAULT,
  type EditState,
  FONTS,
  GEOMETRY_DEFAULT,
  PRESETS,
  type Shape,
  SLIDERS,
  STYLE_DEFAULT,
} from "./model";
import { Renderer } from "./renderer";
import "./ImageEditor.css";

type Tab = "adjust" | "filters" | "annotate";

const TOOLS: { id: AnnotateTool; label: string; icon: () => ReactNode }[] = [
  { id: "select", label: "Sélectionner / déplacer", icon: PointerIcon },
  { id: "text", label: "Texte", icon: TextIcon },
  { id: "brush", label: "Pinceau", icon: BrushIcon },
  { id: "highlight", label: "Surligneur", icon: HighlighterIcon },
  { id: "eraser", label: "Gomme (efface les annotations)", icon: EraserIcon },
  { id: "arrow", label: "Flèche", icon: ArrowIcon },
  { id: "line", label: "Trait", icon: LineIcon },
  { id: "rect", label: "Rectangle", icon: RectIcon },
  { id: "ellipse", label: "Ellipse", icon: EllipseIcon },
];

/** Épaisseur réelle d'un tracé : le surligneur et la gomme sont plus larges que le pinceau. */
const WIDTH_FACTOR: Partial<Record<Shape["type"], number>> = { highlight: 4, eraser: 3 };

interface Props {
  path: string;
  /** Appelé avec le chemin du fichier enregistré. */
  onSaved: (output: string) => void;
  /** Boutons supplémentaires du pied de fenêtre (ex : « Passer » dans un lot). */
  footerExtra?: ReactNode;
}

export default function ImageEditor({ path, onSaved, footerExtra }: Props) {
  const history = useHistory<EditState>(EDIT_DEFAULT);
  const { state, update } = history;
  const [renderer, setRenderer] = useState<Renderer | null>(null);
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
  const [version, setVersion] = useState(0);
  const [size, setSize] = useState<{ w: number; h: number } | null>(null);
  const [scale, setScale] = useState(1);
  const [tab, setTab] = useState<Tab>("adjust");
  const [tool, setTool] = useState<AnnotateTool>("select");
  const [style, setStyle] = useState<DrawStyle>(STYLE_DEFAULT);
  const [selected, setSelected] = useState<string | null>(null);
  const [focusText, setFocusText] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<CanvasHandle>(null);
  const textRef = useRef<HTMLTextAreaElement>(null);

  // Chargement : octets via IPC → image (orientation EXIF appliquée au dessin) → textures.
  useEffect(() => {
    let url: string | null = null;
    let cancelled = false;
    let main: Renderer | null = null;
    invoke<ArrayBuffer>("read_image", { path })
      .then((bytes) => {
        url = URL.createObjectURL(new Blob([bytes]));
        const img = new Image();
        img.src = url;
        return img.decode().then(() => img);
      })
      .then((img) => {
        if (cancelled) return;
        main = new Renderer();
        main.setImage(img);
        setRenderer(main);
        setThumbs(presetThumbs(img));
      })
      .catch((e) => setError(String(e)));
    return () => {
      cancelled = true;
      main?.dispose();
      if (url) URL.revokeObjectURL(url);
    };
  }, [path]);

  // Rendu WebGL à chaque changement de réglage, de filtre ou de géométrie.
  const { adjust, invert, preset, geometry } = state;
  useLayoutEffect(() => {
    if (!renderer) return;
    renderer.render({ ...combined(state), invert, geometry });
    setSize(renderer.outputSize(geometry));
    setVersion((v) => v + 1);
  }, [renderer, adjust, invert, preset, geometry]);

  // Échelle d'affichage : l'image tient dans la zone disponible.
  useLayoutEffect(() => {
    const area = areaRef.current;
    if (!area || !size) return;
    const fit = () => {
      const pad = 48;
      setScale(Math.min((area.clientWidth - pad) / size.w, (area.clientHeight - pad - 44) / size.h, 4));
    };
    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(area);
    return () => observer.disconnect();
  }, [size]);

  const side = size ? Math.max(size.w, size.h) : 1;
  const selectedShape = state.shapes.find((s) => s.id === selected) ?? null;

  useEffect(() => {
    if (focusText && selected === focusText && textRef.current) {
      textRef.current.focus();
      textRef.current.select();
      setFocusText(null);
    }
  }, [focusText, selected, tab]);

  function upsertShape(shape: Shape, mergeKey?: string) {
    update((s) => {
      const exists = s.shapes.some((x) => x.id === shape.id);
      return { ...s, shapes: exists ? s.shapes.map((x) => (x.id === shape.id ? shape : x)) : [...s.shapes, shape] };
    }, mergeKey);
  }

  function onCanvasChange(shape: Shape) {
    const isNew = !state.shapes.some((s) => s.id === shape.id);
    upsertShape(shape);
    if (isNew && shape.type === "text") {
      setTool("select");
      setFocusText(shape.id);
    }
  }

  function deleteSelected() {
    if (!selected) return;
    update((s) => ({ ...s, shapes: s.shapes.filter((x) => x.id !== selected) }));
    setSelected(null);
  }

  /** Change un style : sur l'annotation sélectionnée s'il y en a une, et pour les suivantes. */
  function setStyleProp<K extends keyof DrawStyle>(key: K, value: DrawStyle[K]) {
    setStyle((s) => ({ ...s, [key]: value }));
    const shape = selectedShape;
    if (!shape) return;
    const next: Record<string, unknown> = {};
    if (key === "width") next.width = Math.max(1, (value as number) * side * (WIDTH_FACTOR[shape.type] ?? 1));
    else if (key === "size") next.size = Math.round((value as number) * side);
    else if (key === "color" || key === "fill" || key === "font" || key === "bold" || key === "outline" || key === "shadow")
      next[key] = value;
    if (Object.keys(next).length && (key in shape || key === "size")) {
      upsertShape({ ...shape, ...next } as Shape, `shape.${shape.id}.${key}`);
    }
  }

  /** Valeur affichée dans le panneau : celle de l'annotation sélectionnée, sinon le style courant. */
  const shown: DrawStyle = useMemo(() => {
    const s = selectedShape;
    if (!s) return style;
    return {
      ...style,
      color: s.color,
      ...("width" in s ? { width: s.width / side / (WIDTH_FACTOR[s.type] ?? 1) } : {}),
      ...(s.type === "rect" || s.type === "ellipse" ? { fill: s.fill } : {}),
      ...(s.type === "text"
        ? { font: s.font, size: s.size / side, bold: s.bold, outline: s.outline, shadow: s.shadow }
        : {}),
    };
  }, [selectedShape, style, side]);

  async function save() {
    if (busy || !canvasRef.current) return;
    const outDir = await chooseOutputDir();
    if (outDir === null) return;
    setBusy(true);
    setError(null);
    try {
      const blob = await canvasRef.current.exportPng();
      const bytes = new Uint8Array(await blob.arrayBuffer());
      const output = await invoke<string>("save_edited", bytes, {
        headers: { "x-path": encodeURIComponent(path), ...(outDir ? { "x-out-dir": encodeURIComponent(outDir) } : {}) },
      });
      onSaved(output);
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  // Raccourcis : annuler/rétablir, supprimer, Entrée ou Ctrl+S pour enregistrer.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key.toLowerCase() === "s") {
        e.preventDefault();
        save();
      } else if (typing) {
        return;
      } else if (mod && e.key.toLowerCase() === "z") {
        e.preventDefault();
        if (e.shiftKey) history.redo();
        else history.undo();
      } else if (mod && e.key.toLowerCase() === "y") {
        e.preventDefault();
        history.redo();
      } else if (e.key === "Delete" || e.key === "Backspace") {
        deleteSelected();
      } else if (e.key === "Escape") {
        setSelected(null);
      } else if (e.key === "Enter") {
        save();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const setGeometry = (fn: (g: typeof geometry) => typeof geometry) =>
    update((s) => ({ ...s, geometry: fn(s.geometry) }), "geometry.straighten");

  const kind = selectedShape?.type ?? tool;
  const hasColor = kind !== "eraser" && kind !== "select";
  const hasWidth = ["brush", "highlight", "eraser", "arrow", "line", "rect", "ellipse"].includes(kind);
  const hasFill = kind === "rect" || kind === "ellipse";
  const isText = kind === "text";

  return (
    <div className="editor">
      <div className="editor-area" ref={areaRef}>
        <div className="editor-toolbar">
          <button title="Annuler (Ctrl+Z)" onClick={history.undo} disabled={!history.canUndo}>
            <UndoIcon />
          </button>
          <button title="Rétablir (Ctrl+Y)" onClick={history.redo} disabled={!history.canRedo}>
            <RedoIcon />
          </button>
          <span className="editor-toolbar-sep" />
          <button title="Pivoter à gauche" onClick={() => update((s) => ({ ...s, geometry: { ...s.geometry, quarter: (s.geometry.quarter + 3) % 4 } }))}>
            <RotateLeftIcon />
          </button>
          <button title="Pivoter à droite" onClick={() => update((s) => ({ ...s, geometry: { ...s.geometry, quarter: (s.geometry.quarter + 1) % 4 } }))}>
            <RotateRightIcon />
          </button>
          <button title="Miroir horizontal" onClick={() => update((s) => ({ ...s, geometry: { ...s.geometry, flipH: !s.geometry.flipH } }))}>
            <FlipHIcon />
          </button>
          <button title="Miroir vertical" onClick={() => update((s) => ({ ...s, geometry: { ...s.geometry, flipV: !s.geometry.flipV } }))}>
            <FlipVIcon />
          </button>
        </div>

        <div className="editor-stage">
          {renderer && size && (
            <Canvas
              ref={canvasRef}
              base={renderer.canvas}
              version={version}
              width={size.w}
              height={size.h}
              scale={scale}
              shapes={state.shapes}
              tool={tab === "annotate" ? tool : "select"}
              style={style}
              selected={selected}
              onSelect={setSelected}
              onChange={onCanvasChange}
            />
          )}
          {!renderer && !error && <p className="tool-hint">Chargement…</p>}
        </div>
      </div>

      <aside className="tool-panel editor-panel">
        <div className="editor-tabs" role="tablist">
          {(
            [
              ["adjust", "Réglages"],
              ["filters", "Filtres"],
              ["annotate", "Annoter"],
            ] as const
          ).map(([id, label]) => (
            <button key={id} role="tab" aria-selected={tab === id} className={tab === id ? "is-active" : ""} onClick={() => setTab(id)}>
              {label}
            </button>
          ))}
        </div>

        {tab === "adjust" && (
          <>
            <section>
              {SLIDERS.map(({ key, label, min }) => (
                <Slider
                  key={key}
                  label={label}
                  min={min}
                  max={100}
                  value={adjust[key]}
                  onChange={(v) => update((s) => ({ ...s, adjust: { ...s.adjust, [key]: v } }), `adjust.${key}`)}
                />
              ))}
              <label className="check">
                <input type="checkbox" checked={invert} onChange={(e) => update((s) => ({ ...s, invert: e.target.checked }))} />
                Inverser les couleurs
              </label>
            </section>
            <section>
              <h2>Géométrie</h2>
              <Slider
                label="Redresser"
                min={-45}
                max={45}
                step={0.5}
                unit="°"
                value={geometry.straighten}
                onChange={(v) => setGeometry((g) => ({ ...g, straighten: v }))}
              />
            </section>
            <button
              className="link"
              onClick={() => update((s) => ({ ...s, adjust: ADJUST_DEFAULT, invert: false, geometry: GEOMETRY_DEFAULT }))}
            >
              Réinitialiser les réglages
            </button>
          </>
        )}

        {tab === "filters" && (
          <section>
            <div className="presets">
              {PRESETS.map((p) => (
                <button
                  key={p.id}
                  className={`preset ${preset === p.id ? "is-active" : ""}`}
                  onClick={() => update((s) => ({ ...s, preset: p.id }))}
                >
                  {thumbs[p.id] ? <img src={thumbs[p.id]} alt="" /> : <span className="preset-blank" />}
                  <span>{p.label}</span>
                </button>
              ))}
            </div>
            <p className="tool-hint">Le filtre s'ajoute aux réglages : on peut l'affiner dans l'onglet Réglages.</p>
          </section>
        )}

        {tab === "annotate" && (
          <>
            <section>
              <div className="annotate-tools">
                {TOOLS.map(({ id, label, icon: Icon }) => (
                  <button
                    key={id}
                    title={label}
                    aria-label={label}
                    className={tool === id ? "is-active" : ""}
                    onClick={() => {
                      setTool(id);
                      if (id !== "select") setSelected(null);
                    }}
                  >
                    <Icon />
                  </button>
                ))}
              </div>
            </section>

            {selectedShape?.type === "text" && (
              <section>
                <h2>Texte</h2>
                <textarea
                  ref={textRef}
                  className="editor-text"
                  rows={3}
                  value={selectedShape.text}
                  onChange={(e) => upsertShape({ ...selectedShape, text: e.target.value }, `shape.${selectedShape.id}.text`)}
                />
              </section>
            )}

            {(hasColor || hasWidth || isText) && (
              <section>
                <h2>Style</h2>
                {hasColor && (
                  <div className="swatches">
                    {COLORS.map((c) => (
                      <button
                        key={c}
                        className={`swatch ${shown.color === c ? "is-active" : ""}`}
                        style={{ background: c }}
                        aria-label={c}
                        onClick={() => setStyleProp("color", c)}
                      />
                    ))}
                    <input
                      type="color"
                      className="swatch-custom"
                      title="Autre couleur"
                      value={shown.color}
                      onChange={(e) => setStyleProp("color", e.target.value)}
                    />
                  </div>
                )}
                {hasWidth && (
                  <Slider
                    label="Épaisseur"
                    min={1}
                    max={60}
                    value={Math.round(shown.width * 2000)}
                    onChange={(v) => setStyleProp("width", v / 2000)}
                  />
                )}
                {hasFill && (
                  <label className="check">
                    <input type="checkbox" checked={shown.fill} onChange={(e) => setStyleProp("fill", e.target.checked)} />
                    Plein
                  </label>
                )}
                {isText && (
                  <>
                    <select className="editor-select" value={shown.font} onChange={(e) => setStyleProp("font", e.target.value)}>
                      {FONTS.map((f) => (
                        <option key={f} value={f} style={{ fontFamily: f }}>
                          {f}
                        </option>
                      ))}
                    </select>
                    <Slider
                      label="Taille"
                      min={1}
                      max={30}
                      step={0.5}
                      unit=" %"
                      value={Math.round(shown.size * 200) / 2}
                      onChange={(v) => setStyleProp("size", v / 100)}
                    />
                    <label className="check">
                      <input type="checkbox" checked={shown.bold} onChange={(e) => setStyleProp("bold", e.target.checked)} />
                      Gras
                    </label>
                    <label className="check">
                      <input type="checkbox" checked={shown.outline} onChange={(e) => setStyleProp("outline", e.target.checked)} />
                      Contour
                    </label>
                    <label className="check">
                      <input type="checkbox" checked={shown.shadow} onChange={(e) => setStyleProp("shadow", e.target.checked)} />
                      Ombre
                    </label>
                  </>
                )}
              </section>
            )}

            {selectedShape && (
              <button className="link link-danger" onClick={deleteSelected}>
                <TrashIcon /> Supprimer l'annotation
              </button>
            )}
            <p className="tool-hint">
              {tool === "select"
                ? "Cliquez une annotation pour la déplacer, la redimensionner ou la faire pivoter. Suppr pour l'effacer."
                : tool === "text"
                  ? "Cliquez sur l'image pour placer un texte."
                  : "Dessinez directement sur l'image."}
            </p>
          </>
        )}
      </aside>

      <footer className="tool-footer">
        <span className={`tool-status ${error ? "is-error" : ""}`}>
          {error ??
            (size
              ? `${size.w} × ${size.h} px${renderer?.downscaled ? " (réduite : trop grande pour l'éditeur)" : ""}`
              : "")}
        </span>
        {footerExtra}
        <button className="primary" onClick={save} disabled={!renderer || busy}>
          {busy ? "Enregistrement…" : "Enregistrer"}
        </button>
      </footer>
    </div>
  );
}
