import { useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import "./Halo.css";

/** Halo qui suit le curseur pendant un Shift+glisser au-dessus de la zone de dépôt Morph. */
export default function Halo() {
  // Relance l'animation d'apparition à chaque nouveau survol.
  const [key, setKey] = useState(0);

  useEffect(() => {
    const unlisten = getCurrentWebviewWindow().listen("halo-show", () => setKey((k) => k + 1));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return (
    <div className="halo" key={key}>
      <div className="halo-glow" />
      <div className="halo-ring" />
      <span className="halo-mark">M</span>
    </div>
  );
}
