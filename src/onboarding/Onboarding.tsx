import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AnimatePresence, motion } from "motion/react";

import { usePlatform, type Settings } from "../settings";
import { ACCENTS, ColorChoice, FavoritesPicker, ThemePicker } from "../settingsParts";
import Logo from "../Logo";
import { LANGUAGES, lang, t } from "../i18n";
import LinuxIntegration from "./LinuxIntegration";
import "./Onboarding.css";

interface Props {
  settings: Settings;
  save: (patch: Partial<Settings>) => void;
  /** Rouvert depuis les Réglages : à la fin, on y revient au lieu de masquer la fenêtre. */
  reviewing?: boolean;
}

const STEPS = () => [t("onboarding.welcome"), t("onboarding.appearance"), t("onboarding.favourites"), t("onboarding.ready")];

/** Écran d'accueil du premier lancement : présentation du geste, thème, formats favoris,
 *  lancement au démarrage. Passable à tout moment ; rouvrable depuis les Réglages. */
export default function Onboarding({ settings, save, reviewing }: Props) {
  const [step, setStep] = useState(0);
  const [dir, setDir] = useState(1);
  const platform = usePlatform();
  const linux = platform === "linux";

  const go = (next: number) => {
    setDir(next > step ? 1 : -1);
    setStep(next);
  };

  async function finish() {
    save({ onboarded: true });
    // Morph vit ensuite dans la zone de notification.
    if (!reviewing) await getCurrentWindow().close();
  }

  const steps = STEPS();
  const last = step === steps.length - 1;

  return (
    <div className="onboarding">
      <header className="onboarding-head">
        <div className="onboarding-dots" aria-label={t("onboarding.step_of", { step: step + 1, total: steps.length })}>
          {steps.map((label, i) => (
            <button key={label} className={i === step ? "is-active" : i < step ? "is-done" : ""} onClick={() => go(i)} title={label}>
              {i === step && <motion.span layoutId="onboarding-dot" className="onboarding-dot-pill" />}
            </button>
          ))}
        </div>
        {!last && (
          <button className="onboarding-skip" onClick={finish}>
            {t("onboarding.skip")}
          </button>
        )}
      </header>

      <main className="onboarding-stage">
        <AnimatePresence mode="wait" custom={dir}>
          <motion.section
            key={step}
            className="onboarding-step"
            custom={dir}
            initial={{ opacity: 0, x: 32 * dir }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -32 * dir }}
            transition={{ duration: 0.22, ease: "easeOut" }}
          >
            {step === 0 && (
              <>
                <Logo className="onboarding-logo" />
                <h1>{t("onboarding.welcome_morph")}</h1>
                <p className="onboarding-lead">
                  {t("onboarding.convert_edit_images_sounds_videos")}
                </p>
                <div className="onboarding-block">
                  <LanguageChoice save={save} />
                </div>
                <Demo linux={linux} />
                <p className="onboarding-caption">
                  {linux ? (
                    <>
                      {t("onboarding.right_click_files")}
                      <strong>{t("onboarding.convert_morph")}</strong>
                      {t("onboarding.formats_appear_as_petals")}
                    </>
                  ) : (
                    <>
                      {t("onboarding.drag_files_from_desktop_explorer")}
                      <kbd>Shift</kbd>
                      {t("onboarding.then_drop_them_anywhere_formats")}
                    </>
                  )}
                </p>
              </>
            )}

            {step === 1 && (
              <>
                <h1>{t("onboarding.style")}</h1>
                <p className="onboarding-lead">{t("onboarding.can_change_any_time_settings")}</p>
                <div className="onboarding-block">
                  <h2>{t("onboarding.theme")}</h2>
                  <ThemePicker settings={settings} save={save} id="onboarding-theme" />
                </div>
                <div className="onboarding-block">
                  <h2>{t("onboarding.accent_colour")}</h2>
                  <ColorChoice value={settings.accent} colors={ACCENTS} onChange={(accent) => save({ accent })} />
                </div>
                <div className="onboarding-preview" aria-hidden>
                  <span className="settings-petal">PNG</span>
                  <span className="onboarding-hub">1</span>
                  <span className="settings-petal">MP3</span>
                </div>
              </>
            )}

            {step === 2 && (
              <>
                <h1>{t("onboarding.favourite_formats")}</h1>
                <p className="onboarding-lead">
                  {t("onboarding.theyll_appear_first_menu_order")}
                </p>
                <div className="onboarding-block onboarding-favorites">
                  <FavoritesPicker settings={settings} save={save} />
                </div>
              </>
            )}

            {step === 3 && (
              <>
                <h1>{t("onboarding.youre_all_set")}</h1>
                <p className="onboarding-lead">
                  {linux
                    ? t("onboarding.morph_stays_quietly_notification_area")
                    : t("onboarding.morph_stays_quietly_notification_area_2")}
                </p>
                <label className="settings-toggle onboarding-block">
                  <input type="checkbox" checked={settings.autostart} onChange={(e) => save({ autostart: e.target.checked })} />
                  <span>
                    <strong>{t("onboarding.start_morph_when_i_sign")}</strong>
                    <small>
                      {linux
                        ? t("onboarding.recommended_menu_only_works_while")
                        : t("onboarding.recommended_shift_drag_only_works")}
                    </small>
                  </span>
                </label>
                {linux && (
                  <div className="onboarding-block">
                    <h2>{t("onboarding.desktop_integration")}</h2>
                    <LinuxIntegration />
                  </div>
                )}
              </>
            )}
          </motion.section>
        </AnimatePresence>
      </main>

      <footer className="onboarding-foot">
        {step > 0 ? (
          <button className="settings-button" onClick={() => go(step - 1)}>
            {t("onboarding.back")}
          </button>
        ) : (
          <span />
        )}
        <button className="settings-button is-primary" onClick={() => (last ? finish() : go(step + 1))}>
          {last ? t("onboarding.lets_go") : t("onboarding.next")}
        </button>
      </footer>
    </div>
  );
}

/** Choix de la langue dès l'accueil (une par catalogue de `locales/`). */
function LanguageChoice({ save }: Pick<Props, "save">) {
  const active = lang();
  return (
    <div className="settings-segmented">
      {LANGUAGES.map(({ code, name }) => (
        <button key={code} className={active === code ? "is-active" : ""} aria-pressed={active === code} onClick={() => save({ language: code })}>
          {active === code && <motion.span layoutId="onboarding-lang" className="settings-segmented-pill" />}
          <span lang={code}>{name}</span>
        </button>
      ))}
    </div>
  );
}

/** Démo en boucle : un fichier glissé (Shift enfoncé, ou clic droit sous Linux), puis
 *  le menu qui s'ouvre en pétales au point de dépôt. Transformations et opacité seulement. */
function Demo({ linux }: { linux: boolean }) {
  const loop = { duration: 4.8, repeat: Infinity, ease: "easeInOut" } as const;
  const petals = ["PNG", "WEBP", "JPG", "GIF", "ICO"];
  return (
    <div className="demo" aria-hidden>
      <motion.div
        className="demo-file"
        // Glissé jusqu'au point de dépôt, il disparaît, revient invisible puis réapparaît en fondu.
        animate={{ x: [0, 0, 190, 190, 0, 0], y: [0, 0, -28, -28, 0, 0], opacity: [1, 1, 1, 0, 0, 1] }}
        transition={{ ...loop, times: [0, 0.12, 0.42, 0.48, 0.9, 1] }}
      >
        <span className="demo-thumb" />
        <span>photo.png</span>
      </motion.div>
      <motion.span
        className="demo-key"
        animate={{ opacity: [0, 1, 1, 0, 0], scale: [0.8, 1, 1, 0.9, 0.8] }}
        transition={{ ...loop, times: [0, 0.1, 0.42, 0.5, 1] }}
      >
        {linux ? t("onboarding.right_click") : "Shift"}
      </motion.span>
      <div className="demo-drop">
        <motion.span
          className="demo-hub"
          animate={{ opacity: [0, 0, 1, 1, 0], scale: [0.4, 0.4, 1, 1, 0.6] }}
          transition={{ ...loop, times: [0, 0.46, 0.56, 0.88, 0.96] }}
        >
          1
        </motion.span>
        {petals.map((p, i) => {
          const a = -Math.PI / 2 + (i / petals.length) * 2 * Math.PI;
          return (
            <motion.span
              key={p}
              className="demo-petal"
              animate={{
                x: [0, 0, Math.cos(a) * 62, Math.cos(a) * 62, 0],
                y: [0, 0, Math.sin(a) * 62, Math.sin(a) * 62, 0],
                opacity: [0, 0, 1, 1, 0],
                scale: [0.3, 0.3, 1, 1, 0.3],
              }}
              transition={{ ...loop, times: [0, 0.5 + i * 0.015, 0.62 + i * 0.015, 0.88, 0.96] }}
            >
              {p}
            </motion.span>
          );
        })}
      </div>
    </div>
  );
}
