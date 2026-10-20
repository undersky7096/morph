import type { ReactElement } from "react";
import { CompressIcon, CropIcon, EditIcon } from "../icons";
import { isCompressible, kindOfPath } from "../formats";
import type { Kind } from "../types";

export type ToolId = "crop" | "compress" | "edit";

export interface ToolInfo {
  label: string;
  icon: () => ReactElement;
  /** Types de fichiers acceptés. */
  kinds: Kind[];
  /** Lot traité d'un coup (Compress) ; sinon, un lot est traité fichier par fichier. */
  wholeBatch: boolean;
  /** Filtre plus fin que le type (formats réellement pris en charge). */
  accepts?: (path: string) => boolean;
  /** Milestone où l'outil arrive pour un type, tant qu'il n'est pas disponible. */
  comingFor?: Partial<Record<Kind, string>>;
}

export const TOOLS: Record<ToolId, ToolInfo> = {
  crop: { label: "Recadrer", icon: CropIcon, kinds: ["image"], wholeBatch: false },
  compress: {
    label: "Compresser",
    icon: CompressIcon,
    kinds: ["image", "audio", "video"],
    wholeBatch: true,
    accepts: isCompressible,
  },
  edit: {
    label: "Éditer",
    icon: EditIcon,
    kinds: ["image", "audio", "video"],
    wholeBatch: false,
    comingFor: { audio: "M6b", video: "M6c" },
  },
};

/** Outils proposés pour un ensemble de fichiers (tous doivent être pris en charge). */
export function toolsFor(paths: string[]): ToolId[] {
  const kinds = paths.map(kindOfPath);
  if (kinds.length === 0 || kinds.some((k) => k === null)) return [];
  return (Object.keys(TOOLS) as ToolId[]).filter((id) => {
    const tool = TOOLS[id];
    return (
      kinds.every((k) => tool.kinds.includes(k as Kind)) && (!tool.accepts || paths.every(tool.accepts))
    );
  });
}

/** Milestone attendue si l'outil n'est pas encore disponible pour l'un des fichiers. */
export function comingIn(tool: ToolId, paths: string[]): string | undefined {
  const coming = TOOLS[tool].comingFor;
  if (!coming) return undefined;
  for (const path of paths) {
    const kind = kindOfPath(path);
    if (kind && coming[kind]) return coming[kind];
  }
  return undefined;
}
