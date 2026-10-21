// Enregistrement depuis un outil : dossier (réglage « demander »), annulation, doublon à
// trancher. Partagé par Recadrer, Compresser et les éditeurs.

import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { type AppError, describe, newTask } from "../errors";
import { chooseOutputDir } from "../settings";
import { t } from "../i18n";

export type Duplicates = "keep" | "replace";

/** Options transmises au cœur (miroir de `commands::RunOptions`). */
export interface RunOptions {
  outDir?: string;
  duplicates?: Duplicates;
  task: string;
}

interface Conflict {
  message: string;
  retry: (choice: Duplicates) => void;
}

export interface SaveFlow {
  /** Prépare une opération : `null` si l'utilisateur renonce (choix du dossier annulé).
   *  `duplicates` : réessai après un doublon (le dossier déjà choisi est réutilisé). */
  begin: (duplicates?: Duplicates) => Promise<RunOptions | null>;
  /** Erreur de l'opération : doublon → proposition, annulation → note discrète ;
   *  renvoie l'erreur à afficher, ou `null` si elle est déjà prise en charge. */
  fail: (e: unknown, retry: (choice: Duplicates) => void) => AppError | null;
  /** Fin de l'opération (réussie ou non). */
  end: () => void;
  cancel: () => void;
  running: boolean;
  conflict: Conflict | null;
  dismissConflict: () => void;
  /** Propose de trancher un doublon (lot : plusieurs fichiers d'un coup). */
  ask: (message: string, retry: (choice: Duplicates) => void) => void;
  notice: string | null;
  /** Dernière erreur, dont le détail peut être copié. */
  failure: AppError | null;
}

export function useSaveFlow(): SaveFlow {
  const [conflict, setConflict] = useState<Conflict | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [failure, setFailure] = useState<AppError | null>(null);
  const task = useRef<string | null>(null);
  const outDir = useRef<string | undefined>(undefined);

  return {
    async begin(duplicates) {
      setConflict(null);
      setNotice(null);
      setFailure(null);
      if (!duplicates) {
        const dir = await chooseOutputDir();
        if (dir === null) return null;
        outDir.current = dir;
      }
      task.current = newTask();
      setRunning(true);
      return { outDir: outDir.current, duplicates, task: task.current };
    },
    fail(e, retry) {
      const err = describe(e);
      if (err.code === "exists") {
        setConflict({ message: err.message, retry });
        return null;
      }
      if (err.code === "cancelled") {
        setNotice(t("Enregistrement annulé.", "Save cancelled."));
        return null;
      }
      setFailure(err);
      return err;
    },
    end() {
      task.current = null;
      setRunning(false);
    },
    cancel() {
      if (task.current) invoke("cancel_task", { task: task.current });
    },
    running,
    conflict,
    dismissConflict: () => setConflict(null),
    ask: (message, retry) => setConflict({ message, retry }),
    notice,
    failure,
  };
}

/** Pied de fenêtre : « Annuler » pendant l'enregistrement, ou choix pour un doublon. */
export function SaveExtras({ flow }: { flow: SaveFlow }) {
  if (flow.conflict) {
    const { message, retry } = flow.conflict;
    return (
      <span className="save-conflict">
        <span>{message}</span>
        <button onClick={() => retry("replace")}>{t("Remplacer", "Replace")}</button>
        <button onClick={() => retry("keep")}>{t("Garder les deux", "Keep both")}</button>
        <button onClick={flow.dismissConflict}>{t("Annuler", "Cancel")}</button>
      </span>
    );
  }
  if (flow.running) return <button onClick={flow.cancel}>{t("Annuler", "Cancel")}</button>;
  if (flow.notice) return <span className="save-notice">{flow.notice}</span>;
  if (flow.failure) return <CopyDetail error={flow.failure} />;
  return null;
}

/** Copie le détail technique d'une erreur (pour un rapport de bug). */
export function CopyDetail({ error, className = "" }: { error: AppError; className?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      className={className}
      title={error.detail}
      onClick={async () => {
        await navigator.clipboard.writeText(error.detail);
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
      }}
    >
      {copied ? t("Copié ✓", "Copied ✓") : t("Copier le détail", "Copy details")}
    </button>
  );
}
