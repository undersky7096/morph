import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, type Variants } from "motion/react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import { chooseOutputDir, getSettings } from "../settings";
import { comingIn, TOOLS } from "../tools/registry";
import type { ConvertProgress, ConvertResult, RadialPayload } from "../types";
import { actionItems, buildItems, type Item } from "./items";
import "./Radial.css";

const RADIUS = 118;
const HUB_RING = 2 * Math.PI * 34;

/** Niveau du menu. `done` : légende du ✓ quand l'anneau d'actions suit un traitement réussi. */
type Level = { label: string; items: Item[]; done?: string };
type Phase =
  | { name: "menu" }
  | { name: "running"; progress: number }
  | { name: "done"; ok: number; errors: string[] };

const spring = { type: "spring", stiffness: 520, damping: 32 } as const;

/** Entrée / sortie d'un anneau de pétales ; `dir` : +1 vers un sous-menu, −1 retour. */
const RING: Variants = {
  enter: (dir: number) => ({ rotate: -18 * dir, opacity: 0 }),
  center: { rotate: 0, opacity: 1, transition: { type: "spring", stiffness: 260, damping: 26 } },
  leave: (dir: number) => ({ rotate: 14 * dir, opacity: 0, transition: { duration: 0.12, ease: "easeIn" } }),
};

export default function Radial() {
  const [open, setOpen] = useState(false);
  const [session, setSession] = useState(0);
  const [fileCount, setFileCount] = useState(0);
  const [stack, setStackState] = useState<Level[]>([]);
  /** Anneau affiché (change à chaque niveau) et sens du passage : +1 vers un sous-menu, −1 retour. */
  const [ring, setRing] = useState({ id: 0, dir: 1 });
  const setStack = (next: Level[] | ((s: Level[]) => Level[]), dir = 1) => {
    setStackState(next);
    setRing((r) => ({ id: r.id + 1, dir }));
  };
  const [phase, setPhase] = useState<Phase>({ name: "menu" });
  const openRef = useRef(false);
  const sessionRef = useRef(0);
  const progressRef = useRef<number[]>([]);
  /** Choix du dossier de sortie en cours : la perte de focus ne ferme pas le menu. */
  const pickingRef = useRef(false);
  /** La fenêtre a obtenu le focus depuis l'ouverture du menu. */
  const focusedRef = useRef(false);

  openRef.current = open;
  sessionRef.current = session;

  const close = useCallback(() => setOpen(false), []);

  useEffect(() => {
    let lastId = 0;
    const openWith = (payload: RadialPayload) => {
      // Une même demande peut arriver par l'événement et par take_radial_payload.
      if (payload.id <= lastId) return;
      lastId = payload.id;
      setFileCount(payload.paths.length);
      setStack([
        payload.actions
          ? { label: "", items: actionItems(payload.paths), done: "Enregistré" }
          : { label: "", items: buildItems(payload.menu, getSettings()) },
      ]);
      setPhase({ name: "menu" });
      setSession((s) => s + 1);
      focusedRef.current = false;
      setOpen(true);
    };

    const win = getCurrentWebviewWindow();
    const opened = win.listen<RadialPayload>("radial-open", ({ payload }) => openWith(payload));
    // Une fois abonné, on récupère une demande arrivée avant que ce webview ne soit prêt
    // (ex : lancement avec des fichiers en arguments).
    opened.then(() => invoke<RadialPayload | null>("take_radial_payload")).then((p) => p && openWith(p));

    const unlisten = [
      opened,
      // Menu non modal : il se ferme dès que la fenêtre perd le focus (clic ailleurs) —
      // seulement s'il l'a eu : si Windows le refuse à l'ouverture, le menu reste affiché.
      win.onFocusChanged(({ payload: focused }) => {
        if (focused) focusedRef.current = true;
        else if (focusedRef.current && !pickingRef.current) setOpen(false);
      }),
      win.listen<ConvertProgress>("convert-progress", ({ payload }) => {
        const values = progressRef.current;
        if (payload.index >= values.length) return;
        values[payload.index] = payload.progress;
        const total = values.reduce((a, b) => a + b, 0) / values.length;
        setPhase((p) => (p.name === "running" ? { name: "running", progress: total } : p));
      }),
    ];
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("keydown", onKey);
    return () => {
      unlisten.forEach((p) => p.then((fn) => fn()));
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const level = stack[stack.length - 1];
  const items = level?.items ?? [];

  // Rien à proposer (fichiers non supportés) : on affiche le message puis on ferme.
  useEffect(() => {
    if (open && phase.name === "menu" && stack.length === 1 && items.length === 0) {
      const t = setTimeout(close, 1800);
      return () => clearTimeout(t);
    }
  }, [open, phase.name, stack.length, items.length, close]);

  async function choose(item: Item) {
    if (item.type === "group") {
      setStack((s) => [...s, { label: item.label, items: item.items }]);
      return;
    }
    if (item.type === "action") {
      if (comingIn(item.tool, item.paths)) return;
      await invoke("open_tool", { tool: item.tool, paths: item.paths });
      close();
      return;
    }
    // Réglage « demander à chaque fois » : le menu reste ouvert pendant le choix du dossier.
    pickingRef.current = getSettings().saveMode === "ask";
    const outDir = await chooseOutputDir();
    if (pickingRef.current) {
      pickingRef.current = false;
      getCurrentWebviewWindow().setFocus();
    }
    if (outDir === null) return;
    const mySession = sessionRef.current;
    progressRef.current = item.paths.map(() => 0);
    setPhase({ name: "running", progress: 0 });
    const results = await invoke<ConvertResult[]>("convert_files", {
      paths: item.paths,
      target: item.format,
      outDir,
    });
    if (sessionRef.current !== mySession) return;
    const errors = results.filter((r) => r.error).map((r) => `${r.input}\n${r.error}`);
    const outputs = results.flatMap((r) => (r.output ? [r.output] : []));
    const actions = errors.length ? [] : actionItems(outputs);
    if (actions.length) {
      // Conversion réussie : le menu enchaîne sur les actions d'édition du résultat.
      setStack([{ label: "", items: actions, done: "Converti" }]);
      setPhase({ name: "menu" });
      return;
    }
    setPhase({ name: "done", ok: results.length - errors.length, errors });
    setTimeout(() => sessionRef.current === mySession && close(), errors.length ? 4000 : 1300);
  }

  function onHub() {
    if (phase.name === "menu" && stack.length > 1) setStack((s) => s.slice(0, -1), -1);
    else close();
  }

  return (
    <AnimatePresence
      mode="wait"
      onExitComplete={() => {
        if (!openRef.current) invoke("close_radial");
      }}
    >
      {open && (
        <motion.div
          key={session}
          className="radial"
          initial={{ opacity: 0, scale: 0.8 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.85 }}
          transition={{ duration: 0.18, ease: "easeOut" }}
          onClick={(e) => e.target === e.currentTarget && close()}
          // Menu ouvert sans focus (refusé par Windows) : le premier clic le lui donne,
          // pour qu'un clic ailleurs le referme ensuite.
          onPointerDown={() => !focusedRef.current && getCurrentWebviewWindow().setFocus()}
        >
          <div className="radial-disc" />

          {/* Changement de niveau : l'anneau tourne légèrement dans le sens du passage
              pendant que les pétales rentrent puis ressortent du moyeu. */}
          <AnimatePresence mode="wait" custom={ring.dir}>
            {phase.name === "menu" && (
              <motion.div
                key={ring.id}
                className="radial-ring"
                custom={ring.dir}
                variants={RING}
                initial="enter"
                animate="center"
                exit="leave"
              >
                {items.map((item, i) => {
                  const soon = item.type === "action" ? comingIn(item.tool, item.paths) : undefined;
                  const angle = -Math.PI / 2 + (i / items.length) * 2 * Math.PI;
                  const x = Math.cos(angle) * RADIUS;
                  const y = Math.sin(angle) * RADIUS;
                  return (
                    <motion.button
                      key={itemKey(item)}
                      className={`petal petal-${item.type}`}
                      data-kind={item.kind}
                      aria-disabled={soon ? true : undefined}
                      title={itemTitle(item)}
                      initial={{ x: 0, y: 0, scale: 0.3, opacity: 0 }}
                      animate={{ x, y, scale: 1, opacity: 1, transition: { ...spring, delay: i * 0.02 } }}
                      exit={{ x: x * 0.35, y: y * 0.35, scale: 0.5, opacity: 0, transition: { duration: 0.11, ease: "easeIn" } }}
                      whileHover={soon ? undefined : { scale: 1.14 }}
                      whileTap={soon ? undefined : { scale: 0.94 }}
                      transition={spring}
                      onClick={() => choose(item)}
                    >
                      <PetalContent item={item} />
                    </motion.button>
                  );
                })}
              </motion.div>
            )}
          </AnimatePresence>

          <motion.button
            className={`hub hub-${phase.name}`}
            onClick={onHub}
            whileTap={{ scale: 0.95 }}
            title={phase.name === "done" && phase.errors.length ? phase.errors.join("\n\n") : undefined}
          >
            <AnimatePresence mode="wait" initial={false}>
              <motion.span
                key={hubKey(phase, level)}
                className="hub-content"
                initial={{ opacity: 0, scale: 0.7 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.7 }}
                transition={{ duration: 0.12 }}
              >
                <HubContent phase={phase} fileCount={fileCount} level={level} empty={items.length === 0} />
              </motion.span>
            </AnimatePresence>
          </motion.button>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function HubContent(props: { phase: Phase; fileCount: number; level?: Level; empty: boolean }) {
  const { phase, fileCount, level, empty } = props;

  if (phase.name === "running") {
    return (
      <>
        <svg className="hub-progress" viewBox="0 0 80 80">
          <circle cx="40" cy="40" r="34" className="track" />
          <circle
            cx="40"
            cy="40"
            r="34"
            className="bar"
            strokeDasharray={HUB_RING}
            strokeDashoffset={HUB_RING * (1 - phase.progress)}
          />
        </svg>
        <span className="hub-value">{Math.round(phase.progress * 100)}%</span>
      </>
    );
  }
  if (phase.name === "done") {
    const failed = phase.errors.length > 0;
    return (
      <>
        <span className={`hub-icon ${failed ? "is-error" : "is-ok"}`}>{failed ? "!" : "✓"}</span>
        <span className="hub-caption">
          {failed ? `${phase.errors.length} erreur${phase.errors.length > 1 ? "s" : ""}` : "Terminé"}
        </span>
      </>
    );
  }
  if (level?.done) {
    return (
      <>
        <span className="hub-icon is-ok">✓</span>
        <span className="hub-caption">{level.done}</span>
      </>
    );
  }
  if (empty) return <span className="hub-caption">Format non pris en charge</span>;
  if (level && level.label) {
    return (
      <>
        <span className="hub-icon">←</span>
        <span className="hub-caption">{level.label}</span>
      </>
    );
  }
  return (
    <>
      <span className="hub-value">{fileCount}</span>
      <span className="hub-caption">fichier{fileCount > 1 ? "s" : ""}</span>
    </>
  );
}

/** Contenu du moyeu à fondre d'un état à l'autre (la progression, elle, se met à jour sur place). */
function hubKey(phase: Phase, level?: Level) {
  if (phase.name !== "menu") return phase.name;
  return level?.done ? `done-${level.done}` : `menu-${level?.label ?? ""}`;
}

function itemKey(item: Item) {
  if (item.type === "format") return item.format;
  if (item.type === "action") return item.tool;
  return item.label;
}

function itemTitle(item: Item) {
  if (item.type === "group") return `${item.label} : ${item.items.length} choix`;
  if (item.type === "action") {
    const label = TOOLS[item.tool].label;
    const soon = comingIn(item.tool, item.paths);
    return soon ? `${label} : disponible bientôt (${soon})` : label;
  }
  return undefined;
}

function PetalContent({ item }: { item: Item }) {
  if (item.type === "format") return <>{item.format.toUpperCase()}</>;
  if (item.type === "group" && !item.icon) return <>{item.label} ›</>;
  const Icon = TOOLS[item.type === "group" ? item.icon! : item.tool].icon;
  return (
    <span className="petal-tool">
      <Icon />
      <span>{item.type === "group" ? `${item.label} ›` : TOOLS[item.tool].label}</span>
    </span>
  );
}
