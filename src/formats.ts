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
