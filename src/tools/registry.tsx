import type { ReactElement } from "react";
import { CompressIcon, CropIcon, CutoutIcon, EditIcon, ExtractIcon, MergeIcon } from "../icons";
import { isCompressible, isEditable, isPdf, kindOfPath } from "../formats";
import { CUTOUT, EXTRACT, MERGE } from "../jobs";
import { t } from "../i18n";
import type { Kind } from "../types";

export type ToolId = "crop" | "cutout" | "compress" | "merge" | "extract" | "edit";

export interface ToolInfo {
  label: string;
  icon: () => ReactElement;
  /** Types de fichiers acceptés. */
  kinds: Kind[];
  /** Lot traité d'un coup (Compress) ; sinon, un lot est traité fichier par fichier. */
  wholeBatch: boolean;
  /** Pas de fenêtre : le traitement part aussitôt, comme une conversion, vers cette cible
   *  (Détourer, Fusionner). */
  direct?: string;
  /** Nombre minimal de fichiers (Fusionner : au moins deux). */
  minFiles?: number;
  /** Légende du ✓ quand le traitement direct a réussi. */
  done?: string;
  /** Geste attendu pour ces fichiers : proposé dès le premier niveau du menu (en plus
   *  de « Éditer › »). */
  primary?: boolean;
  /** Filtre plus fin que le type (formats réellement pris en charge). */
  accepts?: (path: string) => boolean;
  /** Milestone où l'outil arrive pour un type, tant qu'il n'est pas disponible. */
  comingFor?: Partial<Record<Kind, string>>;
}

export const TOOLS: Record<ToolId, ToolInfo> = {
  crop: { get label() { return t("Recadrer", "Crop"); }, icon: CropIcon, kinds: ["image"], wholeBatch: false, accepts: isEditable },
  cutout: {
    get label() { return t("Détourer", "Remove BG"); },
    icon: CutoutIcon,
    kinds: ["image"],
    wholeBatch: true,
    direct: CUTOUT,
    get done() { return t("Détouré", "Done"); },
  },
  merge: {
    get label() { return t("Fusionner", "Merge"); },
    icon: MergeIcon,
    kinds: ["document"],
    wholeBatch: true,
    direct: MERGE,
    minFiles: 2,
    accepts: isPdf,
    get done() { return t("Fusionné", "Merged"); },
    primary: true,
  },
  extract: {
    get label() { return t("Extraire", "Extract"); },
    icon: ExtractIcon,
    kinds: ["archive"],
    wholeBatch: true,
    direct: EXTRACT,
    get done() { return t("Extrait", "Extracted"); },
    primary: true,
  },
  compress: {
    get label() { return t("Compresser", "Compress"); },
    icon: CompressIcon,
    kinds: ["image", "audio", "video", "document"],
    wholeBatch: true,
    accepts: isCompressible,
  },
  edit: {
    get label() { return t("Éditer", "Edit"); },
    icon: EditIcon,
    kinds: ["image", "audio", "video"],
    wholeBatch: false,
    accepts: isEditable,
  },
};

/** Outils proposés pour un ensemble de fichiers (tous doivent être pris en charge). */
export function toolsFor(paths: string[]): ToolId[] {
  const kinds = paths.map(kindOfPath);
  if (kinds.length === 0 || kinds.some((k) => k === null)) return [];
  return (Object.keys(TOOLS) as ToolId[]).filter((id) => {
    const tool = TOOLS[id];
    return (
      kinds.every((k) => tool.kinds.includes(k as Kind)) &&
      (!tool.accepts || paths.every(tool.accepts)) &&
      paths.length >= (tool.minFiles ?? 1)
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
