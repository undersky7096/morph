import { type ReactNode, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { chooseOutputDir } from "../../settings";
import { Slider } from "./controls";
import { useHistory } from "./history";
import type { Segment } from "./segments";
import { keptSummary, SegmentList, TrimTimeline, TrimTransport, useTrimmer } from "./Timeline";
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

export interface Track {
  duration: number;
  peaks: Uint8Array;
}

/** Forme d'onde d'un fichier audio ou du son d'une vidéo (durée en tête, puis un pic par 10 ms). */
export async function loadTrack(path: string): Promise<Track> {
  const buf = await invoke<ArrayBuffer>("audio_peaks", { path });
  return { duration: new DataView(buf).getFloat64(0, true), peaks: new Uint8Array(buf, 8) };
}

/** Passages gardés et volume (0…200 %), dans un même historique. */
interface AudioState {
  segments: Segment[];
  volume: number;
}

/** Formats que le webview ne sait pas lire : écoutés via une copie WAV préparée par ffmpeg. */
const NOT_PLAYABLE = new Set(["wma"]);

/** Découpe audio : on garde un ou plusieurs passages, les trous sont retirés à l'enregistrement. */
export default function AudioEditor({ path, onSaved, footerExtra }: Props) {
  const history = useHistory<AudioState>({ segments: [], volume: 100 });
  const { state, update } = history;
  const { segments, volume } = state;
  const [track, setTrack] = useState<Track | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [player] = useState(() => new Audio());
  const updateSegments = (fn: (s: Segment[]) => Segment[], key?: string) =>
    update((s) => ({ ...s, segments: fn(s.segments) }), key);
  const trimmer = useTrimmer(player, track?.duration ?? 0, segments, updateSegments, setError, volume / 100);

  // Forme d'onde et fichier à écouter.
  useEffect(() => {
    let url: string | null = null;
    let cancelled = false;
    const load = async (preview: boolean) => {
      const bytes = await invoke<ArrayBuffer>(preview ? "audio_preview" : "read_media", { path });
      if (cancelled) return;
      if (url) URL.revokeObjectURL(url);
      url = URL.createObjectURL(new Blob([bytes]));
      player.src = url;
    };
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    let fellBack = NOT_PLAYABLE.has(ext);
    player.onerror = () => {
      if (fellBack) return setError("Lecture impossible pour ce fichier (la découpe reste possible).");
      fellBack = true;
      load(true).catch((e) => setError(String(e)));
    };
    load(fellBack).catch((e) => setError(String(e)));

    loadTrack(path)
      .then((t) => {
        if (cancelled) return;
        setTrack(t);
        history.reset({ segments: [{ start: 0, end: t.duration }], volume: 100 });
      })
      .catch((e) => setError(String(e)));

    return () => {
      cancelled = true;
      player.pause();
      player.onerror = null;
      player.removeAttribute("src");
      if (url) URL.revokeObjectURL(url);
    };
  }, [path]);

  async function save() {
    if (!track || !segments.length || busy !== null) return;
    const outDir = await chooseOutputDir();
    if (outDir === null) return;
    player.pause();
    setBusy(0);
    setError(null);
    const unlisten = await listen<number>("trim-progress", ({ payload }) => setBusy(payload));
    try {
      const output = await invoke<string>("trim_audio", { path, segments, volume: volume / 100, outDir });
      onSaved(output);
    } catch (e) {
      setError(String(e));
    } finally {
      unlisten();
      setBusy(null);
    }
  }

  // Raccourcis : ceux de la découpe, annuler/rétablir, Entrée ou Ctrl+S pour enregistrer.
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
      } else if (track && trimmer.handleKey(e)) {
        return;
      } else if (e.key === "Enter") {
        save();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const status =
    error ??
    (busy !== null
      ? `Enregistrement… ${Math.round(busy * 100)} %`
      : track
        ? `Résultat : ${keptSummary(segments, track.duration)}`
        : "Analyse du fichier…");

  return (
    <div className="editor audio-editor">
      <div className="audio-area">
        {track ? (
          <TrimTimeline trimmer={trimmer} peaks={track.peaks} />
        ) : (
          <p className="tool-hint audio-loading">{error ? "" : "Analyse de la forme d'onde…"}</p>
        )}
        <TrimTransport trimmer={trimmer} history={history} ready={!!track} />
      </div>

      <aside className="tool-panel">
        <section>
          <h2>Passages gardés</h2>
          <SegmentList trimmer={trimmer} ready={!!track} />
        </section>

        <section>
          <h2>Volume</h2>
          <Slider
            label="Volume"
            min={0}
            max={200}
            neutral={100}
            unit=" %"
            value={volume}
            onChange={(v) => update((s) => ({ ...s, volume: v }), "volume")}
          />
          <p className="tool-hint">Au-delà de 100 %, un limiteur évite la saturation.</p>
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
