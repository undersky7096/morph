import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import { basename, kindOfPath } from "../formats";
import { CloseIcon, MinimizeIcon } from "../icons";
import Compress from "./compress/Compress";
import Crop from "./crop/Crop";
import { TOOLS, type ToolId } from "./registry";
import "./tools.css";

// Konva est lourd : chargé seulement quand l'éditeur s'ouvre (le menu radial reste léger).
const ImageEditor = lazy(() => import("./edit/ImageEditor"));
const AudioEditor = lazy(() => import("./edit/AudioEditor"));
const VideoEditor = lazy(() => import("./edit/VideoEditor"));

interface Payload {
  tool: ToolId;
  paths: string[];
}

interface Session extends Payload {
  /** Change à chaque ouverture : remonte l'outil à neuf. */
  id: number;
  /** Fichier en cours, pour les outils qui traitent un lot fichier par fichier. */
  index: number;
  /** Fichiers enregistrés jusqu'ici. */
  outputs: string[];
}

let nextId = 0;

/**
 * Popup d'un outil. Après l'enregistrement, on passe au fichier suivant du lot ;
 * une fois tout traité, la fenêtre se ferme et l'anneau d'actions réapparaît
 * sur les résultats. Fermée avant la fin, elle ramène au choix des outils.
 */
export default function ToolWindow() {
  const [session, setSession] = useState<Session | null>(null);
  const win = getCurrentWebviewWindow();
  const sessionRef = useRef(session);
  sessionRef.current = session;
  /** Fermeture après le dernier fichier (les pétales s'ouvrent déjà sur les résultats). */
  const finishingRef = useRef(false);

  useEffect(() => {
    const start = (p: Payload) => {
      finishingRef.current = false;
      setSession({ ...p, id: ++nextId, index: 0, outputs: [] });
    };
    invoke<Payload | null>("take_tool_payload").then((p) => p && start(p));
    const unlisten = win.listen<Payload>("tool-open", ({ payload }) => {
      invoke("take_tool_payload");
      start(payload);
    });
    // Fenêtre fermée (bouton, Alt+F4…) avant la fin : retour aux pétales des outils.
    const closing = win.onCloseRequested(async () => {
      const current = sessionRef.current;
      if (current && !finishingRef.current) await invoke("open_tools", { paths: current.paths });
    });
    return () => {
      unlisten.then((fn) => fn());
      closing.then((fn) => fn());
    };
  }, []);

  const tool = session ? TOOLS[session.tool] : null;
  const perFile = !!tool && !tool.wholeBatch;
  const current = session && (perFile ? [session.paths[session.index]] : session.paths);
  const counter = session && perFile && session.paths.length > 1 ? `${session.index + 1}/${session.paths.length}` : null;

  useEffect(() => {
    if (tool && current) document.title = `${tool.label} : ${describe(current)} · Morph`;
  }, [tool, current?.join("\n")]);

  /** Retour aux pétales sur les résultats, puis fermeture de la fenêtre. */
  async function finish(outputs: string[]) {
    finishingRef.current = true;
    if (outputs.length) await invoke("open_actions", { paths: outputs });
    await win.close();
  }

  /** Fichier en cours traité (`output`) ou passé (`null`) : fichier suivant, ou fin du lot. */
  function advance(output: string | null) {
    if (!session) return;
    const outputs = output ? [...session.outputs, output] : session.outputs;
    if (session.index + 1 < session.paths.length) {
      setSession({ ...session, index: session.index + 1, outputs });
    } else {
      finish(outputs);
    }
  }

  const skip = counter && (
    <button onClick={() => advance(null)} title="Garder ce fichier tel quel et passer au suivant">
      Passer
    </button>
  );
  const key = session && `${session.id}-${session.index}`;

  return (
    <div className="tool">
      <header className="titlebar" data-tauri-drag-region>
        {tool && current && (
          <span className="titlebar-title" data-tauri-drag-region>
            <tool.icon />
            <strong data-tauri-drag-region>{tool.label}</strong>
            <span className="titlebar-file" data-tauri-drag-region>
              {describe(current)}
            </span>
            {counter && <span className="titlebar-counter">{counter}</span>}
          </span>
        )}
        <div className="titlebar-buttons">
          <button aria-label="Réduire" onClick={() => win.minimize()}>
            <MinimizeIcon />
          </button>
          <button aria-label="Fermer" className="is-close" onClick={() => win.close()}>
            <CloseIcon />
          </button>
        </div>
      </header>

      <main className="tool-body">
        {session?.tool === "crop" && (
          <Crop key={key} path={current![0]} footerExtra={skip} onSaved={advance} />
        )}
        {session?.tool === "edit" && (
          <Suspense fallback={<p className="tool-hint tool-loading">Chargement de l'éditeur…</p>}>
            {kindOfPath(current![0]) === "audio" ? (
              <AudioEditor key={key} path={current![0]} footerExtra={skip} onSaved={advance} />
            ) : kindOfPath(current![0]) === "video" ? (
              <VideoEditor key={key} path={current![0]} footerExtra={skip} onSaved={advance} />
            ) : (
              <ImageEditor key={key} path={current![0]} footerExtra={skip} onSaved={advance} />
            )}
          </Suspense>
        )}
        {session?.tool === "compress" && <Compress key={key} paths={session.paths} onDone={finish} />}
      </main>
    </div>
  );
}

const describe = (paths: string[]) => (paths.length === 1 ? basename(paths[0]) : `${paths.length} fichiers`);
