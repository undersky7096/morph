import { invoke } from "@tauri-apps/api/core";
import type { Kind } from "./types";
import { locale, t } from "./i18n";

/** Ligne de la table des formats (`src-tauri/src/convert/catalog.rs`, seule source). */
export interface FormatInfo {
  ext: string;
  aliases: string[];
  kind: Kind;
  /** Moteur qui le lit / l'écrit (null : sortie seulement / entrée seulement). */
  read: string | null;
  write: string | null;
  /** Compressible tel quel par l'outil Compresser. */
  compress: boolean;
  /** Ouvrable dans Recadrer et l'éditeur d'image. */
  edit: boolean;
}

let catalog: FormatInfo[] = [];
const byExt = new Map<string, FormatInfo>();

/** Charge la table des formats (une fois par fenêtre, avant d'afficher l'interface). */
export async function loadCatalog() {
  catalog = await invoke<FormatInfo[]>("format_catalog");
  for (const f of catalog) for (const ext of [f.ext, ...f.aliases]) byExt.set(ext, f);
}

const extOf = (path: string) => path.split(".").pop()?.toLowerCase() ?? "";

/** Format (lisible) d'un fichier d'après son extension. */
const formatOfPath = (path: string) => {
  const format = byExt.get(extOf(path));
  return format?.read ? format : undefined;
};

export const isAudioFormat = (format: string) => byExt.get(format)?.kind === "audio";

/** Type d'un format de sortie (pour la couleur des pétales). Un GIF compte comme image. */
export function kindOf(format: string): Kind {
  return byExt.get(format)?.kind ?? "image";
}

/** Type d'un fichier d'après son extension, ou null s'il n'est pas reconnu. */
export function kindOfPath(path: string): Kind | null {
  return formatOfPath(path)?.kind ?? null;
}

/** Formats de sortie d'un type, dans l'ordre de la table. */
export const outputsOf = (kind: Kind) => catalog.filter((f) => f.kind === kind && f.write).map((f) => f.ext);

export const basename = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** BMP, TIFF, WAV… sont à convertir avant de pouvoir être compressés. */
export const isCompressible = (path: string) => formatOfPath(path)?.compress ?? false;

/** HEIC, SVG… ne s'affichent pas dans les outils : à convertir d'abord. */
export const isEditable = (path: string) => formatOfPath(path)?.edit ?? false;

export const isPng = (path: string) => extOf(path) === "png";

export const isPdf = (path: string) => extOf(path) === "pdf";

/** Audio/vidéo dont le débit peut viser une taille (le FLAC reste sans perte). */
export const hasBitrate = (path: string) => {
  const kind = kindOfPath(path);
  return (kind === "audio" || kind === "video") && extOf(path) !== "flac";
};

/** Poids lisible, en unités décimales (1 Mo = 10⁶ octets, comme la taille cible). */
export function formatBytes(bytes: number): string {
  const fmt = (n: number) => n.toLocaleString(locale(), { maximumFractionDigits: n < 10 ? 1 : 0 });
  if (bytes >= 1e9) return `${fmt(bytes / 1e9)} ${t("formats.gb")}`;
  if (bytes >= 1e6) return `${fmt(bytes / 1e6)} ${t("formats.mb")}`;
  if (bytes >= 1e3) return `${fmt(bytes / 1e3)} ${t("formats.kb")}`;
  return `${bytes} ${t("formats.b")}`;
}

/** Extensions lisibles des formats retenus (filtres des boîtes de dialogue d'ouverture). */
export const readableExts = (keep: (f: FormatInfo) => boolean) =>
  catalog.filter((f) => f.read && keep(f)).flatMap((f) => [f.ext, ...f.aliases]);

/** Nombre de formats reconnus en entrée, par type. */
export const readableCount = (kind?: Kind) => catalog.filter((f) => f.read && (!kind || f.kind === kind)).length;
