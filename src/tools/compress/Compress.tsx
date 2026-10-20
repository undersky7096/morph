import { type ReactNode, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { basename, formatBytes, hasBitrate, isPng, kindOfPath } from "../../formats";
import type { ConvertProgress } from "../../types";
import "./Compress.css";

type Level = "light" | "medium" | "strong";
type Mode = "quality" | "target";

// Miroir de `edit::compress::{Options, Outcome}`.
interface Options {
  level: Level;
  reduceColors: boolean;
  targetMb: number | null;
}

type Outcome =
  | { status: "saved"; output: string; before: number; after: number }
  | { status: "noGain"; before: number };

interface CompressResult {
  input: string;
  outcome: Outcome | null;
  error: string | null;
}

const LEVELS: { label: string; value: Level; hint: string }[] = [
  { label: "Léger", value: "light", hint: "Différence quasi invisible." },
  { label: "Moyen", value: "medium", hint: "Bon compromis poids / qualité." },
  { label: "Fort", value: "strong", hint: "Poids minimal, perte visible." },
];

/** Limite d'envoi de fichiers sur Discord (compte gratuit). */
const DISCORD_MB = 20;

interface Props {
  paths: string[];
  /** Appelé avec les fichiers compressés une fois enregistrés. */
  onSaved: (outputs: string[]) => void;
  /** Bouton « Outil suivant », fourni par la fenêtre. */
  nextTool: ReactNode;
}

const percentLess = (before: number, after: number) => `−${Math.round((1 - after / before) * 100)} %`;

export default function Compress({ paths: initialPaths, onSaved, nextTool }: Props) {
  // Les fichiers d'origine : la fenêtre passe aux résultats après l'enregistrement.
  const [paths] = useState(initialPaths);
  const [level, setLevel] = useState<Level>("medium");
  const [reduceColors, setReduceColors] = useState(false);
  const [mode, setMode] = useState<Mode>("quality");
  const [targetText, setTargetText] = useState(String(DISCORD_MB));
  const [sizes, setSizes] = useState<number[]>([]);
  /** Progression par fichier pendant la compression, null sinon. */
  const [progress, setProgress] = useState<number[] | null>(null);
  const [results, setResults] = useState<CompressResult[]>([]);
  const [error, setError] = useState<string | null>(null);

  const single = paths.length === 1 ? paths[0] : null;
  const previewPath = single && kindOfPath(single) === "image" ? single : null;
  const hasImages = paths.some((p) => kindOfPath(p) === "image");
  const hasMedia = paths.some(hasBitrate);
  const targetMb = mode === "target" && hasMedia ? Number(targetText.replace(",", ".")) : null;
  const targetValid = targetMb === null || (Number.isFinite(targetMb) && targetMb > 0);

  useEffect(() => {
    invoke<number[]>("file_sizes", { paths }).then(setSizes);
  }, [paths]);

  useEffect(() => {
    const unlisten = listen<ConvertProgress>("compress-progress", ({ payload }) =>
      setProgress((p) => p && p.map((v, i) => (i === payload.index ? payload.progress : v))),
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const preview = usePreview(previewPath, level, reduceColors, setError);

  async function run() {
    if (progress || !targetValid) return;
    setError(null);
    setResults([]);
    setProgress(paths.map(() => 0));
    try {
      const options: Options = { level, reduceColors, targetMb };
      const res = await invoke<CompressResult[]>("compress_files", { paths, options });
      setResults(res);
      const outputs = res.flatMap((r) => (r.outcome?.status === "saved" ? [r.outcome.output] : []));
      if (outputs.length) onSaved(outputs);
    } catch (e) {
      setError(String(e));
    } finally {
      setProgress(null);
    }
  }

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter" && !(e.target instanceof HTMLInputElement)) run();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <div className="compress">
      {previewPath ? (
        <PreviewCanvas preview={preview} before={sizes[0]} />
      ) : (
        <ul className="compress-list">
          {paths.map((p, i) => (
            <FileRow key={p} path={p} size={sizes[i]} progress={progress?.[i]} result={results[i]} />
          ))}
        </ul>
      )}

      <aside className="tool-panel">
        {(mode === "quality" || hasImages) && (
          <section>
            <h2>Niveau</h2>
            <div className="chips">
              {LEVELS.map((l) => (
                <button
                  key={l.value}
                  className={`chip ${level === l.value ? "is-active" : ""}`}
                  onClick={() => setLevel(l.value)}
                >
                  {l.label}
                </button>
              ))}
            </div>
            <p className="tool-hint">
              {LEVELS.find((l) => l.value === level)!.hint}
              {mode === "target" && hasMedia && " En mode taille cible, le niveau ne s'applique qu'aux images."}
            </p>
          </section>
        )}

        {paths.some(isPng) && (
          <section>
            <h2>PNG</h2>
            <label className="check">
              <input type="checkbox" checked={reduceColors} onChange={(e) => setReduceColors(e.target.checked)} />
              Réduire les couleurs
            </label>
            <p className="tool-hint">
              Jusqu'à −80 % en passant à 256 couleurs maximum, comme TinyPNG. L'optimisation sans perte est toujours
              appliquée.
            </p>
          </section>
        )}

        {hasMedia && (
          <section>
            <h2>Audio et vidéo</h2>
            <div className="chips chips-2">
              <button className={`chip ${mode === "quality" ? "is-active" : ""}`} onClick={() => setMode("quality")}>
                Qualité
              </button>
              <button className={`chip ${mode === "target" ? "is-active" : ""}`} onClick={() => setMode("target")}>
                Taille cible
              </button>
            </div>
            {mode === "target" && (
              <>
                <label className="target">
                  <input
                    type="number"
                    min={0.1}
                    step={0.5}
                    value={targetText}
                    aria-invalid={!targetValid}
                    onChange={(e) => setTargetText(e.target.value)}
                  />
                  <span>Mo par fichier</span>
                </label>
                <button
                  className={`chip chip-wide ${targetMb === DISCORD_MB ? "is-active" : ""}`}
                  onClick={() => setTargetText(String(DISCORD_MB))}
                >
                  Discord · {DISCORD_MB} Mo
                </button>
                <p className="tool-hint">
                  Le débit est calculé d'après la durée ; la résolution baisse si nécessaire.
                </p>
              </>
            )}
          </section>
        )}
      </aside>

      <footer className="tool-footer">
        <Status
          error={error}
          progress={progress}
          results={results}
          sizes={sizes}
          previewSize={previewPath ? preview.compressed?.size : undefined}
        />
        {nextTool}
        <button className="primary" onClick={run} disabled={!!progress || !targetValid}>
          {progress ? "Compression…" : "Compresser"}
        </button>
      </footer>
    </div>
  );
}

interface Preview {
  original: string | null;
  compressed: { url: string; size: number } | null;
  busy: boolean;
}

/** Aperçu d'une image : original, et version compressée recalculée à chaque réglage. */
function usePreview(
  path: string | null,
  level: Level,
  reduceColors: boolean,
  setError: (e: string | null) => void,
): Preview {
  const [original, setOriginal] = useState<string | null>(null);
  const [compressed, setCompressed] = useState<Preview["compressed"]>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!path) return;
    let url: string | null = null;
    invoke<ArrayBuffer>("read_image", { path })
      .then((bytes) => {
        url = URL.createObjectURL(new Blob([bytes]));
        setOriginal(url);
      })
      .catch((e) => setError(String(e)));
    return () => {
      if (url) URL.revokeObjectURL(url);
    };
  }, [path]);

  useEffect(() => {
    if (!path) return;
    let cancelled = false;
    let url: string | null = null;
    setBusy(true);
    // Petit délai : on ne recalcule pas pour chaque clic intermédiaire.
    const timer = setTimeout(() => {
      const options: Options = { level, reduceColors, targetMb: null };
      invoke<ArrayBuffer>("compress_preview", { path, options })
        .then((bytes) => {
          if (cancelled) return;
          url = URL.createObjectURL(new Blob([bytes]));
          setCompressed({ url, size: bytes.byteLength });
          setError(null);
        })
        .catch((e) => !cancelled && setError(String(e)))
        .finally(() => !cancelled && setBusy(false));
    }, 200);
    return () => {
      cancelled = true;
      clearTimeout(timer);
      if (url) URL.revokeObjectURL(url);
    };
  }, [path, level, reduceColors]);

  return { original, compressed, busy };
}

function PreviewCanvas({ preview, before }: { preview: Preview; before: number | undefined }) {
  const [showOriginal, setShowOriginal] = useState(false);
  const { original, compressed, busy } = preview;
  const src = showOriginal ? original : (compressed?.url ?? original);
  const label = showOriginal || !compressed ? "Original" : "Compressé";
  const size = showOriginal || !compressed ? before : compressed.size;

  return (
    <div className="compress-canvas">
      {src && <img src={src} alt="" draggable={false} />}
      <div className="compress-badge">
        <strong>{label}</strong>
        {size !== undefined && <span>{formatBytes(size)}</span>}
        {busy && <span className="compress-busy">Calcul…</span>}
      </div>
      {compressed && (
        <button
          className="compress-compare"
          onPointerDown={() => setShowOriginal(true)}
          onPointerUp={() => setShowOriginal(false)}
          onPointerLeave={() => setShowOriginal(false)}
        >
          Maintenir pour voir l'original
        </button>
      )}
      {!src && <p className="tool-hint">Chargement…</p>}
    </div>
  );
}

function FileRow(props: { path: string; size?: number; progress?: number; result?: CompressResult }) {
  const { path, size, progress, result } = props;
  const outcome = result?.outcome;
  let after: ReactNode = null;
  if (result?.error) {
    after = (
      <span className="is-error" title={result.error}>
        Erreur
      </span>
    );
  } else if (outcome?.status === "saved") {
    after = (
      <span title={outcome.output}>
        {formatBytes(outcome.after)} <em>{percentLess(outcome.before, outcome.after)}</em>
      </span>
    );
  } else if (outcome?.status === "noGain") {
    after = <span className="is-muted">déjà optimisé</span>;
  } else if (progress !== undefined) {
    after = <span className="is-muted">{Math.round(progress * 100)} %</span>;
  }

  return (
    <li className={`kind-${kindOfPath(path)}`}>
      <span className="compress-name" title={path}>
        {basename(path)}
      </span>
      <span className="compress-size">{size !== undefined ? formatBytes(size) : ""}</span>
      <span className="compress-after">{after}</span>
      {progress !== undefined && <div className="compress-bar" style={{ width: `${progress * 100}%` }} />}
    </li>
  );
}

function Status(props: {
  error: string | null;
  progress: number[] | null;
  results: CompressResult[];
  sizes: number[];
  previewSize?: number;
}) {
  const { error, progress, results, sizes, previewSize } = props;
  let text: string;
  if (error) {
    text = error;
  } else if (progress) {
    text = `Compression… ${Math.round((progress.reduce((a, b) => a + b, 0) / progress.length) * 100)} %`;
  } else if (results.length) {
    const failed = results.filter((r) => r.error).length;
    const done = results.flatMap((r) => (r.outcome ? [r.outcome] : []));
    const before = done.reduce((s, o) => s + o.before, 0);
    const after = done.reduce((s, o) => s + (o.status === "saved" ? o.after : o.before), 0);
    if (done.length && done.every((o) => o.status === "noGain")) {
      text = "Déjà optimisé : rien n'a été enregistré.";
    } else if (done.length) {
      text = `${formatBytes(before)} → ${formatBytes(after)} (${percentLess(before, after)})`;
    } else {
      text = "";
    }
    if (failed) text += `${text ? " · " : ""}${failed} erreur${failed > 1 ? "s" : ""} (survoler pour le détail)`;
  } else if (previewSize !== undefined && sizes[0]) {
    text =
      previewSize < sizes[0]
        ? `${formatBytes(sizes[0])} → ${formatBytes(previewSize)} (${percentLess(sizes[0], previewSize)})`
        : `${formatBytes(sizes[0])} : déjà optimisé à ce niveau`;
  } else {
    text = sizes.length ? `Poids actuel : ${formatBytes(sizes.reduce((a, b) => a + b, 0))}` : "";
  }

  const failedOnly = !error && results.length > 0 && results.every((r) => r.error);
  return (
    <span className={`tool-status ${error || failedOnly ? "is-error" : ""}`} title={failedOnly ? results[0].error! : undefined}>
      {text}
    </span>
  );
}
