import { type ReactElement, type ReactNode, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "motion/react";

import Onboarding from "./onboarding/Onboarding";
import LinuxIntegration from "./onboarding/LinuxIntegration";
import {
  type Duplicates,
  saveSettings,
  type SaveMode,
  type Settings,
  usePlatform,
  useSettings,
  useSettingsLoaded,
} from "./settings";
import { ACCENTS, BACKGROUNDS, ColorChoice, FavoritesPicker, PILL, ThemePicker } from "./settingsParts";
import Logo from "./Logo";
import { LANGUAGES, t, locale } from "./i18n";
import HomeSection, { SITE } from "./HomeSection";
import AccountSection, { useAccount } from "./AccountSection";
import { HomeIcon, InfoIcon, PaletteIcon, RingIcon, FolderIcon, ToolsIcon, UserIcon, SystemIcon } from "./icons";
import "./Settings.css";

type Section = "home" | "appearance" | "menu" | "save" | "tools" | "account" | "system" | "about";

const SECTIONS: [Section, () => ReactElement][] = [
  ["home", HomeIcon],
  ["appearance", PaletteIcon],
  ["menu", RingIcon],
  ["save", FolderIcon],
  ["tools", ToolsIcon],
  ["account", UserIcon],
  ["system", SystemIcon],
  ["about", InfoIcon],
];
const SECTION_IDS = SECTIONS.map(([id]) => id);

const sectionLabel = (id: Section) =>
  ({
    home: t("home.nav"),
    appearance: t("settings.appearance"),
    menu: t("settings.format_menu"),
    save: t("settings.saving"),
    tools: t("settings.tools"),
    account: t("account.nav"),
    system: t("settings.system"),
    about: t("settings.about"),
  })[id];

/** Section demandée à l'ouverture (`index.html#about`, depuis une notification). */
const initialSection = (): Section => {
  const hash = window.location.hash.slice(1);
  return SECTION_IDS.includes(hash as Section) ? (hash as Section) : "home";
};

const CONTACT = "contact@underskydev.com";

/** Composants libres embarqués : [nom, rôle, licence, site]. */
const COMPONENTS = (): [string, string, string, string][] => [
  ["FFmpeg", t("settings.audio_video_conversion_editing"), "GPL-3.0", "https://ffmpeg.org"],
  ["Tauri", t("settings.application_framework"), "MIT / Apache-2.0", "https://tauri.app"],
  ["React", t("settings.user_interface"), "MIT", "https://react.dev"],
  ["Motion", t("settings.animations"), "MIT", "https://motion.dev"],
  ["Konva", t("settings.image_editor"), "MIT", "https://konvajs.org"],
  ["image-rs", t("settings.image_reading_writing"), "MIT / Apache-2.0", "https://github.com/image-rs/image"],
  ["oxipng", t("settings.lossless_png_compression"), "MIT", "https://github.com/shssoichiro/oxipng"],
  ["libimagequant", t("settings.palette_based_png_compression"), "GPL-3.0", "https://pngquant.org/lib/"],
  ["ONNX Runtime", t("settings.runs_background_removal_model"), "MIT", "https://onnxruntime.ai"],
  ["DirectML", t("settings.background_removal_graphics_card_windows"), t("settings.microsoft_license_redistributable"), "https://github.com/microsoft/DirectML"],
  ["IS-Net (DIS)", t("settings.speed_background_removal_model_downloaded"), "Apache-2.0", "https://github.com/xuebinqin/DIS"],
  ["BiRefNet", t("settings.quality_background_removal_model_downloaded"), "MIT", "https://github.com/ZhengPeng7/BiRefNet"],
  ["resvg", t("settings.svg_rendering"), "Apache-2.0 / MIT", "https://github.com/linebender/resvg"],
  ["pandoc", t("settings.text_document_conversion"), "GPL-2.0", "https://pandoc.org"],
  ["PDFium", t("settings.pdf_reading_merging"), "BSD-3-Clause", "https://pdfium.googlesource.com/pdfium/"],
  ["lopdf", t("settings.pdf_creation_compression"), "MIT", "https://github.com/J-F-Liu/lopdf"],
  ["LibreOffice", t("settings.office_formats_pdf_export_if"), "MPL-2.0", "https://www.libreoffice.org"],
  ["zip / sevenz-rust2", t("settings.zip_7z_archives_tar_gz"), "MIT / Apache-2.0", "https://github.com/zip-rs/zip2"],
];

const DUPLICATES = (): [Duplicates, string, string][] => [
  ["keep", t("settings.keep_both"), t("settings.new_file_gets_number_photo")],
  ["replace", t("settings.replace"), t("settings.old_file_replaced_kept_if")],
  ["ask", t("settings.ask_every_time"), t("settings.replace_keep_both_skip_file")],
];

const SAVE_MODES = (): [SaveMode, string, string][] => [
  ["beside", t("settings.next_original"), t("settings.e_g_photo_png_gives")],
  ["folder", t("settings.fixed_folder"), t("settings.all_results_go_chosen_folder")],
  ["ask", t("settings.ask_every_time"), t("settings.folder_chosen_when_saving")],
];

/** Système, puis une entrée par catalogue de `locales/` (chacune écrite dans sa langue). */
const languageChoices = (): [string, string][] => [["system", t("settings.system")], ...LANGUAGES.map(({ code, name }): [string, string] => [code, name])];

/** Fenêtre des Réglages (ouverte depuis l'icône de la zone de notification). Chaque
 *  changement s'applique aussitôt à toutes les fenêtres. Déposer des fichiers ici ouvre
 *  le menu des formats (pratique pour essayer). */
function App() {
  const settings = useSettings();
  const loaded = useSettingsLoaded();
  // Accueil rouvert depuis les Réglages : à la fin, on y revient (sinon la fenêtre se masque).
  const seenSettings = useRef(false);
  if (loaded && settings.onboarded) seenSettings.current = true;
  const [section, setSection] = useState<Section>(initialSection);
  const [version, setVersion] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [over, setOver] = useState(false);
  const update = useUpdateStatus();
  const account = useAccount();

  useEffect(() => {
    getVersion().then(setVersion);
  }, []);

  // Fenêtre déjà ouverte et notification cliquée : on passe à la section demandée.
  useEffect(() => {
    const unlisten = listen<Section>("settings-section", ({ payload }) => setSection(payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Fenêtre créée cachée : on l'affiche une fois les réglages chargés (pas de flash blanc).
  useEffect(() => {
    if (loaded) invoke("reveal_settings");
  }, [loaded]);

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

  const save = (patch: Partial<Settings>) => {
    setError(null);
    saveSettings(patch).catch((e) => setError(String(e)));
  };

  // Premier lancement (ou « Revoir l'accueil ») : l'écran d'accueil remplace les Réglages.
  if (!loaded) return null;
  if (!settings.onboarded) return <Onboarding settings={settings} save={save} reviewing={seenSettings.current} />;

  return (
    <div className={`settings ${over ? "is-over" : ""}`}>
      <nav className="settings-nav" aria-label={t("settings.sections")}>
        <div className="settings-brand">
          <Logo className="settings-logo" />
          <span>Morph</span>
        </div>
        {SECTIONS.map(([id, Icon]) => (
          <button
            key={id}
            className={section === id ? "is-active" : ""}
            aria-current={section === id ? "page" : undefined}
            title={sectionLabel(id)}
            onClick={() => setSection(id)}
          >
            {section === id && <motion.span layoutId="nav-active" className="settings-nav-pill" transition={PILL} />}
            <span className="settings-nav-icon">
              <Icon />
            </span>
            <span className="settings-nav-label">{sectionLabel(id)}</span>
            {id === "about" && update?.available && (
              <span className="settings-nav-badge" title={t("settings.update_available_badge", { version: update.available.version })}>
                1
              </span>
            )}
            {id === "account" && account?.user && <span className="settings-nav-dot" title={account.user.name} />}
          </button>
        ))}
        <p className="settings-nav-version ext">v{version}</p>
      </nav>

      <main className="settings-body" key={section}>
        {section === "home" && (
          <HomeSection settings={settings} version={version} update={update} account={account} goTo={setSection} />
        )}
        {section === "account" && <AccountSection status={account} />}
        {section === "appearance" && <Appearance settings={settings} save={save} />}
        {section === "menu" && <MenuSection settings={settings} save={save} />}
        {section === "save" && (
          <>
            <SaveSection settings={settings} save={save} />
            <DuplicatesSection settings={settings} save={save} />
          </>
        )}
        {section === "tools" && <ToolsSection settings={settings} save={save} />}
        {section === "system" && <SystemSection settings={settings} save={save} />}
        {section === "about" && <AboutSection version={version} />}
        {error && <p className="settings-error">{error}</p>}
      </main>
      {over && <div className="settings-drop">{t("settings.drop_open_format_menu")}</div>}
    </div>
  );
}

interface SectionProps {
  settings: Settings;
  save: (patch: Partial<Settings>) => void;
}

function Group({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <section className="settings-group">
      <h2>{title}</h2>
      {hint && <p className="settings-hint">{hint}</p>}
      {children}
    </section>
  );
}

function Appearance({ settings, save }: SectionProps) {
  return (
    <>
      <Group title={t("settings.language")}>
        <LanguagePicker settings={settings} save={save} />
      </Group>
      <Group title={t("settings.theme")}>
        <ThemePicker settings={settings} save={save} />
      </Group>
      <Group title={t("settings.accent_colour")} hint={t("settings.buttons_selections_progress_halo")}>
        <ColorChoice value={settings.accent} colors={ACCENTS} onChange={(accent) => save({ accent })} />
      </Group>
      <Group title={t("settings.background")} hint={t("settings.tint_windows_menus")}>
        <ColorChoice value={settings.background} colors={BACKGROUNDS} onChange={(background) => save({ background })} />
      </Group>
      <Group title={t("settings.menu_transparency")}>
        <div className="settings-opacity">
          <div className="settings-opacity-preview">
            <span className="settings-petal">PNG</span>
            <span className="settings-petal">WEBP</span>
          </div>
          <label className="settings-range">
            <span>
              {t("settings.opacity")} <strong>{settings.radialOpacity} %</strong>
            </span>
            <input
              type="range"
              min={40}
              max={100}
              value={settings.radialOpacity}
              onChange={(e) => save({ radialOpacity: Number(e.target.value) })}
            />
          </label>
        </div>
      </Group>
    </>
  );
}

function MenuSection({ settings, save }: SectionProps) {
  return (
    <>
      <Group
        title={t("settings.favourite_formats")}
        hint={t("settings.they_appear_first_menu_order")}
      >
        <FavoritesPicker settings={settings} save={save} />
        {settings.favorites.length > 0 && (
          <button className="settings-link" onClick={() => save({ favorites: [] })}>
            {t("settings.clear_all_favourites")}
          </button>
        )}
      </Group>
      <Group title={t("settings.mixed_batches")} hint={t("settings.e_g_images_sounds_dropped")}>
        <div className="settings-options">
          <Option
            checked={settings.mixedBatch === "groups"}
            onChange={() => save({ mixedBatch: "groups" })}
            title={t("settings.one_submenu_per_type")}
            hint={t("settings.formats_common_all_plus_images")}
          />
          <Option
            checked={settings.mixedBatch === "common"}
            onChange={() => save({ mixedBatch: "common" })}
            title={t("settings.common_formats_only")}
            hint={t("settings.single_menu_level_if_theres")}
          />
        </div>
      </Group>
    </>
  );
}

function Option(props: { checked: boolean; onChange: () => void; title: string; hint: string; children?: ReactNode }) {
  return (
    <label className={`settings-option ${props.checked ? "is-active" : ""}`}>
      <input type="radio" checked={props.checked} onChange={props.onChange} />
      <span>
        <strong>{props.title}</strong>
        <small>{props.hint}</small>
        {props.children}
      </span>
    </label>
  );
}

function SaveSection({ settings, save }: SectionProps) {
  async function pickFolder() {
    const dir = await open({ directory: true, title: t("settings.results_folder") });
    if (typeof dir === "string") save({ saveMode: "folder", saveFolder: dir });
  }
  return (
    <Group
      title={t("settings.where_save_results")}
      hint={t("settings.conversions_tools_crop_remove_background")}
    >
      <div className="settings-options">
        {SAVE_MODES().map(([id, title, hint]) => (
          <Option
            key={id}
            checked={settings.saveMode === id}
            onChange={() => (id === "folder" && !settings.saveFolder ? pickFolder() : save({ saveMode: id }))}
            title={title}
            hint={hint}
          >
            {id === "folder" && (
              <span className="settings-folder">
                <code>{settings.saveFolder ?? t("settings.no_folder_chosen")}</code>
                <button onClick={(e) => (e.preventDefault(), pickFolder())}>{t("settings.choose")}</button>
              </span>
            )}
          </Option>
        ))}
      </div>
    </Group>
  );
}

function DuplicatesSection({ settings, save }: SectionProps) {
  return (
    <Group title={t("settings.if_file_already_exists")} hint={t("settings.e_g_photo_webp_already")}>
      <div className="settings-options">
        {DUPLICATES().map(([id, title, hint]) => (
          <Option key={id} checked={settings.duplicates === id} onChange={() => save({ duplicates: id })} title={title} hint={hint} />
        ))}
      </div>
    </Group>
  );
}

function ToolsSection({ settings, save }: SectionProps) {
  const platform = usePlatform();
  return (
    <>
      <Group
        title={t("settings.remove_background")}
        hint={t("settings.model_used_remove_background_images")}
      >
        <div className="settings-options">
          <Option
            checked={settings.cutout === "fast"}
            onChange={() => save({ cutout: "fast" })}
            title={t("settings.speed")}
            hint={t("settings.one_two_seconds_per_image")}
          />
          <Option
            checked={settings.cutout === "best"}
            onChange={() => save({ cutout: "best" })}
            title={t("settings.quality")}
            hint={t("settings.much_finer_edges_hair_detailed")}
          />
        </div>
      </Group>
      {platform === "windows" && (
        <Group title={t("settings.acceleration")}>
          <label className="settings-toggle">
            <input type="checkbox" checked={settings.gpu} onChange={(e) => save({ gpu: e.target.checked })} />
            <span>
              <strong>{t("settings.use_graphics_card")}</strong>
              <small>
                {t("settings.background_removal_up_10_times")}
              </small>
            </span>
          </label>
        </Group>
      )}
    </>
  );
}

function SystemSection({ settings, save }: SectionProps) {
  const platform = usePlatform();
  return (
    <>
      {platform === "linux" && (
        <Group
          title={t("settings.desktop_integration")}
          hint={t("settings.applications_menu_open_morph_file")}
        >
          <LinuxIntegration />
        </Group>
      )}
      <Group title={t("settings.startup")}>
        <label className="settings-toggle">
          <input type="checkbox" checked={settings.autostart} onChange={(e) => save({ autostart: e.target.checked })} />
          <span>
            <strong>{t("settings.start_morph_when_i_sign")}</strong>
            <small>{t("settings.morph_stays_quietly_notification_area")}</small>
          </span>
        </label>
      </Group>
      <Group title="Morph">
        <div>
          <button className="settings-button" onClick={() => save({ onboarded: false })}>
            {t("settings.show_welcome_screen_again")}
          </button>{" "}
          <button className="settings-button" onClick={() => invoke("quit")}>
            {t("settings.quit_morph")}
          </button>
        </div>
        <p className="settings-hint">
          {t("settings.closing_window_keeps_morph_running")}
        </p>
      </Group>
    </>
  );
}

function AboutSection({ version }: { version: string }) {
  const open = (url: string) => invoke("open_link", { url });
  return (
    <>
      <section className="settings-group about-hero">
        <Logo className="about-logo" />
        <div>
          <h2>Morph</h2>
          <p className="settings-hint">Version {version} · {t("settings.convert_edit_one_gesture")}</p>
          <div className="about-links">
            <button className="settings-button" onClick={() => open(SITE)}>
              {t("settings.website")}
            </button>
            <button className="settings-button" onClick={() => open(`${SITE}/reviews/#write`)}>
              {t("settings.leave_review")}
            </button>
            <button className="settings-button" onClick={() => open(`${SITE}/roadmap/`)}>
              {t("settings.suggest_idea")}
            </button>
            <button className="settings-button" onClick={() => open(`mailto:${CONTACT}`)}>
              {t("settings.contact")}{CONTACT}
            </button>
          </div>
        </div>
      </section>
      <UpdatesGroup />
      <Group title={t("settings.license")} hint={t("settings.morph_free_software_released_under")}>
        <p className="settings-hint">© 2026 UnderSkyDev.</p>
      </Group>
      <Group title={t("settings.open_source_components")} hint={t("settings.morph_builds_these_projects_thanks")}>
        <ul className="about-components">
          {COMPONENTS().map(([name, role, license, url]) => (
            <li key={name}>
              <button className="settings-link" onClick={() => open(url)}>
                {name}
              </button>
              <span>{role}</span>
              <small>{license}</small>
            </li>
          ))}
        </ul>
      </Group>
    </>
  );
}

interface UpdateStatus {
  available: { version: string; notes: string | null } | null;
  checking: boolean;
  installing: number | null;
  error: string | null;
  lastCheckMs: number | null;
}

/** « il y a 3 h », « il y a 5 min », « à l'instant ». */
function ago(ms: number) {
  const rtf = new Intl.RelativeTimeFormat(locale(), { numeric: "auto" });
  const min = Math.round((Date.now() - ms) / 60000);
  if (min < 1) return t("settings.just_now");
  if (min < 60) return rtf.format(-min, "minute");
  const h = Math.round(min / 60);
  return h < 24 ? rtf.format(-h, "hour") : rtf.format(-Math.round(h / 24), "day");
}

/** État des mises à jour, suivi en direct. */
function useUpdateStatus() {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  useEffect(() => {
    invoke<UpdateStatus>("update_status").then(setStatus);
    const unlisten = listen<UpdateStatus>("update-status", ({ payload }) => setStatus(payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);
  return status;
}

/** Mises à jour : vérifiées automatiquement (démarrage, puis toutes les 6 h), installées sur clic. */
function UpdatesGroup() {
  const status = useUpdateStatus();
  const [error, setError] = useState<string | null>(null);

  async function install() {
    setError(null);
    try {
      await invoke("install_update");
    } catch (e) {
      setError(String(e));
    }
  }

  if (!status) return null;
  const { available, checking, installing, lastCheckMs } = status;
  return (
    <Group title={t("settings.updates")}>
      {installing !== null ? (
        <div className="about-update">
          <p>
            {t("settings.downloading_morph")} {available?.version}… {Math.round(installing * 100)} %
          </p>
          <div className="about-bar">
            <span style={{ transform: `scaleX(${installing})` }} />
          </div>
          <p className="settings-hint">{t("settings.morph_will_restart_by_itself")}</p>
        </div>
      ) : available ? (
        <div className="about-update">
          <p>
            <strong>{t("settings.version_available", { version: available.version })}</strong>
          </p>
          {available.notes && <p className="about-notes">{available.notes}</p>}
          <div className="about-links">
            <button className="settings-button is-primary" onClick={install}>
              {t("settings.install_restart")}
            </button>
            <button className="settings-button" onClick={() => invoke("open_link", { url: `${SITE}/changelog/` })}>
              {t("settings.all_release_notes")}
            </button>
          </div>
        </div>
      ) : (
        <div className="about-update">
          <p>{checking ? t("settings.checking_updates") : t("settings.morph_up_date")}</p>
          {lastCheckMs !== null && !checking && (
            <p className="settings-hint">
              {t("settings.last_checked")} {ago(lastCheckMs)}.
            </p>
          )}
          <button className="settings-button" disabled={checking} onClick={() => invoke("check_update")}>
            {t("settings.check_now")}
          </button>
        </div>
      )}
      {(error ?? status.error) && <p className="settings-error">{error ?? status.error}</p>}
    </Group>
  );
}

/** Système / Français / English. */
function LanguagePicker({ settings, save }: SectionProps) {
  return (
    <div className="settings-segmented">
      {languageChoices().map(([language, label]) => {
        const active = settings.language === language;
        return (
          <button key={language} className={active ? "is-active" : ""} onClick={() => save({ language })}>
            {active && <motion.span layoutId="language-active" className="settings-segmented-pill" transition={PILL} />}
            <span lang={language === "system" ? undefined : language}>{label}</span>
          </button>
        );
      })}
    </div>
  );
}

export default App;
