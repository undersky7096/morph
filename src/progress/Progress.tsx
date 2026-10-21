import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { AnimatePresence, motion } from "motion/react";

import { basename } from "../formats";
import { CloseIcon, MinimizeIcon } from "../icons";
import { countStatus, CUTOUT, formatDuration, type Job, type JobFile, jobLabel, jobProgress, jobRemaining } from "../jobs";
// Barre de titre commune avec les fenêtres d'outils.
import "../tools/tools.css";
import "./Progress.css";

/** Fermeture automatique quand tout s'est bien passé (sauf si la souris est dessus). */
const AUTO_CLOSE_MS = 5000;

/**
 * Mini-fenêtre des conversions longues : progression par fichier, temps restant,
 * annulation, erreurs (réessayer, copier le détail) et doublons à trancher.
 */
export default function Progress() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [now, setNow] = useState(Date.now());
  const [hover, setHover] = useState(false);
  const win = getCurrentWebviewWindow();

  useEffect(() => {
    invoke<Job[]>("list_jobs").then((list) => setJobs(list.filter((j) => j.detached)));
    const unlisten = win.listen<Job>("job-update", ({ payload: job }) => {
      if (!job.detached) return;
      setJobs((list) => {
        const i = list.findIndex((j) => j.id === job.id);
        return i < 0 ? [...list, job] : list.map((j) => (j.id === job.id ? job : j));
      });
    });
    const tick = window.setInterval(() => setNow(Date.now()), 1000);
    return () => {
      unlisten.then((fn) => fn());
      window.clearInterval(tick);
    };
  }, []);

  const running = jobs.some((j) => !j.finished);
  const trouble = jobs.some((j) => countStatus(j, "error", "conflict") > 0);

  async function closeWindow() {
    await invoke("clear_jobs");
    await win.close();
  }

  // Tout s'est bien passé : la fenêtre se referme d'elle-même (la notification reste).
  const closeTimer = useRef(0);
  useEffect(() => {
    window.clearTimeout(closeTimer.current);
    if (jobs.length && !running && !trouble && !hover) {
      closeTimer.current = window.setTimeout(closeWindow, AUTO_CLOSE_MS);
    }
    return () => window.clearTimeout(closeTimer.current);
  }, [jobs, running, trouble, hover]);

  const ordered = [...jobs].reverse();
  const active = jobs.filter((j) => !j.finished);
  const title = !running ? "Terminé" : active.every((j) => j.target === CUTOUT) ? "Détourage en cours" : "Conversion en cours";

  return (
    <div className="progress" onPointerEnter={() => setHover(true)} onPointerLeave={() => setHover(false)}>
      <header className="titlebar" data-tauri-drag-region>
        <span className="titlebar-title" data-tauri-drag-region>
          <strong data-tauri-drag-region>{title}</strong>
        </span>
        <div className="titlebar-buttons">
          <button aria-label="Réduire" onClick={() => win.minimize()}>
            <MinimizeIcon />
          </button>
          <button
            aria-label="Fermer"
            className="is-close"
            title={running ? "Fermer (le traitement continue)" : "Fermer"}
            onClick={closeWindow}
          >
            <CloseIcon />
          </button>
        </div>
      </header>

      <main className="progress-list">
        <AnimatePresence initial={false}>
          {ordered.map((job) => (
            <motion.section
              key={job.id}
              className="job"
              layout
              initial={{ opacity: 0, y: -8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
            >
              <JobCard job={job} now={now} />
            </motion.section>
          ))}
        </AnimatePresence>
        {!jobs.length && <p className="progress-empty">Rien en cours.</p>}
      </main>
    </div>
  );
}

function JobCard({ job, now }: { job: Job; now: number }) {
  const total = job.files.length;
  const done = countStatus(job, "done");
  const conflicts = countStatus(job, "conflict");
  const errors = countStatus(job, "error");
  const remaining = jobRemaining(job, now);
  const outputs = job.files.flatMap((f) => (f.output ? [f.output] : []));
  const progress = jobProgress(job);

  let status: string;
  if (!job.finished) {
    status = `${done}/${total}${remaining !== null ? ` · encore ${formatDuration(remaining)}` : ""}`;
  } else if (errors || conflicts) {
    status = `${done}/${total} réussi${done > 1 ? "s" : ""}`;
  } else if (!done) {
    status = "Annulée";
  } else {
    status = "Terminé";
  }

  return (
    <>
      <div className="job-head">
        <span className="job-title">
          {jobLabel(job.target)}{" "}
          <small>· {total} fichier{total > 1 ? "s" : ""}</small>
        </span>
        <span className="job-status">{status}</span>
        {!job.finished && (
          <button className="job-cancel" onClick={() => invoke("cancel_job", { id: job.id })}>
            Annuler
          </button>
        )}
      </div>
      <div className={`bar ${job.finished ? (errors || conflicts ? "is-warn" : "is-done") : ""}`}>
        <span style={{ transform: `scaleX(${progress})` }} />
      </div>
      {job.note && !job.finished && <p className="job-note">{job.note}</p>}

      {conflicts > 1 && (
        <div className="job-conflicts">
          <span>{conflicts} fichiers existent déjà :</span>
          <ConflictButtons id={job.id} />
        </div>
      )}

      <ul className="files">
        {job.files.map((f, i) => (
          <FileRow key={i} id={job.id} index={i} file={f} single={conflicts <= 1} />
        ))}
      </ul>

      {job.finished && outputs.length > 0 && (
        <div className="job-actions">
          <button onClick={() => invoke("reveal", { path: outputs[0] })}>Ouvrir le dossier</button>
          <button onClick={() => invoke("open_actions", { paths: outputs })}>Éditer…</button>
          {errors > 1 && <button onClick={() => invoke("retry_job", { id: job.id })}>Tout réessayer</button>}
        </div>
      )}
    </>
  );
}

/** Remplacer / Garder les deux / Ignorer, pour un fichier (`index`) ou tous les doublons. */
function ConflictButtons({ id, index }: { id: number; index?: number }) {
  return (
    <span className="conflict-buttons">
      <button onClick={() => invoke("retry_job", { id, index, duplicates: "replace" })}>Remplacer</button>
      <button onClick={() => invoke("retry_job", { id, index, duplicates: "keep" })}>Garder les deux</button>
      <button onClick={() => invoke("skip_conflicts", { id, index })}>Ignorer</button>
    </span>
  );
}

const ICONS: Record<JobFile["status"], string> = {
  pending: "·",
  running: "",
  done: "✓",
  error: "!",
  conflict: "?",
  cancelled: "–",
  skipped: "–",
};

function FileRow({ id, index, file, single }: { id: number; index: number; file: JobFile; single: boolean }) {
  const [copied, setCopied] = useState(false);
  const name = basename(file.output ?? file.input) + (file.output || file.count < 2 ? "" : ` + ${file.count - 1}`);
  const cancellable = file.status === "pending" || file.status === "running";

  async function copy() {
    if (!file.error) return;
    await navigator.clipboard.writeText(`${file.input}\n${file.error.detail}`);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  }

  return (
    <li className={`file is-${file.status}`}>
      <span className="file-icon">{file.status === "running" ? <span className="spinner" /> : ICONS[file.status]}</span>
      <span className="file-main">
        <button
          className="file-name"
          title={file.output ? "Montrer dans l'explorateur" : file.input}
          disabled={!file.output}
          onClick={() => file.output && invoke("reveal", { path: file.output })}
        >
          {name}
        </button>
        {file.status === "running" && (
          <span className="bar is-small">
            <span style={{ transform: `scaleX(${file.progress})` }} />
          </span>
        )}
        {file.status === "error" && file.error && (
          <span className="file-error">
            {file.error.message}
            <span className="file-links">
              <button onClick={() => invoke("retry_job", { id, index })}>Réessayer</button>
              <button onClick={copy}>{copied ? "Copié ✓" : "Copier le détail"}</button>
            </span>
          </span>
        )}
        {file.status === "conflict" && file.error && (
          <span className="file-error is-conflict">
            {file.error.message}
            {single && <ConflictButtons id={id} index={index} />}
          </span>
        )}
        {file.status === "skipped" && <span className="file-note">Ignoré : le fichier existant est conservé.</span>}
        {file.status === "cancelled" && <span className="file-note">Annulé.</span>}
      </span>
      {cancellable && (
        <button className="file-cancel" aria-label="Annuler ce fichier" title="Annuler ce fichier" onClick={() => invoke("cancel_job", { id, index })}>
          ×
        </button>
      )}
    </li>
  );
}
