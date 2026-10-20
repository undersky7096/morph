import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { initSettings } from "./settings";
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

// Chaque fenêtre ne charge que son interface (le halo et le menu, toujours chargés,
// restent légers).
type View = { default: React.ComponentType };
const views: Record<string, () => Promise<View>> = {
  radial: () => import("./radial/Radial"),
  halo: () => import("./halo/Halo"),
  tool: () => import("./tools/ToolWindow"),
  progress: () => import("./progress/Progress"),
  main: () => import("./App"),
};

(views[label] ?? views.main)().then(({ default: View }) =>
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement, {
    onUncaughtError: (error, info) => report(`${String(error)}${info.componentStack ?? ""}`),
  }).render(
    <React.StrictMode>
      <View />
    </React.StrictMode>,
  ),
);
