// État du compte Morph (src-tauri/src/account.rs), partagé par les fenêtres : Réglages,
// outils (préréglages de Compresser), mini-fenêtre de progression (signalement).

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { t } from "./i18n";

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

/** Appel de l'API du site avec le jeton du compte (avis, idées, signalement). */
export function callAccount<T>(method: "GET" | "POST", action: string, body?: object): Promise<T> {
  return invoke<T>("account_call", { method, action, body: body ?? null });
}

/** Message lisible pour un code d'erreur de l'API (voir account::call). */
export function accountErrorText(e: unknown): string {
  const code = String(e);
  const known: Record<string, string> = {
    network: "core.account_network",
    session_expired: "core.account_session_expired",
    too_many_requests: "core.account_too_many",
    not_verified: "account.err_not_verified",
    review_too_short: "account.err_review_too_short",
    invalid_rating: "account.err_invalid_rating",
    title_too_short: "account.err_title_too_short",
    closed: "account.err_closed",
    signed_out: "account.err_signed_out",
  };
  return t(known[code] ?? "core.account_server_error");
}

/** Ouvre les Réglages sur l'onglet Compte (depuis un outil ou la progression). */
export const openAccountSettings = () => invoke("open_settings", { section: "account" });
