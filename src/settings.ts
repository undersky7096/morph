// Réglages partagés par toutes les fenêtres (miroir de `src-tauri/src/settings.rs`) :
// chargés au démarrage de chaque fenêtre, appliqués au thème, mis à jour en direct.

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

export type ThemeMode = "system" | "light" | "dark";
export type MixedBatch = "groups" | "common";
export type SaveMode = "beside" | "folder" | "ask";
export type Duplicates = "keep" | "replace" | "ask";

export interface Settings {
  theme: ThemeMode;
  accent: string | null;
  background: string | null;
  radialOpacity: number;
  favorites: string[];
  mixedBatch: MixedBatch;
  saveMode: SaveMode;
  saveFolder: string | null;
  duplicates: Duplicates;
  autostart: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  accent: null,
  background: null,
  radialOpacity: 82,
  favorites: [],
  mixedBatch: "groups",
  saveMode: "beside",
  saveFolder: null,
  duplicates: "keep",
  autostart: false,
};

let current: Settings = DEFAULT_SETTINGS;
const subscribers = new Set<(s: Settings) => void>();

const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

/** Luminance relative d'une couleur `#rrggbb` (0 = noir, 1 = blanc). */
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Thème effectif : un fond personnalisé impose clair ou sombre selon sa luminosité. */
function effectiveTheme(s: Settings): "light" | "dark" {
  if (s.background) return luminance(s.background) < 0.3 ? "dark" : "light";
  if (s.theme === "system") return darkQuery.matches ? "dark" : "light";
  return s.theme;
}

/** Dernière apparence appliquée (thème effectif, accent, fond). */
let look = "";

/** Variables CSS du thème, sur la racine du document de la fenêtre. `fade` : changement
 *  en cours d'utilisation ; si l'apparence change, la fenêtre passe de l'ancienne à la
 *  nouvelle en fondu (View Transitions : un seul fondu de l'image entière, sans
 *  transition élément par élément). */
function apply(s: Settings, fade = false) {
  const next = `${effectiveTheme(s)}|${s.accent}|${s.background}`;
  const changed = next !== look;
  look = next;
  const doc = document as Document & { startViewTransition?: (update: () => void) => unknown };
  if (fade && changed && doc.startViewTransition && !document.hidden) doc.startViewTransition(() => setVars(s));
  else setVars(s);
}

function setVars(s: Settings) {
  const root = document.documentElement;
  root.dataset.theme = effectiveTheme(s);
  const set = (name: string, value: string | null) =>
    value === null ? root.style.removeProperty(name) : root.style.setProperty(name, value);
  set("--accent", s.accent);
  set("--accent-2", s.accent && `color-mix(in srgb, ${s.accent} 55%, white)`);
  set("--bg", s.background);
  set("--surface-base", s.background);
  set("--radial-opacity", `${s.radialOpacity}%`);
}

/** À appeler une fois par fenêtre : charge, applique et suit les réglages. */
export function initSettings() {
  invoke<Settings>("get_settings")
    .then(update)
    .catch(() => apply(current));
  listen<Settings>("settings-changed", ({ payload }) => update(payload, true));
  darkQuery.addEventListener("change", () => apply(current, true));
  apply(current);
}

function update(s: Settings, fade = false) {
  current = s;
  apply(s, fade);
  subscribers.forEach((fn) => fn(s));
}

export const getSettings = () => current;

/** Réglages courants, mis à jour quand ils changent (depuis n'importe quelle fenêtre). */
export function useSettings(): Settings {
  const [s, setS] = useState(current);
  useEffect(() => {
    subscribers.add(setS);
    setS(current);
    return () => {
      subscribers.delete(setS);
    };
  }, []);
  return s;
}

/** Modifie les réglages (enregistrés, appliqués et diffusés par le cœur de Morph). */
export async function saveSettings(patch: Partial<Settings>): Promise<Settings> {
  const next = await invoke<Settings>("set_settings", { settings: { ...current, ...patch } });
  update(next, true);
  return next;
}

/**
 * Dossier de sortie d'une opération : `undefined` si le réglage n'est pas « demander »
 * (le cœur applique lui-même « à côté » ou « dossier fixe »), le dossier choisi, ou `null`
 * si l'utilisateur annule (l'opération ne doit pas avoir lieu).
 */
export async function chooseOutputDir(): Promise<string | null | undefined> {
  if (current.saveMode !== "ask") return undefined;
  const dir = await open({ directory: true, title: "Enregistrer dans…" });
  return typeof dir === "string" ? dir : null;
}
