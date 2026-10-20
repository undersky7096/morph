import { isAudioFormat, kindOf, kindOfPath } from "../formats";
import type { Settings } from "../settings";
import { type ToolId, toolsFor } from "../tools/registry";
import type { Kind, TargetGroup, TargetMenu } from "../types";

/** Pétale du menu : un format cible, un sous-menu, une action d'édition,
 *  ou le retour aux formats de conversion depuis l'anneau d'actions. */
export type Item =
  | { type: "format"; format: string; kind: Kind; paths: string[] }
  | { type: "group"; label: string; kind: Kind; items: Item[]; icon?: ToolId }
  | { type: "action"; tool: ToolId; kind: Kind; paths: string[] }
  | { type: "convert"; kind: Kind; paths: string[] };

const GROUP_LABELS: Record<Kind, string> = { image: "Images", audio: "Audio", video: "Vidéos" };

/** Formats favoris d'abord (dans l'ordre choisi), puis les autres dans l'ordre habituel. */
function byFavorites(formats: string[], favorites: string[]): string[] {
  const rank = (f: string) => {
    const i = favorites.indexOf(f);
    return i < 0 ? favorites.length : i;
  };
  return formats.map((f, i) => ({ f, i })).sort((a, b) => rank(a.f) - rank(b.f) || a.i - b.i).map(({ f }) => f);
}

function formatItems(formats: string[], paths: string[], favorites: string[]): Item[] {
  return byFavorites(formats, favorites).map((format) => ({ type: "format", format, kind: kindOf(format), paths }));
}

/** Pétales d'un groupe de fichiers du même type. Pour une vidéo, les formats audio
 *  (extraction du son) sont rangés dans un sous-menu pour alléger le premier niveau. */
function groupItems(group: TargetGroup, favorites: string[]): Item[] {
  if (group.kind !== "video") return formatItems(group.targets, group.paths, favorites);
  const audio = group.targets.filter(isAudioFormat);
  const others = group.targets.filter((t) => !isAudioFormat(t));
  const items = formatItems(others, group.paths, favorites);
  if (audio.length) {
    items.push({ type: "group", label: "Audio", kind: "audio", items: formatItems(audio, group.paths, favorites) });
  }
  return items;
}

/** Anneau d'actions (Crop, Compress, Edit, puis Convertir) pour des fichiers ; vide si aucun outil. */
export function actionItems(paths: string[]): Item[] {
  const kind = kindOfPath(paths[0]) ?? "image";
  const tools = toolsFor(paths).map((tool): Item => ({ type: "action", tool, kind, paths }));
  return tools.length ? [...tools, { type: "convert", kind, paths }] : [];
}

export function buildItems(menu: TargetMenu, settings: Settings): Item[] {
  const { groups, common } = menu;
  if (groups.length === 0) return [];

  if (groups.length === 1) {
    const [group] = groups;
    const items = groupItems(group, settings.favorites);
    // Fichier déjà au bon format : accès direct aux actions, sans conversion.
    const actions = actionItems(group.paths);
    if (actions.length) items.push({ type: "group", label: "Éditer", kind: group.kind, items: actions, icon: "edit" });
    return items;
  }

  // Lot mixte : formats communs à tous + un sous-menu par type détecté
  // (ou, selon les réglages, les formats communs seulement quand il y en a).
  const allPaths = groups.flatMap((g) => g.paths);
  if (settings.mixedBatch === "common" && common.length) return formatItems(common, allPaths, settings.favorites);
  return [
    ...formatItems(common, allPaths, settings.favorites),
    ...groups.map(
      (g): Item => ({ type: "group", label: GROUP_LABELS[g.kind], kind: g.kind, items: groupItems(g, settings.favorites) }),
    ),
  ];
}
