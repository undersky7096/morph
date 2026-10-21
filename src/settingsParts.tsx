// Contrôles de réglages partagés par la fenêtre des Réglages et l'écran d'accueil.

import { motion } from "motion/react";

import { kindOf, outputsOf } from "./formats";
import type { Settings, ThemeMode } from "./settings";
import type { Kind } from "./types";

export const THEMES: [ThemeMode, string][] = [
  ["system", "Système"],
  ["light", "Clair"],
  ["dark", "Sombre"],
];

export const ACCENTS = ["#2f6bff", "#8b5cf6", "#ec4899", "#ef4444", "#f97316", "#eab308", "#10b981", "#06b6d4"];
export const BACKGROUNDS = ["#0b0d12", "#16112a", "#0d1f24", "#1f1414", "#f7f3ea", "#eef3ff", "#fdf0f4", "#eefaf3"];

/** Formats de sortie par type, pour choisir les favoris. */
const OUTPUTS: [string, Kind][] = [
  ["Images", "image"],
  ["Audio", "audio"],
  ["Vidéo", "video"],
  ["Documents", "document"],
  ["Archives", "archive"],
];

/** Glissement de l'indicateur de sélection (navigation, choix du thème). */
export const PILL = { type: "spring", stiffness: 500, damping: 38 } as const;

type Save = (patch: Partial<Settings>) => void;

/** Système / Clair / Sombre ; un thème prédéfini remplace les couleurs personnalisées. */
export function ThemePicker({ settings, save, id = "theme-active" }: { settings: Settings; save: Save; id?: string }) {
  const custom = settings.accent !== null || settings.background !== null;
  return (
    <div className="settings-segmented">
      {THEMES.map(([theme, label]) => {
        const active = !custom && settings.theme === theme;
        return (
          <button
            key={theme}
            className={active ? "is-active" : ""}
            onClick={() => save({ theme, accent: null, background: null })}
          >
            {active && <motion.span layoutId={id} className="settings-segmented-pill" transition={PILL} />}
            <span>{label}</span>
          </button>
        );
      })}
    </div>
  );
}

/** Pastilles de couleur + sélecteur libre ; « Par défaut » efface la couleur personnalisée. */
export function ColorChoice(props: { value: string | null; colors: string[]; onChange: (c: string | null) => void }) {
  const { value, colors, onChange } = props;
  const custom = value !== null && !colors.includes(value);
  return (
    <div className="settings-colors">
      <button className={`settings-chip ${value === null ? "is-active" : ""}`} onClick={() => onChange(null)}>
        Par défaut
      </button>
      {colors.map((c) => (
        <button
          key={c}
          className={`settings-swatch ${value === c ? "is-active" : ""}`}
          style={{ background: c }}
          aria-label={c}
          onClick={() => onChange(c)}
        />
      ))}
      <label
        className={`settings-swatch settings-picker ${custom ? "is-active" : ""}`}
        title="Autre couleur"
        style={custom ? { background: value } : undefined}
      >
        <input type="color" value={value ?? "#808080"} onChange={(e) => onChange(e.target.value)} />
        {!custom && "+"}
      </label>
    </div>
  );
}

/** Formats favoris, numérotés dans l'ordre où ils ont été choisis. */
export function FavoritesPicker({ settings, save }: { settings: Settings; save: Save }) {
  const toggle = (f: string) =>
    save({
      favorites: settings.favorites.includes(f) ? settings.favorites.filter((x) => x !== f) : [...settings.favorites, f],
    });
  return (
    <>
      {OUTPUTS.map(([label, kind]) => (
        <div key={label} className="settings-formats">
          <span>{label}</span>
          <div>
            {outputsOf(kind).map((f) => {
              const rank = settings.favorites.indexOf(f);
              return (
                <button
                  key={f}
                  className={`settings-format kind-${kindOf(f)} ${rank >= 0 ? "is-active" : ""}`}
                  onClick={() => toggle(f)}
                >
                  {rank >= 0 && <em>{rank + 1}</em>}
                  {f.toUpperCase()}
                </button>
              );
            })}
          </div>
        </div>
      ))}
    </>
  );
}
