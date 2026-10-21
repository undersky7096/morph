import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { t } from "../i18n";

/** Linux : ajoute ou retire Morph du menu des applications, d'« Ouvrir avec » et du clic droit. */
export default function LinuxIntegration() {
  const [on, setOn] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("system_integrated").then(setOn);
  }, []);

  async function toggle() {
    setBusy(true);
    setError(null);
    try {
      await invoke("set_system_integration", { enable: !on });
      setOn(!on);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (on === null) return null;
  return (
    <div className="linux-integration">
      <button className={on ? "settings-button" : "settings-button is-primary"} onClick={toggle} disabled={busy}>
        {on ? t("Retirer Morph du système", "Remove Morph from the system") : t("Ajouter Morph au système", "Add Morph to the system")}
      </button>
      <span className="settings-hint">
        {on
          ? t(
              "Morph est dans le menu des applications et dans le clic droit (Convertir, Éditer, Compresser avec Morph).",
              "Morph is in the applications menu and in the right-click menu (Convert, Edit, Compress with Morph).",
            )
          : t(
              "Ajoute Morph au menu des applications, à « Ouvrir avec » et au clic droit de Nautilus, Dolphin et Nemo.",
              "Adds Morph to the applications menu, “Open with” and the right-click menu of Nautilus, Dolphin and Nemo.",
            )}
      </span>
      {error && <span className="settings-error">{error}</span>}
    </div>
  );
}
