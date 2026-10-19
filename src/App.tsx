import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/** Fenêtre principale. En attendant le hook Shift+drag (M3) et les réglages (M7),
 *  elle sert de déclencheur de dev : déposer des fichiers ouvre le menu radial. */
function App() {
  const [over, setOver] = useState(false);

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
      <h1>Morph</h1>
      <p>Mode dev : glisse un ou plusieurs fichiers ici pour ouvrir le menu radial des formats.</p>
      <div className={`dropzone ${over ? "is-over" : ""}`}>Dépose tes fichiers ici</div>
    </main>
  );
}

export default App;
