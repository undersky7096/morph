import { isAudioFormat, kindOf, kindOfPath } from "../formats";
import { t } from "../i18n";
import { ARCHIVE_7Z, ARCHIVE_ZIP } from "../jobs";
import type { Settings } from "../settings";
import { TOOLS, type ToolId, toolsFor } from "../tools/registry";
import type { Blocked, Kind, TargetGroup, TargetMenu } from "../types";

/** Pétale du menu : un format cible (grisé avec la raison s'il est indisponible), un
 *  sous-menu, ou une action d'édition. */
export type Item =
  | {
      type: "format";
      format: string;
      kind: Kind;
      paths: string[];
      blocked?: string;
      link?: string | null;
      /** Cible envoyée au moteur, si ce n'est pas le format (archiver la sélection). */
      target?: string;
    }
  | { type: "group"; label: string; kind: Kind; items: Item[]; icon?: ToolId }
  | { type: "action"; tool: ToolId; kind: Kind; paths: string[] };

const groupLabel = (kind: Kind) =>
  ({
    image: t("radial.images"),
    audio: t("radial.audio"),
    video: t("radial.videos"),
    document: t("radial.documents"),
    archive: t("radial.archives"),
  })[kind];

/** « Archiver › » : une archive ZIP ou 7Z de toute la sélection (fichiers et dossiers). */
function archiveItems(paths: string[]): Item[] {
  return [
    { type: "format", format: "zip", target: ARCHIVE_ZIP, kind: "archive", paths },
    { type: "format", format: "7z", target: ARCHIVE_7Z, kind: "archive", paths },
  ];
}

const archiveGroup = (paths: string[]): Item => ({ type: "group", label: t("radial.archive"), kind: "archive", items: archiveItems(paths) });

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
  const more: Item = { type: "group", label: t("radial.more"), kind, items: formats.slice(keep) };
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
    items.push({ type: "group", label: t("radial.audio"), kind: "audio", items: audioItems });
  }
  return items;
}

/** Anneau d'actions (Crop, Compress, Edit) pour des fichiers. */
export function actionItems(paths: string[]): Item[] {
  const kind = kindOfPath(paths[0]) ?? "image";
  return toolsFor(paths).map((tool) => ({ type: "action", tool, kind, paths }));
}

export function buildItems(menu: TargetMenu, settings: Settings): Item[] {
  const { groups, common, folders } = menu;
  // Dossiers seuls : ils s'archivent, directement au premier niveau.
  if (groups.length === 0) return folders.length ? archiveItems(folders) : [];

  if (groups.length === 1 && folders.length === 0) {
    const [group] = groups;
    const items = groupItems(group, settings.favorites);
    // Fichier déjà au bon format : accès direct aux actions, sans conversion.
    const actions = actionItems(group.paths);
    // Geste attendu (fusionner des PDF, extraire une archive) : dès le premier niveau.
    items.unshift(...actions.filter((a) => a.type === "action" && TOOLS[a.tool].primary));
    if (actions.length) items.push({ type: "group", label: t("radial.edit"), kind: group.kind, items: actions, icon: "edit" });
    // Toute sélection s'archive (sauf des archives, qui changent plutôt de format).
    if (group.kind !== "archive") items.push(archiveGroup(group.paths));
    return fit(items, group.kind);
  }

  // Lot mixte (ou fichiers et dossiers) : formats communs à tous + un sous-menu par type
  // détecté (ou, selon les réglages, les formats communs seulement quand il y en a), et
  // « Archiver › » pour tout le lot.
  const allPaths = groups.flatMap((g) => g.paths);
  if (settings.mixedBatch === "common" && common.length && !folders.length) {
    return formatItems(common, allPaths, settings.favorites);
  }
  const kind = groups[0].kind;
  // Un dossier ne se convertit pas : aucun format n'est commun à tout le lot.
  const shared = folders.length ? [] : common;
  return fit(
    [
      ...formatItems(shared, allPaths, settings.favorites),
      ...groups.map(
        (g): Item => ({
          type: "group",
          label: groupLabel(g.kind),
          kind: g.kind,
          items: fit(groupItems(g, settings.favorites), g.kind),
        }),
      ),
      archiveGroup([...allPaths, ...folders]),
    ],
    kind,
  );
}
