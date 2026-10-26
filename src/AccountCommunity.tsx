// Avis et idées depuis l'application (compte Morph connecté) : mêmes données que les pages
// /reviews/ et /roadmap/ du site, par l'API du compte (account::call).

import { type FormEvent, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";

import { accountErrorText, callAccount } from "./account";
import { lang, plural, t } from "./i18n";
import { SITE } from "./HomeSection";

interface MyReview {
  rating: number;
  title: string | null;
  body: string;
  version: string | null;
  hidden: boolean;
}

/** Message de retour d'un formulaire : réussite ou erreur. */
type Msg = { ok: boolean; text: string } | null;

const Feedback = ({ msg }: { msg: Msg }) =>
  msg && (
    <p className={msg.ok ? "settings-ok" : "settings-error"} role="status">
      {msg.text}
    </p>
  );

/** Lien pour confirmer l'adresse e-mail sur le site (condition pour publier). */
const NotVerified = () => (
  <p className="settings-hint">
    {t("account.err_not_verified")}{" "}
    <button className="settings-link" onClick={() => invoke("open_link", { url: `${SITE}/account/` })}>
      {t("account.open_site")}
    </button>
  </p>
);

export function ReviewGroup() {
  const [loaded, setLoaded] = useState<{ review: MyReview | null; canPost: boolean } | null>(null);
  const [rating, setRating] = useState(0);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<Msg>(null);

  useEffect(() => {
    callAccount<{ review: MyReview | null; canPost: boolean }>("GET", "review")
      .then((d) => {
        setLoaded(d);
        if (d.review) {
          setRating(d.review.rating);
          setTitle(d.review.title ?? "");
          setBody(d.review.body);
        }
      })
      .catch((e) => setMsg({ ok: false, text: accountErrorText(e) }));
  }, []);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    try {
      await callAccount("POST", "review", { rating, title, body, version: await getVersion(), lang: lang() === "fr" ? "fr" : "en" });
      setMsg({ ok: true, text: loaded?.review ? t("account.review_updated") : t("account.review_published") });
      setLoaded((d) => d && { ...d, review: { rating, title, body, version: null, hidden: false } });
    } catch (err) {
      setMsg({ ok: false, text: accountErrorText(err) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-group">
      <h2>{t("account.review_title")}</h2>
      <p className="settings-hint">{t("account.review_hint")}</p>
      {loaded && !loaded.canPost ? (
        <NotVerified />
      ) : (
        <form className="account-form" onSubmit={submit}>
          <div className="stars-input" role="radiogroup" aria-label={t("account.review_rating")}>
            {[1, 2, 3, 4, 5].map((n) => (
              <button
                key={n}
                type="button"
                role="radio"
                aria-checked={rating === n}
                aria-label={plural(n, "account.review_stars")}
                className={n <= rating ? "is-on" : ""}
                onClick={() => setRating(n)}
              >
                ★
              </button>
            ))}
          </div>
          <input
            className="account-input"
            maxLength={100}
            value={title}
            placeholder={t("account.review_title_placeholder")}
            aria-label={t("account.review_title_placeholder")}
            onChange={(e) => setTitle(e.target.value)}
          />
          <textarea
            className="account-input"
            rows={4}
            maxLength={2000}
            value={body}
            placeholder={t("account.review_body_placeholder")}
            aria-label={t("account.review_body_placeholder")}
            onChange={(e) => setBody(e.target.value)}
          />
          {loaded?.review?.hidden && <p className="settings-hint">{t("account.review_hidden")}</p>}
          <div className="about-links">
            <button className="settings-button is-primary" type="submit" disabled={busy || !loaded || rating === 0 || body.trim().length < 10}>
              {loaded?.review ? t("account.review_update") : t("account.review_publish")}
            </button>
            <button type="button" className="settings-button" onClick={() => invoke("open_link", { url: `${SITE}/reviews/` })}>
              {t("account.review_see_all")}
            </button>
          </div>
          <Feedback msg={msg} />
        </form>
      )}
    </section>
  );
}

interface Idea {
  id: number;
  title: string;
  titleEn: string | null;
  status: "suggested" | "planned" | "in_progress";
  votes: number;
  voted: boolean;
}

export function IdeasGroup() {
  const [data, setData] = useState<{ items: Idea[]; canPost: boolean } | null>(null);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<Msg>(null);

  const load = () =>
    callAccount<{ items: Idea[]; canPost: boolean }>("GET", "ideas")
      .then(setData)
      .catch((e) => setMsg({ ok: false, text: accountErrorText(e) }));
  useEffect(() => {
    load();
  }, []);

  async function vote(idea: Idea) {
    try {
      const r = await callAccount<{ votes: number; voted: boolean }>("POST", "vote", { id: idea.id, vote: !idea.voted });
      setData((d) => d && { ...d, items: d.items.map((i) => (i.id === idea.id ? { ...i, ...r } : i)) });
    } catch (e) {
      setMsg({ ok: false, text: accountErrorText(e) });
    }
  }

  async function suggest(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    try {
      await callAccount("POST", "idea", { title, description });
      setTitle("");
      setDescription("");
      setMsg({ ok: true, text: t("account.idea_sent") });
      load();
    } catch (err) {
      setMsg({ ok: false, text: accountErrorText(err) });
    } finally {
      setBusy(false);
    }
  }

  const statusLabel = (s: Idea["status"]) =>
    ({ suggested: t("account.idea_suggested"), planned: t("account.idea_planned"), in_progress: t("account.idea_in_progress") })[s];

  return (
    <section className="settings-group">
      <h2>{t("account.ideas_title")}</h2>
      <p className="settings-hint">{t("account.ideas_hint")}</p>
      {data && data.items.length > 0 && (
        <ul className="ideas">
          {data.items.map((i) => (
            <li key={i.id}>
              <button
                className={`idea-vote ${i.voted ? "is-on" : ""}`}
                aria-pressed={i.voted}
                title={i.voted ? t("account.idea_unvote") : t("account.idea_vote")}
                onClick={() => vote(i)}
              >
                <span aria-hidden>▲</span>
                <span className="ext">{i.votes}</span>
              </button>
              <span className="idea-title">{lang() !== "fr" && i.titleEn ? i.titleEn : i.title}</span>
              <span className={`idea-status is-${i.status}`}>{statusLabel(i.status)}</span>
            </li>
          ))}
        </ul>
      )}
      {data && !data.canPost ? (
        <NotVerified />
      ) : (
        <form className="account-form" onSubmit={suggest}>
          <input
            className="account-input"
            maxLength={120}
            value={title}
            placeholder={t("account.idea_title_placeholder")}
            aria-label={t("account.idea_title_placeholder")}
            onChange={(e) => setTitle(e.target.value)}
          />
          <textarea
            className="account-input"
            rows={2}
            maxLength={1000}
            value={description}
            placeholder={t("account.idea_desc_placeholder")}
            aria-label={t("account.idea_desc_placeholder")}
            onChange={(e) => setDescription(e.target.value)}
          />
          <div className="about-links">
            <button className="settings-button" type="submit" disabled={busy || title.trim().length < 5}>
              {t("account.idea_send")}
            </button>
          </div>
        </form>
      )}
      <Feedback msg={msg} />
    </section>
  );
}
