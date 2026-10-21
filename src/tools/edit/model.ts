// État de l'éditeur image : réglages, filtre, géométrie et annotations.
// Tout est non destructif : l'image d'origine n'est modifiée qu'à l'enregistrement.

import { t } from "../../i18n";

/** Réglages numériques : −100…100 (centrés sur 0) ou 0…100. */
export interface Adjust {
  exposure: number;
  brightness: number;
  contrast: number;
  saturation: number;
  temperature: number;
  sharpness: number;
  blur: number;
  vignette: number;
  grain: number;
}

export type AdjustKey = keyof Adjust;

export const ADJUST_DEFAULT: Adjust = {
  exposure: 0,
  brightness: 0,
  contrast: 0,
  saturation: 0,
  temperature: 0,
  sharpness: 0,
  blur: 0,
  vignette: 0,
  grain: 0,
};

export const SLIDERS: { key: AdjustKey; label: string; min: number }[] = [
  { key: "exposure", get label() { return t("Exposition", "Exposure"); }, min: -100 },
  { key: "brightness", get label() { return t("Luminosité", "Brightness"); }, min: -100 },
  { key: "contrast", get label() { return t("Contraste", "Contrast"); }, min: -100 },
  { key: "saturation", get label() { return t("Saturation", "Saturation"); }, min: -100 },
  { key: "temperature", get label() { return t("Température", "Temperature"); }, min: -100 },
  { key: "sharpness", get label() { return t("Netteté", "Sharpness"); }, min: 0 },
  { key: "blur", get label() { return t("Flou", "Blur"); }, min: 0 },
  { key: "vignette", get label() { return t("Vignettage", "Vignette"); }, min: 0 },
  { key: "grain", get label() { return t("Grain", "Grain"); }, min: 0 },
];

/** Filtre prédéfini : s'ajoute aux réglages de l'utilisateur. */
export interface Preset {
  id: string;
  label: string;
  adjust: Partial<Adjust>;
  /** Virage sépia, 0…1. */
  sepia?: number;
  /** Noirs relevés (aspect délavé), 0…1. */
  fade?: number;
}

export const PRESETS: Preset[] = [
  { id: "none", get label() { return t("Aucun", "None"); }, adjust: {} },
  { id: "bw", get label() { return t("N&B", "B&W"); }, adjust: { saturation: -100, contrast: 10 } },
  { id: "noir", get label() { return t("Noir", "Noir"); }, adjust: { saturation: -100, contrast: 55, vignette: 50, grain: 20 } },
  { id: "sepia", get label() { return t("Sépia", "Sepia"); }, adjust: { contrast: 5 }, sepia: 0.85 },
  { id: "vintage", get label() { return t("Vintage", "Vintage"); }, adjust: { saturation: -20, contrast: -10, vignette: 35 }, sepia: 0.35, fade: 0.1 },
  { id: "fade", get label() { return t("Délavé", "Faded"); }, adjust: { contrast: -15, saturation: -15 }, fade: 0.18 },
  { id: "warm", get label() { return t("Chaud", "Warm"); }, adjust: { temperature: 35, saturation: 10 } },
  { id: "cold", get label() { return t("Froid", "Cool"); }, adjust: { temperature: -35, saturation: -5 } },
  { id: "vivid", get label() { return t("Éclatant", "Vivid"); }, adjust: { saturation: 45, contrast: 15 } },
  { id: "dramatic", get label() { return t("Dramatique", "Dramatic"); }, adjust: { contrast: 40, saturation: -25, vignette: 45, exposure: -5 } },
];

export interface Geometry {
  /** Quarts de tour dans le sens horaire (0…3). */
  quarter: number;
  flipH: boolean;
  flipV: boolean;
  /** Redressement fin en degrés (−45…45), avec agrandissement pour masquer les coins. */
  straighten: number;
}

export const GEOMETRY_DEFAULT: Geometry = { quarter: 0, flipH: false, flipV: false, straighten: 0 };

// Annotations, en pixels de l'image de sortie (après rotation).

interface ShapeBase {
  id: string;
  x: number;
  y: number;
  rotation: number;
  scaleX: number;
  scaleY: number;
  color: string;
}

/** Tracé à main levée : pinceau, surligneur (translucide) ou gomme (efface les annotations). */
export interface Stroke extends ShapeBase {
  type: "brush" | "highlight" | "eraser";
  points: number[];
  width: number;
}

export interface Segment extends ShapeBase {
  type: "line" | "arrow";
  points: [number, number, number, number];
  width: number;
}

/** Rectangle : (x, y) = coin haut gauche ; ellipse : (x, y) = centre. */
export interface Box extends ShapeBase {
  type: "rect" | "ellipse";
  w: number;
  h: number;
  width: number;
  fill: boolean;
}

export interface Label extends ShapeBase {
  type: "text";
  text: string;
  font: string;
  size: number;
  bold: boolean;
  outline: boolean;
  shadow: boolean;
}

export type Shape = Stroke | Segment | Box | Label;

export interface EditState {
  adjust: Adjust;
  invert: boolean;
  preset: string;
  geometry: Geometry;
  shapes: Shape[];
  /** Fond retiré (éditeur d'image). */
  cutout: boolean;
}

export const EDIT_DEFAULT: EditState = {
  adjust: ADJUST_DEFAULT,
  invert: false,
  preset: "none",
  geometry: GEOMETRY_DEFAULT,
  shapes: [],
  cutout: false,
};

export type AnnotateTool = "select" | "text" | "brush" | "highlight" | "eraser" | "arrow" | "line" | "rect" | "ellipse";

/** Style appliqué aux nouvelles annotations. */
export interface DrawStyle {
  color: string;
  /** Épaisseur, en fraction du plus grand côté de l'image (indépendante de la résolution). */
  width: number;
  fill: boolean;
  font: string;
  /** Taille du texte, en fraction du plus grand côté. */
  size: number;
  bold: boolean;
  outline: boolean;
  shadow: boolean;
}

export const STYLE_DEFAULT: DrawStyle = {
  color: "#ff3b30",
  width: 0.006,
  fill: false,
  font: "Inter",
  size: 0.06,
  bold: true,
  outline: false,
  shadow: true,
};

export const COLORS = ["#ff3b30", "#ff9500", "#ffcc00", "#34c759", "#0a84ff", "#af52de", "#ffffff", "#000000"];

export const FONTS = ["Inter", "Arial", "Georgia", "Times New Roman", "Courier New", "Impact", "Comic Sans MS"];

/** Somme des réglages de l'utilisateur et du filtre, bornée. */
export function combined(state: EditState): { adjust: Adjust; sepia: number; fade: number } {
  const preset = PRESETS.find((p) => p.id === state.preset) ?? PRESETS[0];
  const adjust = { ...state.adjust };
  for (const { key, min } of SLIDERS) {
    adjust[key] = Math.max(min, Math.min(100, adjust[key] + (preset.adjust[key] ?? 0)));
  }
  return { adjust, sepia: preset.sepia ?? 0, fade: preset.fade ?? 0 };
}
