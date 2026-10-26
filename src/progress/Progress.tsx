import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { AnimatePresence, motion } from "motion/react";

import { getVersion } from "@tauri-apps/api/app";

import { accountErrorText, callAccount, useAccount } from "../account";
import { basename } from "../formats";
import { plural, t } from "../i18n";
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
  const title = !running
    ? t("progress.done")
    : active.every((j) => j.target === CUTOUT)
      ? t("progress.removing_background")
      : t("progress.converting");

  return (
    <div className="progress" onPointerEnter={() => setHover(true)} onPointerLeave={() => setHover(false)}>
      <header className="titlebar" data-tauri-drag-region>
        <span className="titlebar-title" data-tauri-drag-region>
          <strong data-tauri-drag-region>{title}</strong>
        </span>
        <div className="titlebar-buttons">
          <button aria-label={t("progress.minimize")} onClick={() => win.minimize()}>
            <MinimizeIcon />
          </button>
          <button
            aria-label={t("progress.close")}
            className="is-close"
            title={running ? t("progress.close_processing_continues") : t("progress.close")}
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
        {!jobs.length && <p className="progress-empty">{t("progress.nothing_progress")}</p>}
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
    status = `${done}/${total}${remaining !== null ? ` · ${t("progress.time_left", { time: formatDuration(remaining) })}` : ""}`;
  } else if (errors || conflicts) {
    status = `${done}/${total} ${plural(done, "progress.succeeded")}`;
  } else if (!done) {
    status = t("progress.cancelled");
  } else {
    status = t("progress.done");
  }

  return (
    <>
      <div className="job-head">
        <span className="job-title">
          {jobLabel(job.target)}{" "}
          <small>
            · {plural(total, "common.files")}
          </small>
        </span>
        <span className="job-status">{status}</span>
        {!job.finished && (
          <button className="job-cancel" onClick={() => invoke("cancel_job", { id: job.id })}>
            {t("progress.cancel")}
          </button>
        )}
      </div>
      <div className={`bar ${job.finished ? (errors || conflicts ? "is-warn" : "is-done") : ""}`}>
        <span style={{ transform: `scaleX(${progress})` }} />
      </div>
      {job.note && !job.finished && <p className="job-note">{job.note}</p>}

      {conflicts > 1 && (
        <div className="job-conflicts">
          <span>{t("progress.files_already_exist", { n: conflicts })}</span>
          <ConflictButtons id={job.id} />
        </div>
      )}

      <ul className="files">
        {job.files.map((f, i) => (
          <FileRow key={i} id={job.id} index={i} file={f} single={conflicts <= 1} target={job.target} />
        ))}
      </ul>

      {job.finished && outputs.length > 0 && (
        <div className="job-actions">
          <button onClick={() => invoke("reveal", { path: outputs[0] })}>{t("progress.open_folder")}</button>
          <button onClick={() => invoke("open_actions", { paths: outputs })}>{t("progress.edit")}</button>
          {errors > 1 && <button onClick={() => invoke("retry_job", { id: job.id })}>{t("progress.retry_all")}</button>}
        </div>
      )}
    </>
  );
}

/** Remplacer / Garder les deux / Ignorer, pour un fichier (`index`) ou tous les doublons. */
function ConflictButtons({ id, index }: { id: number; index?: number }) {
  return (
    <span className="conflict-buttons">
      <button onClick={() => invoke("retry_job", { id, index, duplicates: "replace" })}>{t("progress.replace")}</button>
      <button onClick={() => invoke("retry_job", { id, index, duplicates: "keep" })}>{t("progress.keep_both")}</button>
      <button onClick={() => invoke("skip_conflicts", { id, index })}>{t("progress.skip")}</button>
    </span>
  );
}

/** Signalement d'une erreur au développeur (compte Morph) : on montre exactement ce qui part
 *  (sans dossier personnel ni nom d'utilisateur), jamais le fichier. */
function ReportForm({ file, target, onClose }: { file: JobFile; target: string; onClose: () => void }) {
  const [detail, setDetail] = useState<string | null>(null);
  const [comment, setComment] = useState("");
  const [state, setState] = useState<"idle" | "sending" | "sent">("idle");
  const [error, setError] = useState<string | null>(null);
  const inputExt = file.input.includes(".") ? file.input.split(".").pop()!.toLowerCase().slice(0, 12) : "";

  useEffect(() => {
    invoke<string>("account_anonymize", { text: `${basename(file.input)}\n${file.error?.detail ?? ""}` }).then(setDetail);
  }, [file]);

  async function send() {
    setState("sending");
    setError(null);
    try {
      const [version, os] = await Promise.all([getVersion(), invoke<string>("platform")]);
      await callAccount("POST", "report", { code: file.error?.code, message: file.error?.message, detail, target, inputExt, version, os, comment });
      setState("sent");
    } catch (e) {
      setError(accountErrorText(e));
      setState("idle");
    }
  }

  if (state === "sent") return <span className="report-sent">{t("progress.report_sent")}</span>;
  return (
    <span className="report-form">
      <textarea
        rows={2}
        maxLength={1000}
        value={comment}
        placeholder={t("progress.report_comment")}
        aria-label={t("progress.report_comment")}
        onChange={(e) => setComment(e.target.value)}
      />
      <details>
        <summary>{t("progress.report_what")}</summary>
        <pre>{detail ?? "…"}</pre>
      </details>
      <span className="file-links">
        <button onClick={send} disabled={state === "sending" || detail === null}>
          {state === "sending" ? t("progress.report_sending") : t("progress.report_send")}
        </button>
        <button onClick={onClose}>{t("progress.cancel")}</button>
      </span>
      {error && <span className="report-error">{error}</span>}
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

function FileRow({ id, index, file, single, target }: { id: number; index: number; file: JobFile; single: boolean; target: string }) {
  const [copied, setCopied] = useState(false);
  const [reporting, setReporting] = useState(false);
  const account = useAccount();
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
          title={file.output ? t("progress.show_folder") : file.input}
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
              <button onClick={() => invoke("retry_job", { id, index })}>{t("progress.retry")}</button>
              <button onClick={copy}>{copied ? t("progress.copied") : t("progress.copy_details")}</button>
              {account?.user && !reporting && <button onClick={() => setReporting(true)}>{t("progress.report")}</button>}
            </span>
            {reporting && <ReportForm file={file} target={target} onClose={() => setReporting(false)} />}
          </span>
        )}
        {file.status === "conflict" && file.error && (
          <span className="file-error is-conflict">
            {file.error.message}
            {single && <ConflictButtons id={id} index={index} />}
          </span>
        )}
        {file.status === "skipped" && (
          <span className="file-note">{t("progress.skipped_existing_file_kept")}</span>
        )}
        {file.status === "cancelled" && <span className="file-note">{t("progress.cancelled_2")}</span>}
      </span>
      {cancellable && (
        <button
          className="file-cancel"
          aria-label={t("progress.cancel_file")}
          title={t("progress.cancel_file")}
          onClick={() => invoke("cancel_job", { id, index })}
        >
          ×
        </button>
      )}
    </li>
  );
}
