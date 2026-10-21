// Langue de l'interface : français ou anglais, choisie dans les Réglages (« Système » suit
// la langue de l'ordinateur). Les textes restent écrits dans le code, en paires :
// t("Enregistrer", "Save").

import { getSettings } from "./settings";

export type Language = "system" | "fr" | "en";

/** Langue effective de l'interface. */
export function lang(): "fr" | "en" {
  const chosen = getSettings().language;
  if (chosen === "fr" || chosen === "en") return chosen;
  return navigator.languages.some((l) => l.toLowerCase().startsWith("fr")) ? "fr" : "en";
}

/** Texte dans la langue de l'interface. */
export const t = (fr: string, en: string) => (lang() === "fr" ? fr : en);

/** Pluriel simple : `plural(n, "fichier", "fichiers", "file", "files")` → « 3 fichiers ». */
export function plural(n: number, frOne: string, frMany: string, enOne: string, enMany: string) {
  return `${n.toLocaleString(locale())} ${lang() === "fr" ? (n > 1 ? frMany : frOne) : n === 1 ? enOne : enMany}`;
}

/** Locale pour les nombres et les dates. */
export const locale = () => (lang() === "fr" ? "fr-FR" : "en-GB");
