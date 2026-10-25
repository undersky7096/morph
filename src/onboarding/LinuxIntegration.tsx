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
        {on ? t("onboarding.remove_morph_from_system") : t("onboarding.add_morph_system")}
      </button>
      <span className="settings-hint">
        {on
          ? t("onboarding.morph_applications_menu_right_click")
          : t("onboarding.adds_morph_applications_menu_open")}
      </span>
      {error && <span className="settings-error">{error}</span>}
    </div>
  );
}
