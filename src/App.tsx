import { type ReactNode, useEffect, useRef, useState } from "react";
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
import { type Language, t, locale } from "./i18n";
import "./Settings.css";

type Section = "appearance" | "menu" | "save" | "tools" | "system" | "about";

const SECTION_IDS: Section[] = ["appearance", "menu", "save", "tools", "system", "about"];

const sectionLabel = (id: Section) =>
  ({
    appearance: t("Apparence", "Appearance"),
    menu: t("Menu des formats", "Format menu"),
    save: t("Enregistrement", "Saving"),
    tools: t("Outils", "Tools"),
    system: t("Système", "System"),
    about: t("À propos", "About"),
  })[id];

/** Section demandée à l'ouverture (`index.html#about`, depuis une notification). */
const initialSection = (): Section => {
  const hash = window.location.hash.slice(1);
  return SECTION_IDS.includes(hash as Section) ? (hash as Section) : "appearance";
};

const SITE = "https://morph.underskydev.com";
const CONTACT = "contact@underskydev.com";

/** Composants libres embarqués : [nom, rôle, licence, site]. */
const COMPONENTS = (): [string, string, string, string][] => [
  ["FFmpeg", t("Conversion et édition audio / vidéo", "Audio / video conversion and editing"), "GPL-3.0", "https://ffmpeg.org"],
  ["Tauri", t("Socle de l'application", "Application framework"), "MIT / Apache-2.0", "https://tauri.app"],
  ["React", t("Interface", "User interface"), "MIT", "https://react.dev"],
  ["Motion", t("Animations", "Animations"), "MIT", "https://motion.dev"],
  ["Konva", t("Éditeur d'image", "Image editor"), "MIT", "https://konvajs.org"],
  ["image-rs", t("Lecture et écriture des images", "Image reading and writing"), "MIT / Apache-2.0", "https://github.com/image-rs/image"],
  ["oxipng", t("Compression PNG sans perte", "Lossless PNG compression"), "MIT", "https://github.com/shssoichiro/oxipng"],
  ["libimagequant", t("Compression PNG par palette", "Palette-based PNG compression"), "GPL-3.0", "https://pngquant.org/lib/"],
  ["ONNX Runtime", t("Exécution du modèle de détourage", "Runs the background removal model"), "MIT", "https://onnxruntime.ai"],
  ["DirectML", t("Détourage sur la carte graphique (Windows)", "Background removal on the graphics card (Windows)"), t("Licence Microsoft (redistribuable)", "Microsoft license (redistributable)"), "https://github.com/microsoft/DirectML"],
  ["IS-Net (DIS)", t("Modèle de détourage « Efficacité », téléchargé à la 1re utilisation", "“Speed” background removal model, downloaded on first use"), "Apache-2.0", "https://github.com/xuebinqin/DIS"],
  ["BiRefNet", t("Modèle de détourage « Qualité », téléchargé à la 1re utilisation", "“Quality” background removal model, downloaded on first use"), "MIT", "https://github.com/ZhengPeng7/BiRefNet"],
  ["resvg", t("Rendu des images SVG", "SVG rendering"), "Apache-2.0 / MIT", "https://github.com/linebender/resvg"],
  ["pandoc", t("Conversion des documents texte", "Text document conversion"), "GPL-2.0", "https://pandoc.org"],
  ["PDFium", t("Lecture et fusion des PDF", "PDF reading and merging"), "BSD-3-Clause", "https://pdfium.googlesource.com/pdfium/"],
  ["lopdf", t("Création et compression des PDF", "PDF creation and compression"), "MIT", "https://github.com/J-F-Liu/lopdf"],
  ["LibreOffice", t("Formats bureautiques et export PDF, s'il est installé", "Office formats and PDF export, if installed"), "MPL-2.0", "https://www.libreoffice.org"],
  ["zip / sevenz-rust2", t("Archives ZIP et 7Z (TAR, GZ, BZ2, XZ en lecture)", "ZIP and 7Z archives (TAR, GZ, BZ2, XZ read-only)"), "MIT / Apache-2.0", "https://github.com/zip-rs/zip2"],
];

const DUPLICATES = (): [Duplicates, string, string][] => [
  ["keep", t("Garder les deux", "Keep both"), t("Le nouveau fichier reçoit un numéro : « Photo (2).webp ».", "The new file gets a number: “Photo (2).webp”.")],
  ["replace", t("Remplacer", "Replace"), t("L'ancien fichier est remplacé (il est conservé si l'opération échoue).", "The old file is replaced (it’s kept if the operation fails).")],
  ["ask", t("Demander à chaque fois", "Ask every time"), t("Remplacer, garder les deux ou ignorer, fichier par fichier ou pour tous.", "Replace, keep both or skip, file by file or for all.")],
];

const SAVE_MODES = (): [SaveMode, string, string][] => [
  ["beside", t("À côté de l'original", "Next to the original"), t("Ex. : « Photo.png » donne « Photo.webp » dans le même dossier.", "E.g. “Photo.png” gives “Photo.webp” in the same folder.")],
  ["folder", t("Dans un dossier fixe", "In a fixed folder"), t("Tous les résultats vont dans le dossier choisi.", "All results go to the chosen folder.")],
  ["ask", t("Demander à chaque fois", "Ask every time"), t("Le dossier est choisi au moment d'enregistrer.", "The folder is chosen when saving.")],
];

const LANGUAGES = (): [Language, string][] => [
  ["system", t("Système", "System")],
  ["fr", "Français"],
  ["en", "English"],
];

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
      <nav className="settings-nav">
        <div className="settings-brand">
          <Logo className="settings-logo" />
          <span>
            Morph <small>v{version}</small>
          </span>
        </div>
        {SECTION_IDS.map((id) => (
          <button key={id} className={section === id ? "is-active" : ""} onClick={() => setSection(id)}>
            {section === id && <motion.span layoutId="nav-active" className="settings-nav-pill" transition={PILL} />}
            <span>{sectionLabel(id)}</span>
            {id === "about" && update?.available && (
              <span className="settings-nav-badge" title={t(`Mise à jour ${update.available.version} disponible`, `Update ${update.available.version} available`)}>
                1
              </span>
            )}
          </button>
        ))}
        <p className="settings-tip">
          {t("Glisse des fichiers en maintenant ", "Drag files while holding ")}
          <kbd>Shift</kbd>
          {t(
            " depuis l'explorateur ou le bureau, et lâche-les n'importe où. Tu peux aussi les déposer sur cette fenêtre.",
            " from Explorer or the desktop, and drop them anywhere. You can also drop them on this window.",
          )}
        </p>
      </nav>

      <main className="settings-body" key={section}>
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
      {over && <div className="settings-drop">{t("Lâche pour ouvrir le menu des formats", "Drop to open the format menu")}</div>}
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
      <Group title={t("Langue", "Language")}>
        <LanguagePicker settings={settings} save={save} />
      </Group>
      <Group title={t("Thème", "Theme")}>
        <ThemePicker settings={settings} save={save} />
      </Group>
      <Group title={t("Couleur d'accent", "Accent colour")} hint={t("Boutons, sélections, progression et halo.", "Buttons, selections, progress and halo.")}>
        <ColorChoice value={settings.accent} colors={ACCENTS} onChange={(accent) => save({ accent })} />
      </Group>
      <Group title={t("Fond", "Background")} hint={t("Teinte des fenêtres et des menus.", "Tint of windows and menus.")}>
        <ColorChoice value={settings.background} colors={BACKGROUNDS} onChange={(background) => save({ background })} />
      </Group>
      <Group title={t("Transparence des menus", "Menu transparency")}>
        <div className="settings-opacity">
          <div className="settings-opacity-preview">
            <span className="settings-petal">PNG</span>
            <span className="settings-petal">WEBP</span>
          </div>
          <label className="settings-range">
            <span>
              {t("Opacité", "Opacity")} <strong>{settings.radialOpacity} %</strong>
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
        title={t("Formats favoris", "Favourite formats")}
        hint={t("Ils apparaissent en premier dans le menu, dans l'ordre où tu les choisis.", "They appear first in the menu, in the order you pick them.")}
      >
        <FavoritesPicker settings={settings} save={save} />
        {settings.favorites.length > 0 && (
          <button className="settings-link" onClick={() => save({ favorites: [] })}>
            {t("Retirer tous les favoris", "Clear all favourites")}
          </button>
        )}
      </Group>
      <Group title={t("Lots de types mélangés", "Mixed batches")} hint={t("Ex. : des images et des sons déposés ensemble.", "E.g. images and sounds dropped together.")}>
        <div className="settings-options">
          <Option
            checked={settings.mixedBatch === "groups"}
            onChange={() => save({ mixedBatch: "groups" })}
            title={t("Un sous-menu par type", "One submenu per type")}
            hint={t("Formats communs à tous, plus « Images → … », « Audio → … ».", "Formats common to all, plus “Images → …”, “Audio → …”.")}
          />
          <Option
            checked={settings.mixedBatch === "common"}
            onChange={() => save({ mixedBatch: "common" })}
            title={t("Formats communs seulement", "Common formats only")}
            hint={t(
              "Un seul niveau de menu. S'il n'y a aucun format commun, les sous-menus sont proposés.",
              "A single menu level. If there’s no common format, submenus are shown.",
            )}
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
    const dir = await open({ directory: true, title: t("Dossier des résultats", "Results folder") });
    if (typeof dir === "string") save({ saveMode: "folder", saveFolder: dir });
  }
  return (
    <Group
      title={t("Où enregistrer les résultats", "Where to save results")}
      hint={t("Conversions et outils (Recadrer, Détourer, Compresser, Éditer).", "Conversions and tools (Crop, Remove background, Compress, Edit).")}
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
                <code>{settings.saveFolder ?? t("Aucun dossier choisi", "No folder chosen")}</code>
                <button onClick={(e) => (e.preventDefault(), pickFolder())}>{t("Choisir…", "Choose…")}</button>
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
    <Group title={t("Si le fichier existe déjà", "If the file already exists")} hint={t("Ex. : « Photo.webp » est déjà dans le dossier.", "E.g. “Photo.webp” is already in the folder.")}>
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
        title={t("Détourer", "Remove background")}
        hint={t(
          "Modèle utilisé pour retirer le fond des images et des vidéos (une vidéo est traitée image par image, en WebM transparent). Tout reste sur ton ordinateur.",
          "Model used to remove the background of images and videos (a video is processed frame by frame, into a transparent WebM). Everything stays on your computer.",
        )}
      >
        <div className="settings-options">
          <Option
            checked={settings.cutout === "fast"}
            onChange={() => save({ cutout: "fast" })}
            title={t("Efficacité", "Speed")}
            hint={t(
              "Une à deux secondes par image. Vidéo : environ 1 s par image de la vidéo, 10 fois moins avec la carte graphique. Modèle de 170 Mo.",
              "One or two seconds per image. Video: about 1 s per video frame, 10 times less with the graphics card. 170 MB model.",
            )}
          />
          <Option
            checked={settings.cutout === "best"}
            onChange={() => save({ cutout: "best" })}
            title={t("Qualité", "Quality")}
            hint={t(
              "Contours bien plus fins (cheveux, objets détaillés), mais plus lent : environ 4 s par image avec la carte graphique, 15 s sans. Modèle de 490 Mo avec la carte graphique, 970 Mo sans, téléchargé au premier détourage.",
              "Much finer edges (hair, detailed objects), but slower: about 4 s per frame with the graphics card, 15 s without. 490 MB model with the graphics card, 970 MB without, downloaded on first use.",
            )}
          />
        </div>
      </Group>
      {platform === "windows" && (
        <Group title={t("Accélération", "Acceleration")}>
          <label className="settings-toggle">
            <input type="checkbox" checked={settings.gpu} onChange={(e) => save({ gpu: e.target.checked })} />
            <span>
              <strong>{t("Utiliser la carte graphique", "Use the graphics card")}</strong>
              <small>
                {t(
                  "Détourage jusqu'à 10 fois plus rapide (DirectX 12). Si la carte n'y arrive pas (pilote, mémoire vidéo insuffisante…), Morph passe tout seul au processeur.",
                  "Background removal up to 10 times faster (DirectX 12). If the card can't handle it (driver, not enough video memory…), Morph switches to the processor by itself.",
                )}
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
          title={t("Intégration au bureau", "Desktop integration")}
          hint={t("Menu des applications, « Ouvrir avec Morph » et clic droit des gestionnaires de fichiers.", "Applications menu, “Open with Morph” and file manager right-click.")}
        >
          <LinuxIntegration />
        </Group>
      )}
      <Group title={t("Démarrage", "Startup")}>
        <label className="settings-toggle">
          <input type="checkbox" checked={settings.autostart} onChange={(e) => save({ autostart: e.target.checked })} />
          <span>
            <strong>{t("Lancer Morph à l'ouverture de session", "Start Morph when I sign in")}</strong>
            <small>{t("Morph reste discret dans la zone de notification, prêt pour le Shift + glisser.", "Morph stays quietly in the notification area, ready for Shift + drag.")}</small>
          </span>
        </label>
      </Group>
      <Group title="Morph">
        <div>
          <button className="settings-button" onClick={() => save({ onboarded: false })}>
            {t("Revoir l'écran d'accueil", "Show the welcome screen again")}
          </button>{" "}
          <button className="settings-button" onClick={() => invoke("quit")}>
            {t("Quitter Morph", "Quit Morph")}
          </button>
        </div>
        <p className="settings-hint">
          {t("Fermer cette fenêtre laisse Morph actif ; l'icône de la zone de notification la rouvre.", "Closing this window keeps Morph running; the notification area icon reopens it.")}
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
          <p className="settings-hint">Version {version} · {t("convertir et retoucher en un geste", "convert and edit in one gesture")}</p>
          <div className="about-links">
            <button className="settings-button" onClick={() => open(SITE)}>
              {t("Site de présentation ↗", "Website ↗")}
            </button>
            <button className="settings-button" onClick={() => open(`${SITE}/reviews/#write`)}>
              {t("Laisser un avis ↗", "Leave a review ↗")}
            </button>
            <button className="settings-button" onClick={() => open(`${SITE}/roadmap/`)}>
              {t("Proposer une idée ↗", "Suggest an idea ↗")}
            </button>
            <button className="settings-button" onClick={() => open(`mailto:${CONTACT}`)}>
              {t("Contact : ", "Contact: ")}{CONTACT}
            </button>
          </div>
        </div>
      </section>
      <UpdatesGroup />
      <Group title={t("Licence", "License")} hint={t("Morph est un logiciel libre, distribué sous licence GPL-3.0-or-later.", "Morph is free software, released under the GPL-3.0-or-later license.")}>
        <p className="settings-hint">© 2026 UnderSkyDev.</p>
      </Group>
      <Group title={t("Composants libres", "Open-source components")} hint={t("Morph s'appuie sur ces projets, merci à leurs auteurs.", "Morph builds on these projects, thanks to their authors.")}>
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
  if (min < 1) return t("à l'instant", "just now");
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
    <Group title={t("Mises à jour", "Updates")}>
      {installing !== null ? (
        <div className="about-update">
          <p>
            {t("Téléchargement de Morph", "Downloading Morph")} {available?.version}… {Math.round(installing * 100)} %
          </p>
          <div className="about-bar">
            <span style={{ transform: `scaleX(${installing})` }} />
          </div>
          <p className="settings-hint">{t("Morph redémarrera tout seul une fois la mise à jour installée.", "Morph will restart by itself once the update is installed.")}</p>
        </div>
      ) : available ? (
        <div className="about-update">
          <p>
            <strong>{t(`Morph ${available.version} est disponible.`, `Morph ${available.version} is available.`)}</strong>
          </p>
          {available.notes && <p className="about-notes">{available.notes}</p>}
          <div className="about-links">
            <button className="settings-button is-primary" onClick={install}>
              {t("Installer et redémarrer", "Install and restart")}
            </button>
            <button className="settings-button" onClick={() => invoke("open_link", { url: `${SITE}/changelog/` })}>
              {t("Toutes les nouveautés ↗", "All release notes ↗")}
            </button>
          </div>
        </div>
      ) : (
        <div className="about-update">
          <p>{checking ? t("Recherche d'une mise à jour…", "Checking for updates…") : t("Morph est à jour.", "Morph is up to date.")}</p>
          {lastCheckMs !== null && !checking && (
            <p className="settings-hint">
              {t("Dernière vérification", "Last checked")} {ago(lastCheckMs)}.
            </p>
          )}
          <button className="settings-button" disabled={checking} onClick={() => invoke("check_update")}>
            {t("Rechercher maintenant", "Check now")}
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
      {LANGUAGES().map(([language, label]) => {
        const active = settings.language === language;
        return (
          <button key={language} className={active ? "is-active" : ""} onClick={() => save({ language })}>
            {active && <motion.span layoutId="language-active" className="settings-segmented-pill" transition={PILL} />}
            <span>{label}</span>
          </button>
        );
      })}
    </div>
  );
}

export default App;
