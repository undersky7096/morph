// Conversions en arrière-plan (miroir de `src-tauri/src/jobs.rs`).

import type { AppError } from "./errors";

export type FileStatus = "pending" | "running" | "done" | "error" | "conflict" | "cancelled" | "skipped";

export interface JobFile {
  input: string;
  status: FileStatus;
  progress: number;
  output: string | null;
  error: AppError | null;
}

export interface Job {
  id: number;
  target: string;
  files: JobFile[];
  startedMs: number;
  finished: boolean;
  detached: boolean;
}

/** Fichiers retirés du lot (annulés, ignorés) : ils ne comptent pas dans l'avancement. */
const dropped = (s: FileStatus) => s === "cancelled" || s === "skipped";

/** Avancement global, 0…1 : fichiers traités + part du fichier en cours, parmi ceux qui
 *  restent dans le lot (annuler un fichier ne fait pas bondir la barre ni l'estimation). */
export function jobProgress(job: Job): number {
  const kept = job.files.filter((f) => !dropped(f.status));
  if (!kept.length) return 1;
  const sum = kept.reduce((n, f) => n + (f.status === "running" ? f.progress : f.status === "pending" ? 0 : 1), 0);
  return sum / kept.length;
}

/** Temps restant estimé en secondes, d'après le rythme depuis le début (null si trop tôt). */
export function jobRemaining(job: Job, now = Date.now()): number | null {
  const p = jobProgress(job);
  const elapsed = (now - job.startedMs) / 1000;
  if (job.finished || p < 0.03 || elapsed < 1.5) return null;
  return (elapsed * (1 - p)) / p;
}

export const countStatus = (job: Job, ...statuses: FileStatus[]) =>
  job.files.filter((f) => statuses.includes(f.status)).length;

/** « 2 min 05 », « 45 s ». */
export function formatDuration(secs: number): string {
  const s = Math.max(1, Math.round(secs));
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ${String(s % 60).padStart(2, "0")}`;
  return `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, "0")}`;
}
