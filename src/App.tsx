import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/** Fenêtre principale. En attendant le hook Shift+drag (M3) et les réglages (M7),
 *  elle sert de déclencheur de dev : déposer des fichiers ouvre le menu radial. */
function App() {
  const [over, setOver] = useState(false);
  const [version, setVersion] = useState("");

  useEffect(() => {
    getVersion().then(setVersion);
  }, []);

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (payload.type === "enter" || payload.type === "over") setOver(true);
      else if (payload.type === "leave") setOver(false);
      else if (payload.type === "drop") {
        setOver(false);
        if (payload.paths.length) invoke("open_radial", { paths: payload.paths });
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <main className="app">
      <h1>
        Morph <span className="version">v{version}</span>
      </h1>
      <p>
        Depuis l’explorateur ou le bureau, glisse des fichiers en maintenant <kbd>Shift</kbd> et lâche-les
        n’importe où : le menu des formats s’ouvre. (Dépôt direct ici aussi possible.)
      </p>
      <div className={`dropzone ${over ? "is-over" : ""}`}>Dépose tes fichiers ici</div>
    </main>
  );
}

export default App;
