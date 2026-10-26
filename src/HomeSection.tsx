// Onglet Accueil des Réglages : rappel du geste, ce que Morph sait faire (avec de vrais
// raccourcis : choisir des fichiers ouvre le menu ou l'outil), quelques astuces vérifiées,
// la version et le compte. Rien d'inventé : chaque chiffre vient de la table des formats.

import type { ReactElement } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import { type FormatInfo, readableCount, readableExts } from "./formats";
import { plural, t } from "./i18n";
import {
  ArrowRightIcon,
  CompressIcon,
  ConvertIcon,
  CropIcon,
  CutoutIcon,
  EditIcon,
  ExternalIcon,
  ExtractIcon,
  UserIcon,
} from "./icons";
import type { Settings } from "./settings";
import type { AccountStatus } from "./AccountSection";
import Logo from "./Logo";

export type HomeTarget = "appearance" | "menu" | "save" | "tools" | "account" | "about";

export const SITE = "https://morph.underskydev.com";

interface Props {
  settings: Settings;
  version: string;
  update: { available: { version: string } | null; checking: boolean } | null;
  account: AccountStatus | null;
  goTo: (section: HomeTarget) => void;
}

/** Une capacité de Morph : ce qu'elle fait, sur quoi, et un raccourci réel. */
interface Capability {
  id: string;
  icon: () => ReactElement;
  title: string;
  detail: string;
  /** Formats proposés par la boîte d'ouverture. */
  accepts: (f: FormatInfo) => boolean;
  /** Outil à ouvrir sur les fichiers choisis ; sinon, le menu des formats. */
  tool?: "crop" | "compress" | "edit";
  multiple: boolean;
}

const capabilities = (): Capability[] => [
  {
    id: "convert",
    icon: ConvertIcon,
    title: t("home.cap_convert"),
    detail: plural(readableCount(), "home.cap_convert_detail"),
    accepts: () => true,
    multiple: true,
  },
  {
    id: "compress",
    icon: CompressIcon,
    title: t("tools.compress"),
    detail: t("home.cap_compress_detail"),
    accepts: (f) => f.compress,
    tool: "compress",
    multiple: true,
  },
  {
    id: "edit",
    icon: EditIcon,
    title: t("tools.edit"),
    detail: t("home.cap_edit_detail"),
    accepts: (f) => f.edit,
    tool: "edit",
    multiple: false,
  },
  {
    id: "crop",
    icon: CropIcon,
    title: t("tools.crop"),
    detail: t("home.cap_crop_detail"),
    accepts: (f) => f.edit && f.kind === "image",
    tool: "crop",
    multiple: false,
  },
  {
    id: "cutout",
    icon: CutoutIcon,
    title: t("tools.remove_bg"),
    detail: t("home.cap_cutout_detail"),
    accepts: (f) => f.kind === "image" || f.kind === "video",
    multiple: true,
  },
  {
    id: "archive",
    icon: ExtractIcon,
    title: t("home.cap_archives"),
    detail: t("home.cap_archives_detail"),
    accepts: (f) => f.kind === "archive",
    multiple: true,
  },
];

/** Choisit des fichiers, puis ouvre l'outil ou le menu des formats sur eux. */
async function pick(cap: Capability) {
  const extensions = readableExts(cap.accepts);
  const chosen = await open({
    multiple: cap.multiple,
    title: cap.title,
    filters: [{ name: cap.title, extensions }],
  });
  const paths = chosen === null ? [] : Array.isArray(chosen) ? chosen : [chosen];
  if (!paths.length) return;
  if (cap.tool) await invoke("open_tool", { tool: cap.tool, paths });
  else await invoke("open_radial", { paths });
}

export default function HomeSection({ settings, version, update, account, goTo }: Props) {
  const link = (url: string) => invoke("open_link", { url });
  const saveHint = { beside: t("home.step3_beside"), folder: t("home.step3_folder"), ask: t("home.step3_ask") }[settings.saveMode];
  const caps = capabilities();

  return (
    <div className="home">
      <header className="home-head">
        <Logo className="home-logo" />
        <div className="home-title">
          <h1>{t("home.title")}</h1>
          <p>{t("home.lede")}</p>
        </div>
        <button className={`home-version ${update?.available ? "is-update" : ""}`} onClick={() => goTo("about")}>
          <span className="ext">v{version}</span>
          <span>
            {update?.available
              ? t("home.update_ready", { version: update.available.version })
              : update?.checking
                ? t("settings.checking_updates")
                : t("home.up_to_date")}
          </span>
        </button>
      </header>

      <AccountBanner account={account} goTo={goTo} />

      <section className="home-gesture" aria-labelledby="home-gesture-title">
        <GestureFigure />
        <div className="home-gesture-text">
          <h2 id="home-gesture-title">{t("home.gesture_title")}</h2>
          <ol className="home-steps">
            <li>
              <span className="home-step-n">1</span>
              <span>
                {t("home.step1_before")} <kbd>Shift</kbd> {t("home.step1_after")}
              </span>
            </li>
            <li>
              <span className="home-step-n">2</span>
              <span>{t("home.step2")}</span>
            </li>
            <li>
              <span className="home-step-n">3</span>
              <span>
                {saveHint}{" "}
                <button className="settings-link" onClick={() => goTo("save")}>
                  {t("home.change")}
                </button>
              </span>
            </li>
          </ol>
          <p className="home-drop-hint">{t("home.drop_hint")}</p>
        </div>
      </section>

      <section className="home-block" aria-labelledby="home-caps-title">
        <h2 id="home-caps-title">{t("home.caps_title")}</h2>
        <ul className="home-caps">
          {caps.map((cap) => (
            <li key={cap.id}>
              <button className="home-cap" onClick={() => pick(cap).catch(() => {})}>
                <span className="home-cap-icon">
                  <cap.icon />
                </span>
                <span className="home-cap-text">
                  <strong>{cap.title}</strong>
                  <small>{cap.detail}</small>
                </span>
                <span className="home-cap-go">
                  {t("home.choose_files")}
                  <ArrowRightIcon />
                </span>
              </button>
            </li>
          ))}
        </ul>
        <p className="settings-hint home-local">{t("home.local_note")}</p>
      </section>

      <section className="home-block" aria-labelledby="home-tips-title">
        <h2 id="home-tips-title">{t("home.tips_title")}</h2>
        <ul className="home-tips">
          <li>
            <span>{t("home.tip_favorites")}</span>
            <button className="settings-link" onClick={() => goTo("menu")}>
              {t("settings.format_menu")}
            </button>
          </li>
          <li>
            <span>{t("home.tip_escape")}</span>
          </li>
          <li>
            <span>{t("home.tip_duplicates")}</span>
            <button className="settings-link" onClick={() => goTo("save")}>
              {t("settings.saving")}
            </button>
          </li>
          <li>
            <span>{t("home.tip_cutout")}</span>
            <button className="settings-link" onClick={() => goTo("tools")}>
              {t("settings.tools")}
            </button>
          </li>
        </ul>
      </section>

      <footer className="home-links">
        {(
          [
            [t("settings.website"), SITE],
            [t("settings.all_release_notes"), `${SITE}/changelog/`],
            [t("settings.leave_review"), `${SITE}/reviews/#write`],
            [t("settings.suggest_idea"), `${SITE}/roadmap/`],
          ] as const
        ).map(([label, url]) => (
          <button key={url} className="home-link" onClick={() => link(url)}>
            {label.replace(/\s*↗$/, "")}
            <ExternalIcon />
          </button>
        ))}
      </footer>
    </div>
  );
}

/** Compte Morph, en tête de l'Accueil : se connecter en un clic, ou qui est connecté. */
function AccountBanner({ account, goTo }: Pick<Props, "account" | "goTo">) {
  if (!account) return null;
  const { user, link, busy } = account;
  return (
    <section className={`home-account ${user ? "is-signed-in" : ""}`} aria-labelledby="home-account-title">
      <span className="avatar home-account-avatar" aria-hidden>
        {user ? initials(user.name) : <UserIcon />}
      </span>
      <div className="home-account-text">
        <h2 id="home-account-title">{user ? user.name : t("account.title")}</h2>
        <p>
          {user
            ? account.sync
              ? t("account.sync_on_short")
              : t("account.sync_off_short")
            : link
              ? t("account.banner_waiting", { code: link.userCode })
              : t("account.pitch")}
        </p>
      </div>
      {user || link ? (
        <button className="settings-button" onClick={() => goTo("account")}>
          {user ? t("account.manage") : t("account.banner_see_code")}
        </button>
      ) : (
        <button
          className="settings-button is-primary"
          disabled={busy}
          onClick={() => {
            invoke("account_link_start");
            goTo("account");
          }}
        >
          {t("account.sign_in")}
        </button>
      )}
    </section>
  );
}

export const initials = (name: string) =>
  name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]!.toUpperCase())
    .join("");

/** Le geste, dessiné : la touche Maj, un fichier qui glisse, puis l'anneau de pétales.
 *  Illustration (pas une capture) ; elle se joue une fois à l'ouverture. */
function GestureFigure() {
  const petals = ["webp", "png", "jpg", "avif", "pdf", "gif"];
  return (
    <figure className="gesture" aria-hidden>
      <span className="gesture-key">Shift</span>
      <span className="gesture-file">
        <span className="ext">photo.heic</span>
      </span>
      <span className="gesture-ring">
        {petals.map((p, i) => (
          <span key={p} className="gesture-petal ext" style={{ "--i": i } as React.CSSProperties}>
            {p}
          </span>
        ))}
        <span className="gesture-hub">1</span>
      </span>
    </figure>
  );
}
