// Langue de l'interface, choisie dans les Réglages (« Système » suit la langue de
// l'ordinateur). Les textes vivent dans les catalogues `locales/<code>.json`, partagés avec
// le cœur Rust (`src-tauri/src/i18n.rs`) : t("settings.save") → « Enregistrer ».
//
// Ajouter une langue = ajouter `locales/<code>.json` (même clés, `_meta.name` et
// `_meta.locale`) : elle apparaît d'elle-même dans les Réglages. Une clé absente d'un
// catalogue retombe sur l'anglais, puis sur le français.

import { getSettings } from "./settings";

/** Texte simple, ou formes du pluriel (catégories `Intl.PluralRules` : one, other…). */
type Entry = string | Partial<Record<Intl.LDMLPluralRule, string>>;
type Catalog = Record<string, Entry> & { _meta: { name: string; locale: string } };

const CATALOGS: Record<string, Catalog> = Object.fromEntries(
  Object.entries(import.meta.glob<Catalog>("../locales/*.json", { eager: true, import: "default" })).map(([file, catalog]) => [
    file.replace(/^.*\/(\w+)\.json$/, "$1"),
    catalog,
  ]),
);

/** Langue de repli quand l'ordinateur parle une langue sans catalogue. */
const FALLBACK = "en";
/** Langue d'écriture du projet : la plus complète. */
const SOURCE = "fr";

/** « system » ou le code d'une langue disponible (« fr », « en »…). */
export type Language = string;

/** Langues disponibles, dans l'ordre alphabétique de leur nom. */
export const LANGUAGES: { code: string; name: string }[] = Object.entries(CATALOGS)
  .map(([code, c]) => ({ code, name: c._meta.name }))
  .sort((a, b) => a.name.localeCompare(b.name));

/** Langue de l'ordinateur, si un catalogue existe pour elle. */
function systemLanguage(): string {
  for (const l of navigator.languages) {
    const code = l.toLowerCase().split("-")[0];
    if (code in CATALOGS) return code;
  }
  return FALLBACK;
}

/** Langue effective de l'interface. */
export function lang(): string {
  const chosen = getSettings().language;
  return chosen in CATALOGS ? chosen : systemLanguage();
}

/** Locale pour les nombres et les dates (« fr-FR », « en-GB »). */
export const locale = () => CATALOGS[lang()]._meta.locale;

function entry(key: string): Entry | undefined {
  return CATALOGS[lang()][key] ?? CATALOGS[FALLBACK]?.[key] ?? CATALOGS[SOURCE]?.[key];
}

type Params = Record<string, string | number>;

/** Remplace `{nom}` par la valeur ; les nombres sont écrits à la façon de la langue. */
function fill(text: string, params?: Params) {
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (all, name: string) => {
    const v = params[name];
    return v === undefined ? all : typeof v === "number" ? v.toLocaleString(locale()) : v;
  });
}

/** Texte dans la langue de l'interface : `t("progress.time_left", { time: "2 min" })`. */
export function t(key: string, params?: Params): string {
  const e = entry(key);
  if (e === undefined) return key;
  return fill(typeof e === "string" ? e : (e.other ?? ""), params);
}

/** Pluriel selon la langue : `plural(3, "common.files")` → « 3 fichiers ». `{n}` = le nombre. */
export function plural(n: number, key: string, params?: Params): string {
  const e = entry(key);
  if (e === undefined) return key;
  if (typeof e === "string") return fill(e, { n, ...params });
  const form = e[new Intl.PluralRules(locale()).select(n)] ?? e.other ?? "";
  return fill(form, { n, ...params });
}
