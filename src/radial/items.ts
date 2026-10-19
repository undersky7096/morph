import type { Kind, TargetGroup, TargetMenu } from "../types";

/** Pétale du menu : soit un format cible, soit un sous-menu. */
export type Item =
  | { type: "format"; format: string; kind: Kind; paths: string[] }
  | { type: "group"; label: string; kind: Kind; items: Item[] };

const AUDIO = new Set(["mp3", "wav", "ogg", "flac", "m4a", "opus"]);
const VIDEO = new Set(["mp4", "webm", "mkv", "mov", "avi"]);

const GROUP_LABELS: Record<Kind, string> = { image: "Images", audio: "Audio", video: "Vidéos" };

/** Type du format de sortie (pour la couleur du pétale). */
export function kindOf(format: string): Kind {
  if (AUDIO.has(format)) return "audio";
  if (VIDEO.has(format)) return "video";
  return "image";
}

function formatItems(formats: string[], paths: string[]): Item[] {
  return formats.map((format) => ({ type: "format", format, kind: kindOf(format), paths }));
}

/** Pétales d'un groupe de fichiers du même type. Pour une vidéo, les formats audio
 *  (extraction du son) sont rangés dans un sous-menu pour alléger le premier niveau. */
function groupItems(group: TargetGroup): Item[] {
  if (group.kind !== "video") return formatItems(group.targets, group.paths);
  const audio = group.targets.filter((t) => AUDIO.has(t));
  const others = group.targets.filter((t) => !AUDIO.has(t));
  const items = formatItems(others, group.paths);
  if (audio.length) {
    items.push({ type: "group", label: "Audio", kind: "audio", items: formatItems(audio, group.paths) });
  }
  return items;
}

export function buildItems(menu: TargetMenu): Item[] {
  const { groups, common } = menu;
  if (groups.length === 0) return [];
  if (groups.length === 1) return groupItems(groups[0]);

  // Lot mixte : formats communs à tous + un sous-menu par type détecté.
  const allPaths = groups.flatMap((g) => g.paths);
  return [
    ...formatItems(common, allPaths),
    ...groups.map(
      (g): Item => ({ type: "group", label: GROUP_LABELS[g.kind], kind: g.kind, items: groupItems(g) }),
    ),
  ];
}
