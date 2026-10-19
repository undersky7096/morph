import { isAudioFormat, kindOf, kindOfPath } from "../formats";
import { type ToolId, toolsFor } from "../tools/registry";
import type { Kind, TargetGroup, TargetMenu } from "../types";

/** Pétale du menu : un format cible, un sous-menu, ou une action d'édition. */
export type Item =
  | { type: "format"; format: string; kind: Kind; paths: string[] }
  | { type: "group"; label: string; kind: Kind; items: Item[]; icon?: ToolId }
  | { type: "action"; tool: ToolId; kind: Kind; paths: string[] };

const GROUP_LABELS: Record<Kind, string> = { image: "Images", audio: "Audio", video: "Vidéos" };

function formatItems(formats: string[], paths: string[]): Item[] {
  return formats.map((format) => ({ type: "format", format, kind: kindOf(format), paths }));
}

/** Pétales d'un groupe de fichiers du même type. Pour une vidéo, les formats audio
 *  (extraction du son) sont rangés dans un sous-menu pour alléger le premier niveau. */
function groupItems(group: TargetGroup): Item[] {
  if (group.kind !== "video") return formatItems(group.targets, group.paths);
  const audio = group.targets.filter(isAudioFormat);
  const others = group.targets.filter((t) => !isAudioFormat(t));
  const items = formatItems(others, group.paths);
  if (audio.length) {
    items.push({ type: "group", label: "Audio", kind: "audio", items: formatItems(audio, group.paths) });
  }
  return items;
}

/** Anneau d'actions (Crop, Compress, Edit) pour des fichiers. */
export function actionItems(paths: string[]): Item[] {
  const kind = kindOfPath(paths[0]) ?? "image";
  return toolsFor(paths).map((tool) => ({ type: "action", tool, kind, paths }));
}

export function buildItems(menu: TargetMenu): Item[] {
  const { groups, common } = menu;
  if (groups.length === 0) return [];

  if (groups.length === 1) {
    const [group] = groups;
    const items = groupItems(group);
    // Fichier déjà au bon format : accès direct aux actions, sans conversion.
    const actions = actionItems(group.paths);
    if (actions.length) items.push({ type: "group", label: "Éditer", kind: group.kind, items: actions, icon: "edit" });
    return items;
  }

  // Lot mixte : formats communs à tous + un sous-menu par type détecté.
  const allPaths = groups.flatMap((g) => g.paths);
  return [
    ...formatItems(common, allPaths),
    ...groups.map(
      (g): Item => ({ type: "group", label: GROUP_LABELS[g.kind], kind: g.kind, items: groupItems(g) }),
    ),
  ];
}
