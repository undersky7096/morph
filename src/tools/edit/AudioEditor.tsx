import { type ReactNode, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import {
  FitIcon,
  KeepIcon,
  PauseIcon,
  PlayIcon,
  RedoIcon,
  ScissorsIcon,
  TrashIcon,
  UndoIcon,
  ZoomInIcon,
  ZoomOutIcon,
} from "../../icons";
import { useHistory } from "./history";
import { formatTime, intersect, moveEdge, type Segment, segmentAt, split, subtract, totalLength } from "./segments";
import Waveform, { Overview, panView, type Range, type View, zoomView } from "./Waveform";
// Barre d'outils et grille communes avec l'éditeur image.
import "./ImageEditor.css";
import "./AudioEditor.css";

interface Props {
  path: string;
  /** Appelé avec le chemin du fichier enregistré. */
  onSaved: (output: string) => void;
  /** Boutons supplémentaires du pied de fenêtre (ex : « Passer » dans un lot). */
  footerExtra?: ReactNode;
}

interface Track {
  duration: number;
  peaks: Uint8Array;
}

/** Formats que le webview ne sait pas lire : écoutés via une copie WAV préparée par ffmpeg. */
const NOT_PLAYABLE = new Set(["wma"]);

/** Découpe audio : on garde un ou plusieurs passages, les trous sont retirés à l'enregistrement. */
export default function AudioEditor({ path, onSaved, footerExtra }: Props) {
  const history = useHistory<Segment[]>([]);
  const { state: segments, update } = history;
  const [track, setTrack] = useState<Track | null>(null);
  const [view, setView] = useState<View>({ start: 0, end: 1 });
  const [selection, setSelection] = useState<Range | null>(null);
  const [time, setTime] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [busy, setBusy] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [player] = useState(() => new Audio());
  const audio = useRef(player);
  // Lus par la boucle de lecture sans la relancer.
  const segsRef = useRef(segments);
  segsRef.current = segments;

  // Forme d'onde et fichier à écouter.
  useEffect(() => {
    let url: string | null = null;
    let cancelled = false;
    const el = audio.current;
    const load = async (preview: boolean) => {
      const bytes = await invoke<ArrayBuffer>(preview ? "audio_preview" : "read_media", { path });
      if (cancelled) return;
      if (url) URL.revokeObjectURL(url);
      url = URL.createObjectURL(new Blob([bytes]));
      el.src = url;
    };
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    let fellBack = NOT_PLAYABLE.has(ext);
    el.onerror = () => {
      if (fellBack) return setError("Lecture impossible pour ce fichier (la découpe reste possible).");
      fellBack = true;
      load(true).catch((e) => setError(String(e)));
    };
    load(fellBack).catch((e) => setError(String(e)));

    invoke<ArrayBuffer>("audio_peaks", { path })
      .then((buf) => {
        if (cancelled) return;
        const duration = new DataView(buf).getFloat64(0, true);
        setTrack({ duration, peaks: new Uint8Array(buf, 8) });
        setView({ start: 0, end: duration });
        history.reset([{ start: 0, end: duration }]);
      })
      .catch((e) => setError(String(e)));

    return () => {
      cancelled = true;
      el.pause();
      el.onerror = null;
      el.removeAttribute("src");
      if (url) URL.revokeObjectURL(url);
    };
  }, [path]);

  // Lecture : le curseur suit le son et saute les passages retirés (on entend le résultat).
  useEffect(() => {
    if (!playing) return;
    const el = audio.current;
    let frame = 0;
    const tick = () => {
      const segs = segsRef.current;
      const t = el.currentTime;
      if (segmentAt(segs, t) < 0) {
        const next = segs.find((s) => s.start > t);
        if (next) el.currentTime = next.start;
        else {
          el.pause();
          setPlaying(false);
          setTime(segs.length ? Math.min(t, segs[segs.length - 1].end) : t);
          return;
        }
      }
      setTime(el.currentTime);
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    const stop = () => setPlaying(false);
    el.addEventListener("pause", stop);
    return () => {
      cancelAnimationFrame(frame);
      el.removeEventListener("pause", stop);
    };
  }, [playing]);

  function seek(t: number) {
    audio.current.currentTime = t;
    setTime(t);
  }

  async function togglePlay() {
    const el = audio.current;
    if (playing) return el.pause();
    if (!segments.length || !el.src) return;
    const last = segments[segments.length - 1];
    let t = time;
    if (t >= last.end - 0.02) t = segments[0].start;
    else if (segmentAt(segments, t) < 0) t = segments.find((s) => s.start > t)!.start;
    seek(t);
    try {
      await el.play();
      setPlaying(true);
    } catch (e) {
      setError(String(e));
    }
  }

  const removeSelection = () => {
    if (!selection) return;
    update((s) => subtract(s, selection.a, selection.b));
    setSelection(null);
  };
  const keepSelection = () => {
    if (!selection) return;
    update((s) => intersect(s, selection.a, selection.b));
    setSelection(null);
  };
  const cutHere = () => update((s) => split(s, time));
  const removeSegment = (i: number) => update((s) => s.filter((_, j) => j !== i));
  const restore = () => track && update(() => [{ start: 0, end: track.duration }]);

  function onEdge(index: number, edge: "start" | "end", t: number) {
    if (!track) return;
    update((s) => moveEdge(s, index, edge, t, track.duration), `edge-${index}-${edge}`);
  }

  function zoom(factor: number) {
    if (!track) return;
    const inView = time >= view.start && time <= view.end;
    setView(zoomView(view, factor, inView ? time : (view.start + view.end) / 2, track.duration));
  }

  async function save() {
    if (!track || !segments.length || busy !== null) return;
    audio.current.pause();
    setBusy(0);
    setError(null);
    const unlisten = await listen<number>("trim-progress", ({ payload }) => setBusy(payload));
    try {
      const output = await invoke<string>("trim_audio", { path, segments });
      onSaved(output);
    } catch (e) {
      setError(String(e));
    } finally {
      unlisten();
      setBusy(null);
    }
  }

  // Raccourcis : Espace lecture, S couper, Suppr retirer, ← → se déplacer, annuler/rétablir, Entrée ou Ctrl+S.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement) return;
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.toLowerCase();
      if (mod && key === "s") {
        e.preventDefault();
        save();
      } else if (mod && key === "z") {
        e.preventDefault();
        if (e.shiftKey) history.redo();
        else history.undo();
      } else if (mod && key === "y") {
        e.preventDefault();
        history.redo();
      } else if (e.key === " ") {
        // Sans quoi le bouton qui a le focus serait aussi activé.
        e.preventDefault();
        (document.activeElement as HTMLElement | null)?.blur();
        togglePlay();
      } else if (key === "s") {
        cutHere();
      } else if (e.key === "Delete" || e.key === "Backspace") {
        removeSelection();
      } else if (key === "k") {
        keepSelection();
      } else if (e.key === "Escape") {
        setSelection(null);
      } else if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        if (!track) return;
        const step = (e.shiftKey ? 0.1 : 1) * (e.key === "ArrowLeft" ? -1 : 1);
        const t = Math.min(track.duration, Math.max(0, time + step));
        seek(t);
        if (t < view.start || t > view.end) setView(panView(view, t - (view.start + view.end) / 2, track.duration));
      } else if (e.key === "Home") {
        seek(segments[0]?.start ?? 0);
      } else if (e.key === "Enter") {
        save();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Pendant la lecture, la vue suit le curseur quand il en sort.
  useEffect(() => {
    if (!playing || !track || (time >= view.start && time <= view.end)) return;
    setView(panView(view, time - view.start - (view.end - view.start) * 0.1, track.duration));
  }, [time]);

  const kept = totalLength(segments);
  const status =
    error ??
    (busy !== null
      ? `Enregistrement… ${Math.round(busy * 100)} %`
      : track
        ? `Résultat : ${formatTime(kept)}${kept < track.duration - 0.005 ? ` (−${formatTime(track.duration - kept)})` : ""}`
        : "Analyse du fichier…");

  return (
    <div className="editor audio-editor">
      <div className="audio-area">
        {track ? (
          <>
            <Waveform
              peaks={track.peaks}
              duration={track.duration}
              segments={segments}
              selection={selection}
              view={view}
              time={time}
              onViewChange={setView}
              onSeek={seek}
              onSelect={setSelection}
              onEdge={onEdge}
            />
            <Overview
              peaks={track.peaks}
              duration={track.duration}
              segments={segments}
              view={view}
              time={time}
              onViewChange={setView}
            />
          </>
        ) : (
          <p className="tool-hint audio-loading">{error ? "" : "Analyse de la forme d'onde…"}</p>
        )}

        <div className="audio-transport">
          <button className="audio-play" onClick={togglePlay} disabled={!track} aria-label={playing ? "Pause" : "Lecture"} title="Lecture / pause (Espace)">
            {playing ? <PauseIcon /> : <PlayIcon />}
          </button>
          <span className="audio-time">
            {formatTime(time)} <span>/ {formatTime(track?.duration ?? 0)}</span>
          </span>

          <div className="editor-toolbar">
            <button onClick={cutHere} disabled={!track} title="Couper au curseur de lecture (S)">
              <ScissorsIcon />
            </button>
            <button onClick={removeSelection} disabled={!selection} title="Retirer la sélection (Suppr)">
              <TrashIcon />
            </button>
            <button onClick={keepSelection} disabled={!selection} title="Ne garder que la sélection (K)">
              <KeepIcon />
            </button>
            <span className="editor-toolbar-sep" />
            <button onClick={history.undo} disabled={!history.canUndo} title="Annuler (Ctrl+Z)">
              <UndoIcon />
            </button>
            <button onClick={history.redo} disabled={!history.canRedo} title="Rétablir (Ctrl+Y)">
              <RedoIcon />
            </button>
            <span className="editor-toolbar-sep" />
            <button onClick={() => zoom(1 / 2)} disabled={!track} title="Zoom avant (Ctrl + molette)">
              <ZoomInIcon />
            </button>
            <button onClick={() => zoom(2)} disabled={!track} title="Zoom arrière">
              <ZoomOutIcon />
            </button>
            <button onClick={() => track && setView({ start: 0, end: track.duration })} disabled={!track} title="Tout afficher">
              <FitIcon />
            </button>
          </div>
        </div>
      </div>

      <aside className="tool-panel">
        <section>
          <h2>Passages gardés</h2>
          <ol className="segments">
            {segments.map((s, i) => {
              const active = selection?.a === s.start && selection?.b === s.end;
              return (
                <li key={i} className={active ? "is-active" : ""}>
                  <button
                    className="segment-main"
                    onClick={() => {
                      setSelection({ a: s.start, b: s.end });
                      seek(s.start);
                    }}
                  >
                    <span className="segment-index">{i + 1}</span>
                    <span className="segment-range">
                      {formatTime(s.start)} → {formatTime(s.end)}
                    </span>
                    <span className="segment-length">{formatTime(s.end - s.start)}</span>
                  </button>
                  <button className="segment-remove" onClick={() => removeSegment(i)} aria-label="Retirer ce passage" title="Retirer ce passage">
                    <TrashIcon />
                  </button>
                </li>
              );
            })}
          </ol>
          {track && !segments.length && <p className="tool-hint">Aucun passage gardé : annulez ou restaurez.</p>}
          {track && (segments.length !== 1 || kept < track.duration - 0.005) && (
            <button className="link" onClick={restore}>
              Tout restaurer
            </button>
          )}
        </section>

        <section>
          <h2>Mode d'emploi</h2>
          <ul className="audio-help">
            <li>Glissez sur la forme d'onde pour sélectionner, puis <kbd>Suppr</kbd> pour retirer ce passage.</li>
            <li>Tirez les bords violets pour ajuster le début et la fin.</li>
            <li><kbd>S</kbd> coupe au curseur de lecture ; double-clic : sélectionne un passage.</li>
            <li>La lecture saute les passages retirés : vous entendez le résultat.</li>
            <li><kbd>Ctrl</kbd> + molette pour zoomer.</li>
          </ul>
        </section>
      </aside>

      <footer className="tool-footer">
        <span className={`tool-status ${error ? "is-error" : ""}`}>{status}</span>
        {footerExtra}
        <button className="primary" onClick={save} disabled={!track || !segments.length || busy !== null}>
          {busy !== null ? "Enregistrement…" : "Enregistrer"}
        </button>
      </footer>
    </div>
  );
}
