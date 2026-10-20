import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App";
import Halo from "./halo/Halo";
import Radial from "./radial/Radial";
import { initSettings } from "./settings";
import ToolWindow from "./tools/ToolWindow";
import "./App.css";

// Une seule page pour toutes les fenêtres : on choisit l'interface selon le label.
const label = getCurrentWindow().label;
document.documentElement.dataset.window = label;
// Thème et réglages, suivis en direct dans chaque fenêtre.
initSettings();

// Les erreurs non gérées remontent dans la console de l'application (debug).
const report = (error: unknown) => {
  const message = error instanceof Error ? `${error.message}
${error.stack ?? ""}` : String(error);
  invoke("log_frontend_error", { message }).catch(() => {});
};
window.addEventListener("error", (e) => report(e.error ?? e.message));
window.addEventListener("unhandledrejection", (e) => report(e.reason));

const views: Record<string, () => React.JSX.Element> = { radial: Radial, halo: Halo, tool: ToolWindow };
const View = views[label] ?? App;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement, {
  onUncaughtError: (error, info) => report(`${String(error)}${info.componentStack ?? ""}`),
}).render(
  <React.StrictMode>
    <View />
  </React.StrictMode>,
);
