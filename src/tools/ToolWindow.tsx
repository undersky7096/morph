import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import { basename } from "../formats";
import { ChevronIcon, CloseIcon, MinimizeIcon } from "../icons";
import Compress from "./compress/Compress";
import Crop from "./crop/Crop";
import { TOOLS, type ToolId, toolsFor } from "./registry";
import "./tools.css";

interface Payload {
  tool: ToolId;
  paths: string[];
}

interface Session {
  /** Change à chaque ouverture d'outil : remonte l'outil à neuf. */
  id: number;
  tool: ToolId;
  /** Fichiers en cours : les originaux, puis les résultats du dernier outil appliqué. */
  paths: string[];
  /** Derniers fichiers enregistrés par un outil dans cette fenêtre. */
  saved: string[];
}

let nextId = 0;

/** Popup d'édition : un outil à la fois, enchaînables via « Outil suivant ». */
export default function ToolWindow() {
  const [session, setSession] = useState<Session | null>(null);

  useEffect(() => {
    const start = (p: Payload) => setSession({ ...p, id: ++nextId, saved: [] });
    invoke<Payload | null>("take_tool_payload").then((p) => p && start(p));
    const unlisten = getCurrentWebviewWindow().listen<Payload>("tool-open", ({ payload }) => {
      invoke("take_tool_payload");
      start(payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (session) document.title = `${TOOLS[session.tool].label} : ${describe(session.paths)} · Morph`;
  }, [session]);

  const win = getCurrentWebviewWindow();
  const tool = session ? TOOLS[session.tool] : null;
  const onSaved = (outputs: string[]) =>
    setSession((s) => s && { ...s, paths: outputs.length ? outputs : s.paths, saved: outputs });

  const nextTool = session && (
    <NextTool
      current={session.tool}
      paths={session.paths}
      onPick={(id) => setSession((s) => s && { ...s, id: ++nextId, tool: id })}
    />
  );

  return (
    <div className="tool">
      <header className="titlebar" data-tauri-drag-region>
        {tool && (
          <span className="titlebar-title" data-tauri-drag-region>
            <tool.icon />
            <strong data-tauri-drag-region>{tool.label}</strong>
            <span className="titlebar-file" data-tauri-drag-region>
              {describe(session!.paths)}
            </span>
          </span>
        )}
        {session && session.saved.length > 0 && (
          <span className="titlebar-saved" title={session.saved.join("\n")}>
            ✓ Enregistré : {describe(session.saved)}
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
          <Crop
            key={session.paths[0]}
            path={session.paths[0]}
            nextTool={nextTool}
            onSaved={(output) => onSaved([output])}
          />
        )}
        {session?.tool === "compress" && (
          <Compress key={session.id} paths={session.paths} nextTool={nextTool} onSaved={onSaved} />
        )}
      </main>
    </div>
  );
}

const describe = (paths: string[]) => (paths.length === 1 ? basename(paths[0]) : `${paths.length} fichiers`);

/** Bouton « Outil suivant » : passe à un autre outil sur les fichiers en cours. */
function NextTool(props: { current: ToolId; paths: string[]; onPick: (id: ToolId) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const options = toolsFor(props.paths).filter((id) => id !== props.current);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("pointerdown", onDown);
    return () => window.removeEventListener("pointerdown", onDown);
  }, [open]);

  if (options.length === 0) return null;
  return (
    <div className="next-tool" ref={ref}>
      <button onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        Outil suivant <ChevronIcon />
      </button>
      {open && (
        <div className="next-tool-menu" role="menu">
          {options.map((id) => {
            const tool = TOOLS[id];
            return (
              <button
                key={id}
                role="menuitem"
                disabled={!!tool.comingIn}
                onClick={() => {
                  setOpen(false);
                  props.onPick(id);
                }}
              >
                <tool.icon />
                <span>{tool.label}</span>
                {tool.comingIn && <em>bientôt</em>}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
