import { isAudioFormat, kindOf, kindOfPath } from "../formats";
import type { Settings } from "../settings";
import { type ToolId, toolsFor } from "../tools/registry";
import type { Blocked, Kind, TargetGroup, TargetMenu } from "../types";

/** Pétale du menu : un format cible (grisé avec la raison s'il est indisponible), un
 *  sous-menu, ou une action d'édition. */
export type Item =
  | { type: "format"; format: string; kind: Kind; paths: string[]; blocked?: string; link?: string | null }
  | { type: "group"; label: string; kind: Kind; items: Item[]; icon?: ToolId }
  | { type: "action"; tool: ToolId; kind: Kind; paths: string[] };

const GROUP_LABELS: Record<Kind, string> = { image: "Images", audio: "Audio", video: "Vidéos", document: "Documents" };

/** Pétales par anneau au-delà desquels les derniers formats passent dans « Plus › ». */
const MAX_PETALS = 9;

/** Formats favoris d'abord (dans l'ordre choisi), puis les autres dans l'ordre habituel. */
function byFavorites(formats: string[], favorites: string[]): string[] {
  const rank = (f: string) => {
    const i = favorites.indexOf(f);
    return i < 0 ? favorites.length : i;
  };
  return formats.map((f, i) => ({ f, i })).sort((a, b) => rank(a.f) - rank(b.f) || a.i - b.i).map(({ f }) => f);
}

function formatItems(formats: string[], paths: string[], favorites: string[], blocked: Blocked[] = []): Item[] {
  return [
    ...byFavorites(formats, favorites).map((format): Item => ({ type: "format", format, kind: kindOf(format), paths })),
    ...blocked.map(
      (b): Item => ({ type: "format", format: b.format, kind: kindOf(b.format), paths, blocked: b.reason, link: b.link }),
    ),
  ];
}

/** Anneau trop chargé : les derniers formats (favoris et courants d'abord) passent dans un
 *  sous-menu « Plus », placé après les formats gardés et avant les autres sous-menus. */
function fit(items: Item[], kind: Kind): Item[] {
  if (items.length <= MAX_PETALS) return items;
  const formats = items.filter((i) => i.type === "format");
  const others = items.filter((i) => i.type !== "format");
  const keep = Math.max(MAX_PETALS - 1 - others.length, 1);
  const more: Item = { type: "group", label: "Plus", kind, items: formats.slice(keep) };
  return [...formats.slice(0, keep), more, ...others];
}

/** Pétales d'un groupe de fichiers du même type. Pour une vidéo, les formats audio
 *  (extraction du son) sont rangés dans un sous-menu pour alléger le premier niveau. */
function groupItems(group: TargetGroup, favorites: string[]): Item[] {
  if (group.kind !== "video") return formatItems(group.targets, group.paths, favorites, group.blocked);
  const audio = group.targets.filter(isAudioFormat);
  const items = formatItems(
    group.targets.filter((t) => !isAudioFormat(t)),
    group.paths,
    favorites,
    group.blocked.filter((b) => !isAudioFormat(b.format)),
  );
  const audioBlocked = group.blocked.filter((b) => isAudioFormat(b.format));
  if (audio.length || audioBlocked.length) {
    const audioItems = fit(formatItems(audio, group.paths, favorites, audioBlocked), "audio");
    items.push({ type: "group", label: "Audio", kind: "audio", items: audioItems });
  }
  return items;
}

/** Anneau d'actions (Crop, Compress, Edit) pour des fichiers. */
export function actionItems(paths: string[]): Item[] {
  const kind = kindOfPath(paths[0]) ?? "image";
  return toolsFor(paths).map((tool) => ({ type: "action", tool, kind, paths }));
}

export function buildItems(menu: TargetMenu, settings: Settings): Item[] {
  const { groups, common } = menu;
  if (groups.length === 0) return [];

  if (groups.length === 1) {
    const [group] = groups;
    const items = groupItems(group, settings.favorites);
    // Fichier déjà au bon format : accès direct aux actions, sans conversion.
    const actions = actionItems(group.paths);
    // Plusieurs PDF : les fusionner est le geste attendu, proposé dès le premier niveau.
    const merge = actions.find((a) => a.type === "action" && a.tool === "merge");
    if (merge) items.unshift(merge);
    if (actions.length) items.push({ type: "group", label: "Éditer", kind: group.kind, items: actions, icon: "edit" });
    return fit(items, group.kind);
  }

  // Lot mixte : formats communs à tous + un sous-menu par type détecté
  // (ou, selon les réglages, les formats communs seulement quand il y en a).
  const allPaths = groups.flatMap((g) => g.paths);
  if (settings.mixedBatch === "common" && common.length) return formatItems(common, allPaths, settings.favorites);
  const kind = groups[0].kind;
  return fit(
    [
      ...formatItems(common, allPaths, settings.favorites),
      ...groups.map(
        (g): Item => ({
          type: "group",
          label: GROUP_LABELS[g.kind],
          kind: g.kind,
          items: fit(groupItems(g, settings.favorites), g.kind),
        }),
      ),
    ],
    kind,
  );
}
