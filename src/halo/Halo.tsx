import { useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import "./Halo.css";

/** Halo qui suit le curseur pendant un Shift+glisser au-dessus de la zone de dépôt Morph. */
export default function Halo() {
  // Relance l'animation d'apparition à chaque nouveau survol ; null quand le halo est masqué.
  const [key, setKey] = useState<number | null>(null);

  useEffect(() => {
    const win = getCurrentWebviewWindow();
    const unlisten = [
      win.listen("halo-show", () => setKey((k) => (k ?? 0) + 1)),
      // Masqué : plus rien à dessiner, l'animation en boucle s'arrête.
      win.listen("halo-hide", () => setKey(null)),
    ];
    return () => {
      unlisten.forEach((p) => p.then((fn) => fn()));
    };
  }, []);

  if (key === null) return null;
  return (
    <div className="halo" key={key}>
      <div className="halo-glow" />
      <div className="halo-ring" />
      <span className="halo-mark">M</span>
    </div>
  );
}
