// Contrôles partagés par les éditeurs image et vidéo (sans Konva, chargé à part).

import { motion } from "motion/react";

import { combined, EDIT_DEFAULT, GEOMETRY_DEFAULT, PRESETS } from "./model";
import { Renderer } from "./renderer";

/** Onglets du panneau latéral ; l'onglet actif est repéré par un fond qui glisse. */
export function EditorTabs<T extends string>(props: {
  tabs: readonly (readonly [T, string])[];
  value: T;
  onChange: (tab: T) => void;
  className?: string;
}) {
  const { tabs, value, onChange, className = "" } = props;
  return (
    <div className={`editor-tabs ${className}`} role="tablist">
      {tabs.map(([id, label]) => (
        <button key={id} role="tab" aria-selected={value === id} className={value === id ? "is-active" : ""} onClick={() => onChange(id)}>
          {value === id && (
            <motion.span layoutId="editor-tab" className="editor-tab-pill" transition={{ type: "spring", stiffness: 500, damping: 38 }} />
          )}
          <span>{label}</span>
        </button>
      ))}
    </div>
  );
}

export function Slider(props: {
  label: string;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  value: number;
  /** Valeur sans effet, rétablie au double-clic (par défaut 0, ou le minimum). */
  neutral?: number;
  onChange: (v: number) => void;
}) {
  const { label, min, max, step = 1, unit = "", value, onChange } = props;
  const neutral = props.neutral ?? (min < 0 ? 0 : min);
  return (
    <label
      className="slider"
      onDoubleClick={() => onChange(neutral)}
      title={`Double-clic : remettre à ${neutral.toLocaleString("fr-FR")}${unit}`}
    >
      <span className="slider-head">
        <span>{label}</span>
        <span className={`slider-value ${value !== neutral ? "is-changed" : ""}`}>
          {value > 0 && min < 0 ? "+" : ""}
          {value.toLocaleString("fr-FR")}
          {unit}
        </span>
      </span>
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} />
    </label>
  );
}

/** Vignettes des filtres, rendues sur une copie réduite de l'image. */
export function presetThumbs(img: CanvasImageSource & { width: number; height: number }): Record<string, string> {
  const r = new Renderer();
  r.setImage(img, 160);
  const out: Record<string, string> = {};
  for (const p of PRESETS) {
    r.render({ ...combined({ ...EDIT_DEFAULT, preset: p.id }), invert: false, geometry: GEOMETRY_DEFAULT });
    out[p.id] = r.canvas.toDataURL("image/jpeg", 0.8);
  }
  r.dispose();
  return out;
}
