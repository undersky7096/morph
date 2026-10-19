import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import Halo from "./halo/Halo";
import Radial from "./radial/Radial";
import "./App.css";

// Une seule page pour toutes les fenêtres : on choisit l'interface selon le label.
const label = getCurrentWindow().label;
document.documentElement.dataset.window = label;

const views: Record<string, () => React.JSX.Element> = { radial: Radial, halo: Halo };
const View = views[label] ?? App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <View />
  </React.StrictMode>,
);
