// Erreurs renvoyées par le cœur de Morph (miroir de `convert::ErrorInfo`).

export interface AppError {
  /** « exists », « cancelled », « denied », « disk-full », « unreadable »… */
  code: string;
  /** Message clair, à afficher. */
  message: string;
  /** Détail technique, à copier pour un rapport de bug. */
  detail: string;
  /** Fichier concerné (doublon). */
  path?: string | null;
}

/** Toute erreur (du cœur, ou de l'interface elle-même) sous la forme d'un `AppError`. */
export function describe(e: unknown): AppError {
  if (e && typeof e === "object" && "message" in e && "code" in e) return e as AppError;
  const text = e instanceof Error ? e.message : String(e);
  return { code: "failed", message: text, detail: text };
}

export const isCancelled = (e: unknown) => describe(e).code === "cancelled";
export const isConflict = (e: unknown) => describe(e).code === "exists";

/** Identifiant d'annulation d'une opération d'outil (voir `cancel_task`). */
export const newTask = () => `task-${Date.now()}-${Math.random().toString(36).slice(2)}`;
