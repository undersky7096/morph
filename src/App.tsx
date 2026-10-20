import { type ReactNode, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "motion/react";

import { kindOf } from "./formats";
import { saveSettings, type SaveMode, type Settings, type ThemeMode, useSettings } from "./settings";
import "./Settings.css";

type Section = "appearance" | "menu" | "save" | "system";

const SECTIONS: [Section, string][] = [
  ["appearance", "Apparence"],
  ["menu", "Menu des formats"],
  ["save", "Enregistrement"],
  ["system", "Système"],
];

const THEMES: [ThemeMode, string][] = [
  ["system", "Système"],
  ["light", "Clair"],
  ["dark", "Sombre"],
];

const ACCENTS = ["#2f6bff", "#8b5cf6", "#ec4899", "#ef4444", "#f97316", "#eab308", "#10b981", "#06b6d4"];
const BACKGROUNDS = ["#0b0d12", "#16112a", "#0d1f24", "#1f1414", "#f7f3ea", "#eef3ff", "#fdf0f4", "#eefaf3"];

/** Formats de sortie (miroir de `formats.rs`), par type, pour choisir les favoris. */
const OUTPUTS: [string, string[]][] = [
  ["Images", ["png", "jpg", "webp", "gif", "bmp", "tiff", "ico", "tga"]],
  ["Audio", ["mp3", "wav", "ogg", "flac", "m4a", "opus"]],
  ["Vidéo", ["mp4", "webm", "mkv", "mov", "avi"]],
];

const SAVE_MODES: [SaveMode, string, string][] = [
  ["beside", "À côté de l'original", "Ex. : « Photo.png » donne « Photo.webp » dans le même dossier."],
  ["folder", "Dans un dossier fixe", "Tous les résultats vont dans le dossier choisi."],
  ["ask", "Demander à chaque fois", "Le dossier est choisi au moment d'enregistrer."],
];

/** Glissement de l'indicateur de sélection (navigation, choix du thème). */
const PILL = { type: "spring", stiffness: 500, damping: 38 } as const;

/** Fenêtre des Réglages (ouverte depuis l'icône de la zone de notification). Chaque
 *  changement s'applique aussitôt à toutes les fenêtres. Déposer des fichiers ici ouvre
 *  le menu des formats (pratique pour essayer). */
function App() {
  const settings = useSettings();
  const [section, setSection] = useState<Section>("appearance");
  const [version, setVersion] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [over, setOver] = useState(false);

  useEffect(() => {
    getVersion().then(setVersion);
  }, []);

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

  return (
    <div className={`settings ${over ? "is-over" : ""}`}>
      <nav className="settings-nav">
        <div className="settings-brand">
          <span className="settings-logo">M</span>
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
        {section === "save" && <SaveSection settings={settings} save={save} />}
        {section === "system" && <SystemSection settings={settings} save={save} version={version} />}
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

/** Pastilles de couleur + sélecteur libre ; « Par défaut » efface la couleur personnalisée. */
function ColorChoice(props: { value: string | null; colors: string[]; onChange: (c: string | null) => void }) {
  const { value, colors, onChange } = props;
  const custom = value !== null && !colors.includes(value);
  return (
    <div className="settings-colors">
      <button className={`settings-chip ${value === null ? "is-active" : ""}`} onClick={() => onChange(null)}>
        Par défaut
      </button>
      {colors.map((c) => (
        <button
          key={c}
          className={`settings-swatch ${value === c ? "is-active" : ""}`}
          style={{ background: c }}
          aria-label={c}
          onClick={() => onChange(c)}
        />
      ))}
      <label className={`settings-swatch settings-picker ${custom ? "is-active" : ""}`} title="Autre couleur" style={custom ? { background: value } : undefined}>
        <input type="color" value={value ?? "#808080"} onChange={(e) => onChange(e.target.value)} />
        {!custom && "+"}
      </label>
    </div>
  );
}

function Appearance({ settings, save }: SectionProps) {
  const custom = settings.accent !== null || settings.background !== null;
  return (
    <>
      <Group
        title="Thème"
        hint={custom ? "Couleurs personnalisées actives : choisir un thème les remplace." : undefined}
      >
        <div className="settings-segmented">
          {THEMES.map(([id, label]) => {
            const active = !custom && settings.theme === id;
            return (
              <button
                key={id}
                className={active ? "is-active" : ""}
                // Un thème prédéfini reprend la main sur les couleurs personnalisées.
                onClick={() => save({ theme: id, accent: null, background: null })}
              >
                {active && <motion.span layoutId="theme-active" className="settings-segmented-pill" transition={PILL} />}
                <span>{label}</span>
              </button>
            );
          })}
        </div>
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
  const toggle = (f: string) =>
    save({
      favorites: settings.favorites.includes(f) ? settings.favorites.filter((x) => x !== f) : [...settings.favorites, f],
    });
  return (
    <>
      <Group title="Formats favoris" hint="Ils apparaissent en premier dans le menu, dans l'ordre où tu les choisis.">
        {OUTPUTS.map(([label, formats]) => (
          <div key={label} className="settings-formats">
            <span>{label}</span>
            <div>
              {formats.map((f) => {
                const rank = settings.favorites.indexOf(f);
                return (
                  <button
                    key={f}
                    className={`settings-format kind-${kindOf(f)} ${rank >= 0 ? "is-active" : ""}`}
                    onClick={() => toggle(f)}
                  >
                    {rank >= 0 && <em>{rank + 1}</em>}
                    {f.toUpperCase()}
                  </button>
                );
              })}
            </div>
          </div>
        ))}
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
    <Group title="Où enregistrer les résultats" hint="Conversions et outils (Recadrer, Compresser, Éditer).">
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
      <p className="settings-hint">Un fichier existant n'est jamais écrasé : « (2) », « (3) »… sont ajoutés au nom.</p>
    </Group>
  );
}

function SystemSection({ settings, save, version }: SectionProps & { version: string }) {
  return (
    <>
      <Group title="Démarrage">
        <label className="settings-toggle">
          <input type="checkbox" checked={settings.autostart} onChange={(e) => save({ autostart: e.target.checked })} />
          <span>
            <strong>Lancer Morph à l'ouverture de session</strong>
            <small>Morph reste discret dans la zone de notification, prêt pour le Shift + glisser.</small>
          </span>
        </label>
      </Group>
      <Group title="Morph" hint={`Version ${version} · licence GPL-3.0-or-later`}>
        <div>
          <button className="settings-button" onClick={() => invoke("quit")}>
            Quitter Morph
          </button>
        </div>
        <p className="settings-hint">Fermer cette fenêtre laisse Morph actif ; l'icône de la zone de notification la rouvre.</p>
      </Group>
    </>
  );
}

export default App;
