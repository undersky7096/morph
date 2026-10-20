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
  /** Applicable à un lot de plusieurs fichiers. */
  batch: boolean;
  /** Filtre plus fin que le type (formats réellement pris en charge). */
  accepts?: (path: string) => boolean;
  /** Milestone où l'outil arrive, tant qu'il n'est pas disponible. */
  comingIn?: string;
}

export const TOOLS: Record<ToolId, ToolInfo> = {
  crop: { label: "Recadrer", icon: CropIcon, kinds: ["image"], batch: false },
  compress: {
    label: "Compresser",
    icon: CompressIcon,
    kinds: ["image", "audio", "video"],
    batch: true,
    accepts: isCompressible,
  },
  edit: {
    label: "Éditer",
    icon: EditIcon,
    kinds: ["image", "audio", "video"],
    batch: false,
    comingIn: "M6",
  },
};

/** Outils proposés pour un ensemble de fichiers (un seul : selon son type ; un lot : outils de lot). */
export function toolsFor(paths: string[]): ToolId[] {
  const kinds = paths.map(kindOfPath);
  if (kinds.length === 0 || kinds.some((k) => k === null)) return [];
  return (Object.keys(TOOLS) as ToolId[]).filter((id) => {
    const tool = TOOLS[id];
    if (paths.length > 1 && !tool.batch) return false;
    return (
      kinds.every((k) => tool.kinds.includes(k as Kind)) && (!tool.accepts || paths.every(tool.accepts))
    );
  });
}
