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
import "./Settings.css";

type Section = "appearance" | "menu" | "save" | "tools" | "system" | "about";

const SECTIONS: [Section, string][] = [
  ["appearance", "Apparence"],
  ["menu", "Menu des formats"],
  ["save", "Enregistrement"],
  ["tools", "Outils"],
  ["system", "Système"],
  ["about", "À propos"],
];

/** Section demandée à l'ouverture (`index.html#about`, depuis une notification). */
const initialSection = (): Section => {
  const hash = window.location.hash.slice(1);
  return SECTIONS.some(([id]) => id === hash) ? (hash as Section) : "appearance";
};

const SITE = "https://morph.underskydev.com";
const CONTACT = "contact@underskydev.com";

/** Composants libres embarqués : [nom, rôle, licence, site]. */
const COMPONENTS: [string, string, string, string][] = [
  ["FFmpeg", "Conversion et édition audio / vidéo", "GPL-3.0", "https://ffmpeg.org"],
  ["Tauri", "Socle de l'application", "MIT / Apache-2.0", "https://tauri.app"],
  ["React", "Interface", "MIT", "https://react.dev"],
  ["Motion", "Animations", "MIT", "https://motion.dev"],
  ["Konva", "Éditeur d'image", "MIT", "https://konvajs.org"],
  ["image-rs", "Lecture et écriture des images", "MIT / Apache-2.0", "https://github.com/image-rs/image"],
  ["oxipng", "Compression PNG sans perte", "MIT", "https://github.com/shssoichiro/oxipng"],
  ["libimagequant", "Compression PNG par palette", "GPL-3.0", "https://pngquant.org/lib/"],
  ["ONNX Runtime", "Exécution du modèle de détourage", "MIT", "https://onnxruntime.ai"],
  ["IS-Net (DIS)", "Modèle de détourage, téléchargé à la 1re utilisation", "Apache-2.0", "https://github.com/xuebinqin/DIS"],
];

const DUPLICATES: [Duplicates, string, string][] = [
  ["keep", "Garder les deux", "Le nouveau fichier reçoit un numéro : « Photo (2).webp »."],
  ["replace", "Remplacer", "L'ancien fichier est remplacé (il est conservé si l'opération échoue)."],
  ["ask", "Demander à chaque fois", "Remplacer, garder les deux ou ignorer, fichier par fichier ou pour tous."],
];

const SAVE_MODES: [SaveMode, string, string][] = [
  ["beside", "À côté de l'original", "Ex. : « Photo.png » donne « Photo.webp » dans le même dossier."],
  ["folder", "Dans un dossier fixe", "Tous les résultats vont dans le dossier choisi."],
  ["ask", "Demander à chaque fois", "Le dossier est choisi au moment d'enregistrer."],
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
        {SECTIONS.map(([id, label]) => (
          <button key={id} className={section === id ? "is-active" : ""} onClick={() => setSection(id)}>
            {section === id && <motion.span layoutId="nav-active" className="settings-nav-pill" transition={PILL} />}
            <span>{label}</span>
          </button>
        ))}
        <p className="settings-tip">
          Glisse des fichiers en maintenant <kbd>Shift</kbd> depuis l'explorateur ou le bureau, et lâche-les
          n'importe où. Tu peux aussi les déposer sur cette fenêtre.
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
      {over && <div className="settings-drop">Lâche pour ouvrir le menu des formats</div>}
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
      <Group title="Thème">
        <ThemePicker settings={settings} save={save} />
      </Group>
      <Group title="Couleur d'accent" hint="Boutons, sélections, progression et halo.">
        <ColorChoice value={settings.accent} colors={ACCENTS} onChange={(accent) => save({ accent })} />
      </Group>
      <Group title="Fond" hint="Teinte des fenêtres et des menus.">
        <ColorChoice value={settings.background} colors={BACKGROUNDS} onChange={(background) => save({ background })} />
      </Group>
      <Group title="Transparence des menus">
        <div className="settings-opacity">
          <div className="settings-opacity-preview">
            <span className="settings-petal">PNG</span>
            <span className="settings-petal">WEBP</span>
          </div>
          <label className="settings-range">
            <span>
              Opacité <strong>{settings.radialOpacity} %</strong>
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
      <Group title="Formats favoris" hint="Ils apparaissent en premier dans le menu, dans l'ordre où tu les choisis.">
        <FavoritesPicker settings={settings} save={save} />
        {settings.favorites.length > 0 && (
          <button className="settings-link" onClick={() => save({ favorites: [] })}>
            Retirer tous les favoris
          </button>
        )}
      </Group>
      <Group title="Lots de types mélangés" hint="Ex. : des images et des sons déposés ensemble.">
        <div className="settings-options">
          <Option
            checked={settings.mixedBatch === "groups"}
            onChange={() => save({ mixedBatch: "groups" })}
            title="Un sous-menu par type"
            hint="Formats communs à tous, plus « Images → … », « Audio → … »."
          />
          <Option
            checked={settings.mixedBatch === "common"}
            onChange={() => save({ mixedBatch: "common" })}
            title="Formats communs seulement"
            hint="Un seul niveau de menu. S'il n'y a aucun format commun, les sous-menus sont proposés."
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
    const dir = await open({ directory: true, title: "Dossier des résultats" });
    if (typeof dir === "string") save({ saveMode: "folder", saveFolder: dir });
  }
  return (
    <Group title="Où enregistrer les résultats" hint="Conversions et outils (Recadrer, Détourer, Compresser, Éditer).">
      <div className="settings-options">
        {SAVE_MODES.map(([id, title, hint]) => (
          <Option
            key={id}
            checked={settings.saveMode === id}
            onChange={() => (id === "folder" && !settings.saveFolder ? pickFolder() : save({ saveMode: id }))}
            title={title}
            hint={hint}
          >
            {id === "folder" && (
              <span className="settings-folder">
                <code>{settings.saveFolder ?? "Aucun dossier choisi"}</code>
                <button onClick={(e) => (e.preventDefault(), pickFolder())}>Choisir…</button>
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
    <Group title="Si le fichier existe déjà" hint="Ex. : « Photo.webp » est déjà dans le dossier.">
      <div className="settings-options">
        {DUPLICATES.map(([id, title, hint]) => (
          <Option key={id} checked={settings.duplicates === id} onChange={() => save({ duplicates: id })} title={title} hint={hint} />
        ))}
      </div>
    </Group>
  );
}

function ToolsSection({ settings, save }: SectionProps) {
  return (
    <Group title="Détourer" hint="Modèle utilisé pour retirer le fond des images. Tout reste sur ton ordinateur.">
      <div className="settings-options">
        <Option
          checked={settings.cutout === "fast"}
          onChange={() => save({ cutout: "fast" })}
          title="Efficacité"
          hint="Quelques secondes par image. Modèle de 170 Mo."
        />
        <Option
          checked={settings.cutout === "best"}
          onChange={() => save({ cutout: "best" })}
          title="Qualité"
          hint="Contours bien plus fins (cheveux, objets détaillés), mais plus lent. Modèle de 970 Mo, téléchargé au premier détourage."
        />
      </div>
    </Group>
  );
}

function SystemSection({ settings, save }: SectionProps) {
  const platform = usePlatform();
  return (
    <>
      {platform === "linux" && (
        <Group title="Intégration au bureau" hint="Menu des applications, « Ouvrir avec Morph » et clic droit des gestionnaires de fichiers.">
          <LinuxIntegration />
        </Group>
      )}
      <Group title="Démarrage">
        <label className="settings-toggle">
          <input type="checkbox" checked={settings.autostart} onChange={(e) => save({ autostart: e.target.checked })} />
          <span>
            <strong>Lancer Morph à l'ouverture de session</strong>
            <small>Morph reste discret dans la zone de notification, prêt pour le Shift + glisser.</small>
          </span>
        </label>
      </Group>
      <Group title="Morph">
        <div>
          <button className="settings-button" onClick={() => save({ onboarded: false })}>
            Revoir l'écran d'accueil
          </button>{" "}
          <button className="settings-button" onClick={() => invoke("quit")}>
            Quitter Morph
          </button>
        </div>
        <p className="settings-hint">Fermer cette fenêtre laisse Morph actif ; l'icône de la zone de notification la rouvre.</p>
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
          <p className="settings-hint">Version {version} · convertir et retoucher en un geste</p>
          <div className="about-links">
            <button className="settings-button" onClick={() => open(SITE)}>
              Site de présentation ↗
            </button>
            <button className="settings-button" onClick={() => open(`mailto:${CONTACT}`)}>
              Contact : {CONTACT}
            </button>
          </div>
        </div>
      </section>
      <UpdatesGroup />
      <Group title="Licence" hint="Morph est un logiciel libre, distribué sous licence GPL-3.0-or-later.">
        <p className="settings-hint">© 2026 UnderSkyDev.</p>
      </Group>
      <Group title="Composants libres" hint="Morph s'appuie sur ces projets, merci à leurs auteurs.">
        <ul className="about-components">
          {COMPONENTS.map(([name, role, license, url]) => (
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
  const min = Math.round((Date.now() - ms) / 60000);
  if (min < 1) return "à l'instant";
  if (min < 60) return `il y a ${min} min`;
  const h = Math.round(min / 60);
  return h < 24 ? `il y a ${h} h` : `il y a ${Math.round(h / 24)} j`;
}

/** Mises à jour : vérifiées automatiquement (démarrage, puis toutes les 6 h), installées sur clic. */
function UpdatesGroup() {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<UpdateStatus>("update_status").then(setStatus);
    const unlisten = listen<UpdateStatus>("update-status", ({ payload }) => setStatus(payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

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
    <Group title="Mises à jour">
      {installing !== null ? (
        <div className="about-update">
          <p>Téléchargement de Morph {available?.version}… {Math.round(installing * 100)} %</p>
          <div className="about-bar">
            <span style={{ transform: `scaleX(${installing})` }} />
          </div>
          <p className="settings-hint">Morph redémarrera tout seul une fois la mise à jour installée.</p>
        </div>
      ) : available ? (
        <div className="about-update">
          <p>
            <strong>Morph {available.version} est disponible.</strong>
          </p>
          {available.notes && <p className="about-notes">{available.notes}</p>}
          <button className="settings-button is-primary" onClick={install}>
            Installer et redémarrer
          </button>
        </div>
      ) : (
        <div className="about-update">
          <p>{checking ? "Recherche d'une mise à jour…" : "Morph est à jour."}</p>
          {lastCheckMs !== null && !checking && <p className="settings-hint">Dernière vérification {ago(lastCheckMs)}.</p>}
          <button className="settings-button" disabled={checking} onClick={() => invoke("check_update")}>
            Rechercher maintenant
          </button>
        </div>
      )}
      {(error ?? status.error) && <p className="settings-error">{error ?? status.error}</p>}
    </Group>
  );
}

export default App;
