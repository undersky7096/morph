import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import { basename } from "../formats";
import { ChevronIcon, CloseIcon, MinimizeIcon } from "../icons";
import Crop from "./crop/Crop";
import { TOOLS, type ToolId, toolsFor } from "./registry";
import "./tools.css";

interface Session {
  tool: ToolId;
  /** Fichier en cours : l'original, puis le résultat du dernier outil appliqué. */
  path: string;
  /** Dernier fichier enregistré par un outil dans cette fenêtre. */
  saved: string | null;
}

/** Popup d'édition : un outil à la fois, enchaînables via « Outil suivant ». */
export default function ToolWindow() {
  const [session, setSession] = useState<Session | null>(null);

  useEffect(() => {
    const start = (p: { tool: ToolId; path: string }) => setSession({ ...p, saved: null });
    invoke<{ tool: ToolId; path: string } | null>("take_tool_payload").then((p) => p && start(p));
    const unlisten = getCurrentWebviewWindow().listen<{ tool: ToolId; path: string }>("tool-open", ({ payload }) => {
      invoke("take_tool_payload");
      start(payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (session) document.title = `${TOOLS[session.tool].label} : ${basename(session.path)} · Morph`;
  }, [session]);

  const win = getCurrentWebviewWindow();
  const tool = session ? TOOLS[session.tool] : null;

  const nextTool = session && (
    <NextTool
      current={session.tool}
      path={session.path}
      onPick={(id) => setSession((s) => s && { ...s, tool: id })}
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
              {basename(session!.path)}
            </span>
          </span>
        )}
        {session?.saved && (
          <span className="titlebar-saved" title={session.saved}>
            ✓ Enregistré : {basename(session.saved)}
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
            key={session.path}
            path={session.path}
            nextTool={nextTool}
            onSaved={(output) => setSession((s) => s && { ...s, path: output, saved: output })}
          />
        )}
      </main>
    </div>
  );
}

/** Bouton « Outil suivant » : passe à un autre outil sur le fichier en cours. */
function NextTool(props: { current: ToolId; path: string; onPick: (id: ToolId) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const options = toolsFor([props.path]).filter((id) => id !== props.current);

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
