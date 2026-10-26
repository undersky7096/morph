// Banc d'aperçu de l'interface dans un navigateur (développement seulement, jamais livré) :
// simule l'IPC de Tauri avec des données d'exemple pour regarder les fenêtres sans lancer
// l'application. `npm run dev` puis http://localhost:1420/dev/preview.html?window=main
// Paramètres : window=main|radial|progress|tool, theme=light|dark, lang=fr|en,
// section=home|account…, account=out|linking|in.
// Les données sont fictives et servent uniquement à juger la mise en page.

type Handler = (args: Record<string, unknown>) => unknown;

const params = new URLSearchParams(location.search);
const label = params.get("window") ?? "main";
const theme = params.get("theme") ?? "light";
const language = params.get("lang") ?? "fr";
if (params.get("section")) location.hash = params.get("section")!;
// Le volet d'aperçu tourne souvent en arrière-plan, où les animations sont ralenties :
// figées par défaut (motion=1 pour les voir).
if (params.get("motion") !== "1") {
  const style = document.createElement("style");
  style.textContent = "*, *::before, *::after { animation-duration: 0s !important; animation-delay: 0s !important; transition-duration: 0s !important; }";
  document.head.append(style);
}

const F = (ext: string, kind: string, extra: Partial<Record<string, unknown>> = {}) => ({
  ext,
  aliases: [],
  kind,
  read: "x",
  write: "x",
  compress: kind !== "archive",
  edit: kind === "image" || kind === "audio" || kind === "video",
  ...extra,
});

const catalog = [
  ...["png", "jpg", "webp", "avif", "gif", "bmp", "tiff", "ico", "tga"].map((e) => F(e, "image")),
  F("heic", "image", { write: null, edit: false }),
  F("svg", "image", { write: null, edit: false }),
  ...["mp3", "wav", "flac", "ogg", "aac", "m4a", "opus", "wma", "aiff"].map((e) => F(e, "audio")),
  ...["mp4", "mov", "mkv", "webm", "avi", "wmv", "flv", "3gp"].map((e) => F(e, "video")),
  ...["pdf", "docx", "odt", "rtf", "md", "html", "txt", "epub"].map((e) => F(e, "document", { compress: e === "pdf" })),
  ...["zip", "7z"].map((e) => F(e, "archive")),
  ...["tar", "gz", "rar"].map((e) => F(e, "archive", { write: null })),
];

const settings = {
  language,
  theme,
  accent: null,
  background: null,
  radialOpacity: 82,
  favorites: ["webp", "mp3"],
  mixedBatch: "groups",
  saveMode: "beside",
  saveFolder: null,
  duplicates: "keep",
  cutout: "fast",
  gpu: true,
  autostart: true,
  onboarded: params.get("onboarding") !== "1",
};

const accountMode = params.get("account") ?? "out";
const account = {
  user: accountMode === "in" ? { name: "Camille Exemple", email: "camille@example.com", avatar: null, createdAt: "2026-10-21 18:00:00" } : null,
  link: accountMode === "linking" ? { userCode: "K7QX-3MPA", verifyUrl: "#", expiresAtMs: Date.now() + 9 * 60_000 } : null,
  busy: false,
  error: null,
  errorCode: null,
  sync: accountMode === "in",
  lastSyncMs: accountMode === "in" ? Date.now() - 3_600_000 : null,
};

const job = {
  id: 1,
  target: "webp",
  note: null,
  detached: true,
  finished: false,
  startedMs: Date.now() - 4000,
  files: [
    { input: "C:/Exemple/vacances-01.jpg", output: "C:/Exemple/vacances-01.webp", status: "done", progress: 1, error: null, count: 1 },
    { input: "C:/Exemple/vacances-02.jpg", output: null, status: "running", progress: 0.42, error: null, count: 1 },
    { input: "C:/Exemple/un nom de fichier vraiment très long pour vérifier la coupure.png", output: null, status: "pending", progress: 0, error: null, count: 1 },
    {
      input: "C:/Exemple/abimé.png",
      output: null,
      status: "error",
      progress: 0,
      error: { code: "unreadable", message: "Image illisible ou endommagée.", detail: "exemple", path: null },
      count: 1,
    },
  ],
};

const MENU = {
  common: ["png", "jpg", "webp", "avif", "gif", "pdf"],
  groups: [
    {
      kind: "image",
      targets: ["png", "jpg", "webp", "avif", "gif", "bmp", "tiff", "ico", "pdf"],
      blocked: [],
      paths: ["C:/Exemple/photo.heic"],
    },
  ],
  folders: [],
  unsupported: [],
};

const handlers: Record<string, Handler> = {
  get_settings: () => settings,
  set_settings: ({ settings: s }) => Object.assign(settings, s),
  format_catalog: () => catalog,
  platform: () => "windows",
  update_status: () => ({ available: params.get("update") ? { version: "1.7.0", notes: "Exemple de notes." } : null, checking: false, installing: null, error: null, lastCheckMs: Date.now() - 600_000 }),
  account_status: () => account,
  reveal_settings: () => null,
  list_jobs: () => [job],
  take_radial_payload: () => ({
    id: 1,
    paths: ["C:/Exemple/photo.heic"],
    menu: MENU,
    ring: "formats",
  }),
  get_targets: () => (MENU),
  "plugin:app|version": () => "1.7.0-dev",
  "plugin:event|listen": () => 1,
  "plugin:event|unlisten": () => null,
  "plugin:window|theme": () => theme,
};

let nextCallback = 1;
(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label }, currentWebview: { label, windowLabel: label } },
  invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
    const h = handlers[cmd];
    if (!h) {
      console.debug("[aperçu] commande sans réponse simulée :", cmd, args);
      return null;
    }
    return h(args);
  },
  transformCallback: () => nextCallback++,
  unregisterCallback: () => {},
  convertFileSrc: (p: string) => p,
};
(window as unknown as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };

await import("../src/main");
