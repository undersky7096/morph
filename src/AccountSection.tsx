// Onglet Compte des Réglages : liaison facultative au compte du site (voir
// src-tauri/src/account.rs). Toutes les fonctions de Morph restent disponibles sans compte.

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { locale, t } from "./i18n";
import { initials, SITE } from "./HomeSection";

export interface AccountUser {
  name: string;
  email: string | null;
  avatar: string | null;
  createdAt: string | null;
}

export interface AccountStatus {
  user: AccountUser | null;
  link: { userCode: string; verifyUrl: string; expiresAtMs: number } | null;
  busy: boolean;
  error: string | null;
  errorCode: string | null;
  sync: boolean;
  lastSyncMs: number | null;
}

/** État du compte, suivi en direct (événement `account-changed`). */
export function useAccount(): AccountStatus | null {
  const [status, setStatus] = useState<AccountStatus | null>(null);
  useEffect(() => {
    invoke<AccountStatus>("account_status").then(setStatus);
    const unlisten = listen<AccountStatus>("account-changed", ({ payload }) => setStatus(payload));
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);
  return status;
}

/** Secondes restantes avant l'expiration du code (mise à jour chaque seconde). */
function useRemaining(until: number | null) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (until === null) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [until]);
  return until === null ? 0 : Math.max(0, Math.round((until - now) / 1000));
}

export default function AccountSection({ status }: { status: AccountStatus | null }) {
  const remaining = useRemaining(status?.link?.expiresAtMs ?? null);
  const [copied, setCopied] = useState(false);
  if (!status) return null;
  const { user, link, busy, error, sync, lastSyncMs } = status;

  const copy = (code: string) => {
    navigator.clipboard.writeText(code).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };

  return (
    <>
      <section className="settings-group">
        <h2>{t("account.title")}</h2>
        <p className="settings-hint">{t("account.intro")}</p>

        {user ? (
          <div className="account-card">
            <span className="avatar avatar-lg" aria-hidden>
              {initials(user.name)}
            </span>
            <div className="account-who">
              <strong>{user.name}</strong>
              {user.email && <small>{user.email}</small>}
              {user.createdAt && (
                <small>
                  {t("account.member_since", {
                    date: new Date(user.createdAt.replace(" ", "T")).toLocaleDateString(locale(), { year: "numeric", month: "long" }),
                  })}
                </small>
              )}
            </div>
            <div className="account-actions">
              <button className="settings-button" onClick={() => invoke("open_link", { url: `${SITE}/account/` })}>
                {t("account.open_site")}
              </button>
              <button className="settings-button" disabled={busy} onClick={() => invoke("account_logout")}>
                {t("account.sign_out")}
              </button>
            </div>
          </div>
        ) : link ? (
          <div className="account-link" role="status" aria-live="polite">
            <p>{t("account.link_instructions")}</p>
            <div className="account-code">
              <code className="ext">{link.userCode}</code>
              <button className="settings-button" onClick={() => copy(link.userCode)}>
                {copied ? t("tools.copied") : t("account.copy_code")}
              </button>
            </div>
            <p className="settings-hint">
              <span className="account-waiting" aria-hidden />
              {t("account.waiting", { minutes: Math.ceil(remaining / 60) })}
            </p>
            <div className="about-links">
              <button className="settings-button" onClick={() => invoke("open_link", { url: link.verifyUrl })}>
                {t("account.reopen_page")}
              </button>
              <button className="settings-button" onClick={() => invoke("account_link_cancel")}>
                {t("progress.cancel")}
              </button>
            </div>
          </div>
        ) : (
          <div className="account-signed-out">
            <ul className="account-benefits">
              <li>{t("account.benefit_sync")}</li>
              <li>{t("account.benefit_site")}</li>
              <li>{t("account.benefit_optional")}</li>
            </ul>
            <button className="settings-button is-primary" disabled={busy} onClick={() => invoke("account_link_start")}>
              {busy ? t("account.connecting") : t("account.sign_in")}
            </button>
            <p className="settings-hint">{t("account.no_account_yet")}</p>
          </div>
        )}
        {error && <p className="settings-error">{error}</p>}
      </section>

      {user && (
        <section className="settings-group">
          <h2>{t("account.sync_title")}</h2>
          <label className="settings-toggle">
            <input type="checkbox" checked={sync} onChange={(e) => invoke("account_set_sync", { enabled: e.target.checked })} />
            <span>
              <strong>{t("account.sync_label")}</strong>
              <small>{t("account.sync_detail")}</small>
              {sync && lastSyncMs ? (
                <small>
                  {t("account.last_sync", {
                    date: new Date(lastSyncMs).toLocaleString(locale(), { dateStyle: "medium", timeStyle: "short" }),
                  })}
                </small>
              ) : null}
            </span>
          </label>
        </section>
      )}

      <section className="settings-group">
        <h2>{t("account.privacy_title")}</h2>
        <p className="settings-hint">{t("account.privacy_detail")}</p>
      </section>
    </>
  );
}
