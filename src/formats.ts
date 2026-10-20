import type { Kind } from "./types";

const AUDIO = new Set(["mp3", "wav", "ogg", "oga", "flac", "m4a", "aac", "opus", "wma"]);
const VIDEO = new Set(["mp4", "m4v", "webm", "mkv", "mov", "avi", "wmv", "flv", "mpeg", "mpg", "3gp", "ts"]);
const IMAGE = new Set(["png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff", "ico", "tga"]);

export const isAudioFormat = (format: string) => AUDIO.has(format);

/** Type d'un format de sortie (pour la couleur des pétales). Un GIF compte comme image. */
export function kindOf(format: string): Kind {
  if (AUDIO.has(format)) return "audio";
  if (VIDEO.has(format)) return "video";
  return "image";
}

/** Type d'un fichier d'après son extension, ou null s'il n'est pas reconnu. */
export function kindOfPath(path: string): Kind | null {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  if (IMAGE.has(ext)) return "image";
  if (AUDIO.has(ext)) return "audio";
  if (VIDEO.has(ext)) return "video";
  return null;
}

export const basename = (path: string) => path.split(/[\\/]/).pop() ?? path;

const extOf = (path: string) => path.split(".").pop()?.toLowerCase() ?? "";

// Miroir de `edit::compress::supports` : BMP/TIFF/ICO/TGA et WAV sont à convertir d'abord.
const COMPRESSIBLE_IMAGE = new Set(["png", "jpg", "jpeg", "webp", "gif"]);
const COMPRESSIBLE_AUDIO = new Set(["mp3", "ogg", "oga", "opus", "m4a", "aac", "flac", "wma"]);

export function isCompressible(path: string): boolean {
  const ext = extOf(path);
  return COMPRESSIBLE_IMAGE.has(ext) || COMPRESSIBLE_AUDIO.has(ext) || VIDEO.has(ext);
}

export const isPng = (path: string) => extOf(path) === "png";

/** Audio/vidéo dont le débit peut viser une taille (le FLAC reste sans perte). */
export const hasBitrate = (path: string) => kindOfPath(path) !== "image" && extOf(path) !== "flac";

/** Poids lisible, en unités décimales (1 Mo = 10⁶ octets, comme la taille cible). */
export function formatBytes(bytes: number): string {
  const fmt = (n: number) => n.toLocaleString("fr-FR", { maximumFractionDigits: n < 10 ? 1 : 0 });
  if (bytes >= 1e9) return `${fmt(bytes / 1e9)} Go`;
  if (bytes >= 1e6) return `${fmt(bytes / 1e6)} Mo`;
  if (bytes >= 1e3) return `${fmt(bytes / 1e3)} Ko`;
  return `${bytes} o`;
}
