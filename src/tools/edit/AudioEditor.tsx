import { type ReactNode, useEffect, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { describe } from "../../errors";
import { type Duplicates, SaveExtras, useSaveFlow } from "../save";
import { t } from "../../i18n";
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
  const flow = useSaveFlow();
  const [player] = useState(() => new Audio());
  const updateSegments = (fn: (s: Segment[]) => Segment[], key?: string) =>
    update((s) => ({ ...s, segments: fn(s.segments) }), key);
  const trimmer = useTrimmer(player, track?.duration ?? 0, segments, updateSegments, setError, volume / 100);

  // Forme d'onde et fichier à écouter (lu en continu sur le disque, jamais chargé en entier).
  useEffect(() => {
    let cancelled = false;
    const load = async (preview: boolean) => {
      const file = await invoke<string>(preview ? "audio_preview" : "allow_media", { path });
      if (!cancelled) player.src = convertFileSrc(file);
    };
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    let fellBack = NOT_PLAYABLE.has(ext);
    player.onerror = () => {
      if (fellBack) return setError(t("Lecture impossible pour ce fichier (la découpe reste possible).", "This file can’t be played (trimming still works)."));
      fellBack = true;
      load(true).catch((e) => setError(describe(e).message));
    };
    load(fellBack).catch((e) => setError(describe(e).message));

    loadTrack(path)
      .then((loaded) => {
        if (cancelled) return;
        setTrack(loaded);
        history.reset({ segments: [{ start: 0, end: loaded.duration }], volume: 100 });
      })
      .catch((e) => setError(describe(e).message));

    return () => {
      cancelled = true;
      player.pause();
      player.onerror = null;
      player.removeAttribute("src");
    };
  }, [path]);

  async function save(duplicates?: Duplicates) {
    if (!track || !segments.length || busy !== null) return;
    const run = await flow.begin(duplicates);
    if (!run) return;
    player.pause();
    setBusy(0);
    setError(null);
    const unlisten = await listen<number>("trim-progress", ({ payload }) => setBusy(payload));
    try {
      const output = await invoke<string>("trim_audio", { path, segments, volume: volume / 100, run });
      onSaved(output);
    } catch (e) {
      const err = flow.fail(e, save);
      if (err) setError(err.message);
    } finally {
      flow.end();
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
      ? `${t("Enregistrement…", "Saving…")} ${Math.round(busy * 100)} %`
      : track
        ? `${t("Résultat : ", "Result: ")}${keptSummary(segments, track.duration)}`
        : t("Analyse du fichier…", "Analysing file…"));

  return (
    <div className="editor audio-editor">
      <div className="audio-area">
        {track ? (
          <TrimTimeline trimmer={trimmer} peaks={track.peaks} />
        ) : (
          <p className="tool-hint audio-loading">{error ? "" : t("Analyse de la forme d'onde…", "Analysing waveform…")}</p>
        )}
        <TrimTransport trimmer={trimmer} history={history} ready={!!track} />
      </div>

      <aside className="tool-panel">
        <section>
          <h2>{t("Passages gardés", "Kept sections")}</h2>
          <SegmentList trimmer={trimmer} ready={!!track} />
        </section>

        <section>
          <h2>{t("Volume", "Volume")}</h2>
          <Slider
            label={t("Volume", "Volume")}
            min={0}
            max={200}
            neutral={100}
            unit=" %"
            value={volume}
            onChange={(v) => update((s) => ({ ...s, volume: v }), "volume")}
          />
          <p className="tool-hint">{t("Au-delà de 100 %, un limiteur évite la saturation.", "Above 100%, a limiter prevents clipping.")}</p>
        </section>

        <section>
          <h2>{t("Mode d'emploi", "How to use")}</h2>
          <ul className="audio-help">
            <li>
              {t("Glissez sur la forme d'onde pour sélectionner, puis ", "Drag on the waveform to select, then ")}
              <kbd>{t("Suppr", "Delete")}</kbd>
              {t(" pour retirer ce passage.", " to remove that section.")}
            </li>
            <li>{t("Tirez les bords violets pour ajuster le début et la fin.", "Drag the purple edges to adjust the start and end.")}</li>
            <li>
              <kbd>S</kbd>
              {t(" coupe au curseur de lecture ; double-clic : sélectionne un passage.", " cuts at the playhead; double-click selects a section.")}
            </li>
            <li>{t("La lecture saute les passages retirés : vous entendez le résultat.", "Playback skips removed sections: you hear the result.")}</li>
            <li>
              <kbd>Ctrl</kbd>
              {t(" + molette pour zoomer.", " + scroll wheel to zoom.")}
            </li>
          </ul>
        </section>
      </aside>

      <footer className="tool-footer">
        <span className={`tool-status ${error ? "is-error" : ""}`}>{status}</span>
        <SaveExtras flow={flow} />
        {footerExtra}
        <button className="primary" onClick={() => save()} disabled={!track || !segments.length || busy !== null}>
          {busy !== null ? t("Enregistrement…", "Saving…") : t("Enregistrer", "Save")}
        </button>
      </footer>
    </div>
  );
}
