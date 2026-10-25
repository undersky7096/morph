// Découpe commune aux éditeurs audio et vidéo : lecture qui saute les passages retirés,
// sélection, coupes, zoom, raccourcis clavier, barre d'outils et liste des passages.

import { useEffect, useRef, useState } from "react";

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
// `t` désigne ici la découpe en cours : la traduction s'appelle `tr`.
import { t as tr } from "../../i18n";
import { formatTime, intersect, moveEdge, type Segment, segmentAt, split, subtract, totalLength } from "./segments";
import Waveform, { Overview, panView, type Range, type View, zoomView } from "./Waveform";

/** Gain de l'aperçu, par élément (une source Web Audio ne peut être créée qu'une fois). */
const gains = new WeakMap<HTMLMediaElement, GainNode>();

/**
 * Fait passer le son du lecteur par un nœud de gain : `media.volume` plafonne à 100 %,
 * le gain va jusqu'à 200 %. À appeler lors d'un geste de l'utilisateur (lecture), sans
 * quoi le navigateur laisse le contexte audio suspendu.
 */
function gainFor(media: HTMLMediaElement): GainNode {
  let gain = gains.get(media);
  if (!gain) {
    const ctx = new AudioContext();
    gain = ctx.createGain();
    ctx.createMediaElementSource(media).connect(gain).connect(ctx.destination);
    gains.set(media, gain);
  }
  if (gain.context.state === "suspended") (gain.context as AudioContext).resume();
  return gain;
}

export interface Trimmer {
  media: HTMLMediaElement;
  duration: number;
  segments: Segment[];
  time: number;
  playing: boolean;
  selection: Range | null;
  view: View;
  setSelection: (r: Range | null) => void;
  setView: (v: View) => void;
  seek: (t: number) => void;
  togglePlay: () => void;
  removeSelection: () => void;
  keepSelection: () => void;
  cutHere: () => void;
  removeSegment: (i: number) => void;
  restore: () => void;
  onEdge: (index: number, edge: "start" | "end", t: number) => void;
  zoom: (factor: number) => void;
  /** Raccourcis de la découpe ; renvoie vrai si la touche a été prise en charge. */
  handleKey: (e: KeyboardEvent) => boolean;
}

/**
 * État de lecture et opérations de découpe. Les passages vivent dans l'historique de
 * l'éditeur (`update`), pour que annuler/rétablir couvre aussi la découpe.
 */
export function useTrimmer(
  media: HTMLMediaElement,
  duration: number,
  segments: Segment[],
  update: (fn: (segs: Segment[]) => Segment[], mergeKey?: string) => void,
  onError: (message: string) => void,
  /** Volume d'écoute, 0…2 (200 %). */
  volume = 1,
): Trimmer {
  const [view, setView] = useState<View>({ start: 0, end: Math.max(duration, 1) });
  const [selection, setSelection] = useState<Range | null>(null);
  const [time, setTime] = useState(0);
  const [playing, setPlaying] = useState(false);
  // Lus par la boucle de lecture sans la relancer.
  const segsRef = useRef(segments);
  segsRef.current = segments;

  useEffect(() => setView({ start: 0, end: Math.max(duration, 1) }), [duration]);

  // Contexte audio libéré à la fermeture de l'éditeur (le navigateur en limite le nombre).
  useEffect(
    () => () => {
      (gains.get(media)?.context as AudioContext | undefined)?.close();
      gains.delete(media);
    },
    [media],
  );

  // Le gain n'existe qu'après la première lecture ; ensuite il suit le réglage.
  useEffect(() => {
    const gain = gains.get(media);
    if (gain) gain.gain.value = volume;
  }, [volume, media]);

  // Lecture : le curseur suit le média et saute les passages retirés (on entend le résultat).
  useEffect(() => {
    if (!playing) return;
    let frame = 0;
    const tick = () => {
      const segs = segsRef.current;
      const t = media.currentTime;
      if (segmentAt(segs, t) < 0) {
        const next = segs.find((s) => s.start > t);
        if (next) media.currentTime = next.start;
        else {
          media.pause();
          setPlaying(false);
          setTime(segs.length ? Math.min(t, segs[segs.length - 1].end) : t);
          return;
        }
      }
      setTime(media.currentTime);
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    const stop = () => setPlaying(false);
    media.addEventListener("pause", stop);
    return () => {
      cancelAnimationFrame(frame);
      media.removeEventListener("pause", stop);
    };
  }, [playing, media]);

  // Pendant la lecture, la vue suit le curseur quand il en sort.
  useEffect(() => {
    if (!playing || (time >= view.start && time <= view.end)) return;
    setView(panView(view, time - view.start - (view.end - view.start) * 0.1, duration));
  }, [time]);

  function seek(t: number) {
    media.currentTime = t;
    setTime(t);
  }

  async function togglePlay() {
    if (playing) return media.pause();
    if (!segments.length || !media.currentSrc) return;
    const last = segments[segments.length - 1];
    let t = time;
    if (t >= last.end - 0.02) t = segments[0].start;
    else if (segmentAt(segments, t) < 0) t = segments.find((s) => s.start > t)!.start;
    seek(t);
    try {
      gainFor(media).gain.value = volume;
    } catch {
      // Sans Web Audio, on garde le volume natif (plafonné à 100 %).
      media.volume = Math.min(1, volume);
    }
    try {
      await media.play();
      setPlaying(true);
    } catch (e) {
      onError(String(e));
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
  const restore = () => update(() => [{ start: 0, end: duration }]);
  const onEdge = (index: number, edge: "start" | "end", t: number) =>
    update((s) => moveEdge(s, index, edge, t, duration), `edge-${index}-${edge}`);

  function zoom(factor: number) {
    const inView = time >= view.start && time <= view.end;
    setView(zoomView(view, factor, inView ? time : (view.start + view.end) / 2, duration));
  }

  function handleKey(e: KeyboardEvent): boolean {
    const key = e.key.toLowerCase();
    if (e.ctrlKey || e.metaKey || e.altKey) return false;
    if (e.key === " ") {
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
      const step = (e.shiftKey ? 0.1 : 1) * (e.key === "ArrowLeft" ? -1 : 1);
      const t = Math.min(duration, Math.max(0, time + step));
      seek(t);
      if (t < view.start || t > view.end) setView(panView(view, t - (view.start + view.end) / 2, duration));
    } else if (e.key === "Home") {
      seek(segments[0]?.start ?? 0);
    } else {
      return false;
    }
    return true;
  }

  return {
    media,
    duration,
    segments,
    time,
    playing,
    selection,
    view,
    setSelection,
    setView,
    seek,
    togglePlay,
    removeSelection,
    keepSelection,
    cutHere,
    removeSegment,
    restore,
    onEdge,
    zoom,
    handleKey,
  };
}

/** Forme d'onde détaillée + vue d'ensemble. */
export function TrimTimeline({ trimmer: t, peaks }: { trimmer: Trimmer; peaks: Uint8Array }) {
  return (
    <>
      <Waveform
        peaks={peaks}
        duration={t.duration}
        segments={t.segments}
        selection={t.selection}
        view={t.view}
        time={t.time}
        onViewChange={t.setView}
        onSeek={t.seek}
        onSelect={t.setSelection}
        onEdge={t.onEdge}
      />
      <Overview
        peaks={peaks}
        duration={t.duration}
        segments={t.segments}
        view={t.view}
        time={t.time}
        onViewChange={t.setView}
      />
    </>
  );
}

interface UndoRedo {
  undo: () => void;
  redo: () => void;
  canUndo: boolean;
  canRedo: boolean;
}

/** Lecture, temps, coupes, annuler/rétablir et zoom. */
export function TrimTransport({ trimmer: t, history, ready }: { trimmer: Trimmer; history: UndoRedo; ready: boolean }) {
  return (
    <div className="audio-transport">
      <button
        className="audio-play"
        onClick={t.togglePlay}
        disabled={!ready}
        aria-label={t.playing ? tr("editor.pause") : tr("editor.play")}
        title={tr("editor.play_pause_space")}
      >
        {t.playing ? <PauseIcon /> : <PlayIcon />}
      </button>
      <span className="audio-time">
        {formatTime(t.time)} <span>/ {formatTime(ready ? t.duration : 0)}</span>
      </span>

      <div className="editor-toolbar">
        <button onClick={t.cutHere} disabled={!ready} title={tr("editor.cut_at_playhead_s")}>
          <ScissorsIcon />
        </button>
        <button onClick={t.removeSelection} disabled={!t.selection} title={tr("editor.remove_selection_delete")}>
          <TrashIcon />
        </button>
        <button onClick={t.keepSelection} disabled={!t.selection} title={tr("editor.keep_only_selection_k")}>
          <KeepIcon />
        </button>
        <span className="editor-toolbar-sep" />
        <button onClick={history.undo} disabled={!history.canUndo} title={tr("editor.undo_ctrl_z")}>
          <UndoIcon />
        </button>
        <button onClick={history.redo} disabled={!history.canRedo} title={tr("editor.redo_ctrl_y")}>
          <RedoIcon />
        </button>
        <span className="editor-toolbar-sep" />
        <button onClick={() => t.zoom(1 / 2)} disabled={!ready} title={tr("editor.zoom_ctrl_scroll")}>
          <ZoomInIcon />
        </button>
        <button onClick={() => t.zoom(2)} disabled={!ready} title={tr("editor.zoom_out")}>
          <ZoomOutIcon />
        </button>
        <button onClick={() => t.setView({ start: 0, end: t.duration })} disabled={!ready} title={tr("editor.show_all")}>
          <FitIcon />
        </button>
      </div>
    </div>
  );
}

/** Liste des passages gardés (clic : sélection et lecture ; corbeille : retrait). */
export function SegmentList({ trimmer: t, ready }: { trimmer: Trimmer; ready: boolean }) {
  const kept = totalLength(t.segments);
  return (
    <>
      <ol className="segments">
        {t.segments.map((s, i) => {
          const active = t.selection?.a === s.start && t.selection?.b === s.end;
          return (
            <li key={i} className={active ? "is-active" : ""}>
              <button
                className="segment-main"
                onClick={() => {
                  t.setSelection({ a: s.start, b: s.end });
                  t.seek(s.start);
                }}
              >
                <span className="segment-index">{i + 1}</span>
                <span className="segment-range">
                  {formatTime(s.start)} → {formatTime(s.end)}
                </span>
                <span className="segment-length">{formatTime(s.end - s.start)}</span>
              </button>
              <button
                className="segment-remove"
                onClick={() => t.removeSegment(i)}
                aria-label={tr("editor.remove_section")}
                title={tr("editor.remove_section")}
              >
                <TrashIcon />
              </button>
            </li>
          );
        })}
      </ol>
      {ready && !t.segments.length && <p className="tool-hint">{tr("editor.no_section_kept_undo_restore")}</p>}
      {ready && (t.segments.length !== 1 || kept < t.duration - 0.005) && (
        <button className="link" onClick={t.restore}>
          {tr("editor.restore_all")}
        </button>
      )}
    </>
  );
}

/** « 2:31,00 (−0:45,00) » : durée gardée, et ce qui est retiré. */
export function keptSummary(segments: Segment[], duration: number, speed = 1): string {
  const kept = totalLength(segments);
  const removed = kept < duration - 0.005 ? ` (−${formatTime(duration - kept)})` : "";
  return `${formatTime(kept / speed)}${speed === 1 ? removed : ""}`;
}
