import { type ReactNode, useEffect, useLayoutEffect, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { describe } from "../../errors";
import { type Duplicates, SaveExtras, useSaveFlow } from "../save";
import { locale, t } from "../../i18n";
import { FlipHIcon, FlipVIcon, RotateLeftIcon, RotateRightIcon } from "../../icons";
import { dragRect, fitRatio, type Handle, type Rect } from "../crop/geometry";
import { loadTrack, type Track } from "./AudioEditor";
import { EditorTabs, presetThumbs, Slider } from "./controls";
import { useHistory } from "./history";
import {
  ADJUST_DEFAULT,
  type Adjust,
  COLORS,
  combined,
  EDIT_DEFAULT,
  type Geometry,
  GEOMETRY_DEFAULT,
  PRESETS,
  SLIDERS,
} from "./model";
import { Renderer } from "./renderer";
import type { Segment } from "./segments";
import { keptSummary, SegmentList, TrimTimeline, TrimTransport, useTrimmer } from "./Timeline";
import "./ImageEditor.css";
import "./AudioEditor.css";
import "../crop/Crop.css";
import "./VideoEditor.css";

interface Props {
  path: string;
  /** Appelé avec le chemin du fichier enregistré. */
  onSaved: (output: string) => void;
  /** Boutons supplémentaires du pied de fenêtre (ex : « Passer » dans un lot). */
  footerExtra?: ReactNode;
}

interface VideoInfo {
  duration: number;
  width: number;
  height: number;
  hasAudio: boolean;
}

/** Tout ce que l'enregistrement appliquera ; un seul historique pour annuler/rétablir. */
interface VideoState {
  segments: Segment[];
  adjust: Adjust;
  preset: string;
  invert: boolean;
  geometry: Geometry;
  /** Ratio largeur/hauteur visé, ou null pour garder le cadre d'origine. */
  ratio: number | null;
  method: "crop" | "fit";
  /** Zone gardée (méthode Recadrer), en pixels de la vidéo pivotée ; null : centrée. */
  crop: Rect | null;
  /** Fond des bandes (méthode Ajuster) : null = image floutée, sinon une couleur. */
  fill: string | null;
  shortSide: number | null;
  speed: number;
  /** 0…200 %. */
  volume: number;
  mute: boolean;
  normalize: boolean;
  fadeIn: number;
  fadeOut: number;
}

const DEFAULT: VideoState = {
  segments: [],
  adjust: ADJUST_DEFAULT,
  preset: "none",
  invert: false,
  geometry: GEOMETRY_DEFAULT,
  ratio: null,
  method: "crop",
  crop: null,
  fill: null,
  shortSide: null,
  speed: 1,
  volume: 100,
  mute: false,
  normalize: false,
  fadeIn: 0,
  fadeOut: 0,
};

type Tab = "trim" | "adjust" | "filters" | "frame" | "sound";

const TABS = (): [Tab, string][] => [
  ["trim", t("editor.trim")],
  ["adjust", t("editor.adjust")],
  ["filters", t("editor.filters")],
  ["frame", t("editor.frame")],
  ["sound", t("editor.sound")],
];

const RATIOS = (): { label: string; value: number | null }[] => [
  { label: t("editor.original"), value: null },
  { label: "16:9", value: 16 / 9 },
  { label: "9:16", value: 9 / 16 },
  { label: "1:1", value: 1 },
  { label: "4:3", value: 4 / 3 },
  { label: "3:4", value: 3 / 4 },
  { label: "4:5", value: 4 / 5 },
];

const RESOLUTIONS = [2160, 1440, 1080, 720, 480, 360];
const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];
const FILLS = ["#000000", "#ffffff", ...COLORS.filter((c) => c !== "#000000" && c !== "#ffffff")];
const CORNERS: Handle[] = ["nw", "ne", "sw", "se"];
/** Plus grand côté de l'aperçu (le rendu final se fait en pleine résolution par ffmpeg). */
const PREVIEW_MAX = 960;

const even = (v: number) => Math.max(2, Math.round(v / 2) * 2);

/** Dimensions après rotation, après cadre et après résolution (miroir de `edit::video::sizes`). */
function sizes(info: VideoInfo, s: VideoState, crop: Rect) {
  const rotated = s.geometry.quarter % 2 ? { w: info.height, h: info.width } : { w: info.width, h: info.height };
  let framed = { w: even(rotated.w), h: even(rotated.h) };
  if (s.ratio && s.method === "crop") framed = { w: even(crop.w), h: even(crop.h) };
  if (s.ratio && s.method === "fit") {
    framed =
      rotated.w / rotated.h > s.ratio
        ? { w: even(rotated.w), h: even(rotated.w / s.ratio) }
        : { w: even(rotated.h * s.ratio), h: even(rotated.h) };
  }
  const short = Math.min(framed.w, framed.h);
  const k = s.shortSide && s.shortSide < short ? s.shortSide / short : 1;
  const output = k < 1 ? { w: even(framed.w * k), h: even(framed.h * k) } : framed;
  return { rotated, framed, output };
}

/** Assombrissement des bords : même courbe que le shader (smoothstep 0,35 → 1 vers les coins). */
function vignetteGradient(strength: number): string {
  const stops = [0.35, 0.5, 0.65, 0.8, 0.9, 1].map((d) => {
    const t = (d - 0.35) / 0.65;
    const a = strength * t * t * (3 - 2 * t);
    return `rgba(0, 0, 0, ${a.toFixed(3)}) ${Math.round(d * 100)}%`;
  });
  return `radial-gradient(ellipse farthest-corner at center, ${stops.join(", ")})`;
}

/**
 * Éditeur vidéo : découpe (mêmes outils que l'audio), réglages et filtres de l'éditeur image,
 * rotation/miroir, cadre (ratio recadré ou ajusté), résolution, vitesse et son.
 * L'aperçu passe par le même rendu WebGL que l'éditeur image ; ffmpeg applique les mêmes
 * formules à l'enregistrement.
 */
export default function VideoEditor({ path, onSaved, footerExtra }: Props) {
  const history = useHistory<VideoState>(DEFAULT);
  const { state, update } = history;
  const [info, setInfo] = useState<VideoInfo | null>(null);
  const [track, setTrack] = useState<Track | null>(null);
  const [renderer, setRenderer] = useState<Renderer | null>(null);
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
  const [tab, setTab] = useState<Tab>("trim");
  const [area, setArea] = useState({ w: 0, h: 0 });
  const [busy, setBusy] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const flow = useSaveFlow();
  const [hint, setHint] = useState<string | null>(() => t("editor.loading_video"));
  const [video] = useState(() => {
    const v = document.createElement("video");
    // Le protocole asset renvoie les en-têtes CORS : l'image reste lisible par WebGL.
    v.crossOrigin = "anonymous";
    v.preload = "auto";
    v.playsInline = true;
    return v;
  });
  const areaRef = useRef<HTMLDivElement>(null);
  const bgRef = useRef<HTMLCanvasElement>(null);
  const drag = useRef<{ handle: Handle; x: number; y: number; start: Rect } | null>(null);

  const updateSegments = (fn: (s: Segment[]) => Segment[], key?: string) =>
    update((s) => ({ ...s, segments: fn(s.segments) }), key);
  const duration = track?.duration ?? info?.duration ?? 0;
  const trimmer = useTrimmer(video, duration, state.segments, updateSegments, setError, state.volume / 100);

  // Chargement : infos, forme d'onde du son, lecteur (copie lisible si le conteneur ne l'est pas).
  useEffect(() => {
    let cancelled = false;
    let main: Renderer | null = null;
    let fellBack = false;
    let firstFrame = true;

    const play = (file: string) => {
      video.src = convertFileSrc(file);
    };
    video.onerror = () => {
      if (cancelled) return;
      if (fellBack) {
        setHint(null);
        return setError(t("editor.video_cant_be_played"));
      }
      fellBack = true;
      setHint(t("editor.preparing_playable_preview"));
      invoke<string>("video_preview", { path })
        .then((p) => !cancelled && play(p))
        .catch((e) => setError(describe(e).message));
    };
    video.onloadeddata = () => {
      if (cancelled || main) return;
      main = new Renderer();
      main.setImage(video, PREVIEW_MAX);
      setRenderer(main);
      setHint(null);
      // Vignettes des filtres sur une image un peu après le début (la première est souvent noire).
      video.currentTime = Math.min(2, video.duration * 0.1 || 0);
    };
    video.onseeked = () => {
      if (!firstFrame || cancelled) return;
      firstFrame = false;
      setThumbs(presetThumbs(video));
      video.currentTime = 0;
    };

    invoke<string>("allow_media", { path })
      .then((p) => !cancelled && play(p))
      .catch((e) => setError(describe(e).message));
    Promise.all([invoke<VideoInfo>("video_info", { path }), loadTrack(path)])
      .then(([i, t]) => {
        if (cancelled) return;
        setInfo(i);
        setTrack(t);
        history.reset({ ...DEFAULT, segments: [{ start: 0, end: t.duration }] });
      })
      .catch((e) => setError(describe(e).message));

    return () => {
      cancelled = true;
      video.pause();
      video.onerror = video.onloadeddata = video.onseeked = null;
      video.removeAttribute("src");
      video.load();
      main?.dispose();
    };
  }, [path]);

  // Réglages du lecteur : vitesse (hauteur du son conservée), son coupé ; le volume passe
  // par le gain de la découpe (jusqu'à 200 %).
  useEffect(() => {
    video.playbackRate = state.speed;
    video.muted = state.mute;
  }, [state.speed, state.mute]);

  // Taille de la zone d'aperçu.
  useLayoutEffect(() => {
    const el = areaRef.current;
    if (!el) return;
    const update = () => setArea({ w: el.clientWidth, h: el.clientHeight });
    update();
    const observer = new ResizeObserver(update);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const looks = combined({ ...EDIT_DEFAULT, adjust: state.adjust, preset: state.preset });
  // Le vignettage s'applique au cadre final (après recadrage) : affiché par-dessus, pas dans le rendu.
  const params = { ...looks, adjust: { ...looks.adjust, vignette: 0 }, invert: state.invert, geometry: state.geometry };
  const paramsRef = useRef(params);
  paramsRef.current = params;

  /** Image courante → rendu WebGL → fond flou éventuel. */
  function draw() {
    if (!renderer || video.readyState < 2) return;
    renderer.updateImage(video);
    renderer.render(paramsRef.current);
    const bg = bgRef.current;
    if (bg) {
      const ctx = bg.getContext("2d")!;
      const c = renderer.canvas;
      const k = Math.max(bg.width / c.width, bg.height / c.height);
      ctx.drawImage(c, (bg.width - c.width * k) / 2, (bg.height - c.height * k) / 2, c.width * k, c.height * k);
    }
  }

  // Le canvas WebGL est inséré tel quel dans la scène, dès que son conteneur existe
  // (il n'apparaît qu'une fois les dimensions de la vidéo connues).
  const holderRef = (el: HTMLDivElement | null) => {
    if (el && renderer && renderer.canvas.parentElement !== el) el.appendChild(renderer.canvas);
  };

  useLayoutEffect(draw, [renderer, JSON.stringify(params), state.ratio, state.method, state.fill, tab]);
  useEffect(() => {
    video.addEventListener("seeked", draw);
    return () => video.removeEventListener("seeked", draw);
  });
  useEffect(() => {
    if (!trimmer.playing) return;
    let frame = 0;
    const loop = () => {
      draw();
      frame = requestAnimationFrame(loop);
    };
    frame = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(frame);
  }, [trimmer.playing, renderer]);

  const rotated = info && (state.geometry.quarter % 2 ? { w: info.height, h: info.width } : { w: info.width, h: info.height });
  const crop = (rotated && state.crop) ?? (rotated ? fitRatio(rotated, state.ratio) : { x: 0, y: 0, w: 1, h: 1 });
  const dims = info ? sizes(info, state, crop) : null;
  const editingCrop = tab === "frame" && state.ratio !== null && state.method === "crop";

  // Mise en page de l'aperçu, en pixels de la vidéo pivotée : cadre final et position de l'image.
  let box = { w: 1, h: 1, x: 0, y: 0 };
  if (rotated && dims) {
    if (state.ratio && state.method === "crop" && !editingCrop) box = { w: crop.w, h: crop.h, x: -crop.x, y: -crop.y };
    else if (state.ratio && state.method === "fit") {
      const { w, h } = dims.framed;
      box = { w, h, x: (w - rotated.w) / 2, y: (h - rotated.h) / 2 };
    } else box = { w: rotated.w, h: rotated.h, x: 0, y: 0 };
  }
  const scale = Math.max(0.01, Math.min((area.w - 32) / box.w, (area.h - 32) / box.h));
  const fit = state.ratio !== null && state.method === "fit";

  const setGeometry = (fn: (g: Geometry) => Geometry, key?: string) =>
    update((s) => ({ ...s, geometry: fn(s.geometry), crop: null }), key);

  function onCropDown(e: React.PointerEvent, handle: Handle) {
    e.stopPropagation();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { handle, x: e.clientX, y: e.clientY, start: crop };
  }
  function onCropMove(e: React.PointerEvent) {
    const d = drag.current;
    if (!d || !rotated) return;
    const rect = dragRect(d.start, d.handle, (e.clientX - d.x) / scale, (e.clientY - d.y) / scale, rotated, state.ratio);
    update((s) => ({ ...s, crop: rect }), "crop");
  }

  async function save(duplicates?: Duplicates) {
    if (!info || !rotated || !state.segments.length || busy !== null) return;
    const run = await flow.begin(duplicates);
    if (!run) return;
    video.pause();
    setBusy(0);
    setError(null);
    const { adjust, sepia, fade } = combined({ ...EDIT_DEFAULT, adjust: state.adjust, preset: state.preset });
    const frame =
      state.ratio === null
        ? { mode: "original" }
        : state.method === "crop"
          ? { mode: "crop", rect: { x: crop.x / rotated.w, y: crop.y / rotated.h, w: crop.w / rotated.w, h: crop.h / rotated.h } }
          : { mode: "fit", ratio: state.ratio, color: state.fill };
    const edit = {
      segments: state.segments,
      adjust,
      sepia,
      fade,
      invert: state.invert,
      geometry: state.geometry,
      frame,
      shortSide: state.shortSide,
      speed: state.speed,
      volume: state.volume / 100,
      mute: state.mute,
      normalize: state.normalize,
      fadeIn: state.fadeIn,
      fadeOut: state.fadeOut,
    };
    const unlisten = await listen<number>("edit-progress", ({ payload }) => setBusy(payload));
    try {
      const output = await invoke<string>("edit_video", { path, edit, run });
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

  const ready = !!track && !!info;
  const status =
    error ??
    hint ??
    (busy !== null
      ? `${t("editor.saving")} ${Math.round(busy * 100)} %`
      : ready && dims
        ? `${t("editor.result")}${keptSummary(state.segments, duration, state.speed)} · ${dims.output.w} × ${dims.output.h} px`
        : t("editor.analysing_video"));
  const shortSource = dims ? Math.min(dims.framed.w, dims.framed.h) : 0;

  return (
    <div className="editor audio-editor video-editor">
      <div className="audio-area video-area">
        <div className="video-stage" ref={areaRef} onPointerMove={onCropMove} onPointerUp={() => (drag.current = null)}>
          {renderer && rotated && (
            <div
              className="video-box"
              style={{ width: box.w * scale, height: box.h * scale, background: fit && state.fill ? state.fill : undefined }}
            >
              {fit && !state.fill && (
                <canvas ref={bgRef} className="video-bg" width={Math.round(box.w / 8)} height={Math.round(box.h / 8)} />
              )}
              <div
                ref={holderRef}
                className="video-frame"
                style={{ left: box.x * scale, top: box.y * scale, width: rotated.w * scale, height: rotated.h * scale }}
              />
              {editingCrop ? (
                <div
                  className="crop-box"
                  style={{ left: crop.x * scale, top: crop.y * scale, width: crop.w * scale, height: crop.h * scale }}
                  onPointerDown={(e) => onCropDown(e, "move")}
                >
                  <div className="crop-grid" />
                  {looks.adjust.vignette > 0 && (
                    <div className="video-vignette" style={{ background: vignetteGradient(looks.adjust.vignette / 100) }} />
                  )}
                  {CORNERS.map((h) => (
                    <div key={h} className={`crop-handle h-${h}`} onPointerDown={(e) => onCropDown(e, h)} />
                  ))}
                </div>
              ) : (
                looks.adjust.vignette > 0 && (
                  <div className="video-vignette" style={{ background: vignetteGradient(looks.adjust.vignette / 100) }} />
                )
              )}
            </div>
          )}
          {!renderer && <p className="tool-hint">{error ? "" : (hint ?? t("editor.loading"))}</p>}
        </div>

        <TrimTransport trimmer={trimmer} history={history} ready={ready} />
        {track && <TrimTimeline trimmer={trimmer} peaks={track.peaks} />}
      </div>

      <aside className="tool-panel editor-panel">
        <EditorTabs tabs={TABS()} value={tab} onChange={setTab} className="video-tabs" />

        {tab === "trim" && (
          <>
            <section>
              <h2>{t("editor.kept_sections")}</h2>
              <SegmentList trimmer={trimmer} ready={ready} />
            </section>
            <section>
              <ul className="audio-help">
                <li>
                  {t("editor.drag_waveform_select_then")}
                  <kbd>{t("editor.delete")}</kbd>
                  {t("editor.remove_that_section")}
                </li>
                <li>{t("editor.drag_purple_edges_adjust_start")}</li>
                <li>
                  <kbd>S</kbd>
                  {t("editor.cuts_at_cursor_playback_skips")}
                </li>
              </ul>
            </section>
          </>
        )}

        {tab === "adjust" && (
          <section>
            {SLIDERS.map(({ key, label, min }) => (
              <Slider
                key={key}
                label={label}
                min={min}
                max={100}
                value={state.adjust[key]}
                onChange={(v) => update((s) => ({ ...s, adjust: { ...s.adjust, [key]: v } }), `adjust.${key}`)}
              />
            ))}
            <label className="check">
              <input type="checkbox" checked={state.invert} onChange={(e) => update((s) => ({ ...s, invert: e.target.checked }))} />
              {t("editor.invert_colours")}
            </label>
            <button className="link" onClick={() => update((s) => ({ ...s, adjust: ADJUST_DEFAULT, invert: false }))}>
              {t("editor.reset_adjustments")}
            </button>
          </section>
        )}

        {tab === "filters" && (
          <section>
            <div className="presets">
              {PRESETS.map((p) => (
                <button
                  key={p.id}
                  className={`preset ${state.preset === p.id ? "is-active" : ""}`}
                  onClick={() => update((s) => ({ ...s, preset: p.id }))}
                >
                  {thumbs[p.id] ? <img src={thumbs[p.id]} alt="" /> : <span className="preset-blank" />}
                  <span>{p.label}</span>
                </button>
              ))}
            </div>
            <p className="tool-hint">
              {t("editor.filter_adds_adjustments_fine_tune")}
            </p>
          </section>
        )}

        {tab === "frame" && (
          <>
            <section>
              <h2>{t("editor.rotation")}</h2>
              <div className="editor-toolbar video-rotate">
                <button title={t("editor.rotate_left")} onClick={() => setGeometry((g) => ({ ...g, quarter: (g.quarter + 3) % 4 }))}>
                  <RotateLeftIcon />
                </button>
                <button title={t("editor.rotate_right")} onClick={() => setGeometry((g) => ({ ...g, quarter: (g.quarter + 1) % 4 }))}>
                  <RotateRightIcon />
                </button>
                <button title={t("editor.flip_horizontally")} onClick={() => setGeometry((g) => ({ ...g, flipH: !g.flipH }))}>
                  <FlipHIcon />
                </button>
                <button title={t("editor.flip_vertically")} onClick={() => setGeometry((g) => ({ ...g, flipV: !g.flipV }))}>
                  <FlipVIcon />
                </button>
              </div>
              <Slider
                label={t("editor.straighten")}
                min={-45}
                max={45}
                step={0.5}
                unit="°"
                value={state.geometry.straighten}
                onChange={(v) => update((s) => ({ ...s, geometry: { ...s.geometry, straighten: v } }), "straighten")}
              />
            </section>

            <section>
              <h2>{t("editor.aspect_ratio")}</h2>
              <div className="chips">
                {RATIOS().map((r) => (
                  <button
                    key={r.label}
                    className={`chip ${state.ratio === r.value ? "is-active" : ""}`}
                    onClick={() => update((s) => ({ ...s, ratio: r.value, crop: null }))}
                  >
                    {r.label}
                  </button>
                ))}
              </div>
              {state.ratio !== null && (
                <div className="chips video-method">
                  {(
                    [
                      ["crop", t("editor.crop")],
                      ["fit", t("editor.fit")],
                    ] as const
                  ).map(([id, label]) => (
                    <button
                      key={id}
                      className={`chip ${state.method === id ? "is-active" : ""}`}
                      onClick={() => update((s) => ({ ...s, method: id }))}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              )}
              {state.ratio !== null && state.method === "crop" && (
                <p className="tool-hint">{t("editor.move_frame_preview_choose_kept")}</p>
              )}
              {fit && (
                <>
                  <p className="tool-hint">{t("editor.whole_picture_kept_bars_are")}</p>
                  <div className="swatches video-fills">
                    <button
                      className={`chip ${state.fill === null ? "is-active" : ""}`}
                      onClick={() => update((s) => ({ ...s, fill: null }))}
                    >
                      {t("editor.blur")}
                    </button>
                    {FILLS.map((c) => (
                      <button
                        key={c}
                        className={`swatch ${state.fill === c ? "is-active" : ""}`}
                        style={{ background: c }}
                        aria-label={c}
                        onClick={() => update((s) => ({ ...s, fill: c }))}
                      />
                    ))}
                  </div>
                </>
              )}
            </section>

            <section>
              <h2>{t("editor.resolution")}</h2>
              <div className="chips">
                <button
                  className={`chip ${state.shortSide === null ? "is-active" : ""}`}
                  onClick={() => update((s) => ({ ...s, shortSide: null }))}
                >
                  {t("editor.original")}
                </button>
                {RESOLUTIONS.filter((r) => r < shortSource).map((r) => (
                  <button
                    key={r}
                    className={`chip ${state.shortSide === r ? "is-active" : ""}`}
                    onClick={() => update((s) => ({ ...s, shortSide: r }))}
                  >
                    {r}p
                  </button>
                ))}
              </div>
              {dims && (
                <p className="tool-hint">
                  {t("editor.output")}
                  {dims.output.w} × {dims.output.h} px ({t("editor.original_2")} {info!.width} × {info!.height})
                </p>
              )}
            </section>
          </>
        )}

        {tab === "sound" && (
          <>
            <section>
              <h2>{t("editor.speed")}</h2>
              <div className="chips video-speeds">
                {SPEEDS.map((v) => (
                  <button key={v} className={`chip ${state.speed === v ? "is-active" : ""}`} onClick={() => update((s) => ({ ...s, speed: v }))}>
                    {v.toLocaleString(locale())}×
                  </button>
                ))}
              </div>
              <p className="tool-hint">{t("editor.sound_keeps_normal_pitch")}</p>
            </section>
            <section>
              <h2>{t("editor.sound")}</h2>
              {info && !info.hasAudio ? (
                <p className="tool-hint">{t("editor.video_has_no_sound")}</p>
              ) : (
                <>
                  <label className="check">
                    <input type="checkbox" checked={state.mute} onChange={(e) => update((s) => ({ ...s, mute: e.target.checked }))} />
                    {t("editor.mute")}
                  </label>
                  {!state.mute && (
                    <>
                      <Slider
                        label="Volume"
                        min={0}
                        max={200}
                        neutral={100}
                        unit=" %"
                        value={state.volume}
                        onChange={(v) => update((s) => ({ ...s, volume: v }), "volume")}
                      />
                      <label className="check">
                        <input
                          type="checkbox"
                          checked={state.normalize}
                          onChange={(e) => update((s) => ({ ...s, normalize: e.target.checked }))}
                        />
                        {t("editor.normalise_loudness_ebu_r128")}
                      </label>
                      <Slider
                        label={t("editor.fade")}
                        min={0}
                        max={5}
                        step={0.1}
                        unit=" s"
                        value={state.fadeIn}
                        onChange={(v) => update((s) => ({ ...s, fadeIn: v }), "fadeIn")}
                      />
                      <Slider
                        label={t("editor.fade_out")}
                        min={0}
                        max={5}
                        step={0.1}
                        unit=" s"
                        value={state.fadeOut}
                        onChange={(v) => update((s) => ({ ...s, fadeOut: v }), "fadeOut")}
                      />
                      <p className="tool-hint">
                        {t("editor.normalisation_fades_are_applied_when")}
                      </p>
                    </>
                  )}
                </>
              )}
            </section>
          </>
        )}
      </aside>

      <footer className="tool-footer">
        <span className={`tool-status ${error ? "is-error" : ""}`}>{status}</span>
        <SaveExtras flow={flow} />
        {footerExtra}
        <button className="primary" onClick={() => save()} disabled={!ready || !state.segments.length || busy !== null}>
          {busy !== null ? t("editor.saving") : t("editor.save")}
        </button>
      </footer>
    </div>
  );
}
