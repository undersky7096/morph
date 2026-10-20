// Écrit le manifeste des mises à jour (latest.json) d'une release, à partir des signatures
// des installeurs : node scripts/updater-json.mjs v1.0.0 dossier-des-.sig > latest.json
// Morph le lit à l'adresse …/releases/latest/download/latest.json (voir tauri.conf.json).
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const [tag, dir] = process.argv.slice(2);
if (!tag || !dir) throw new Error("usage : updater-json.mjs <tag> <dossier des .sig>");
const version = tag.replace(/^v/, "");
const base = `https://github.com/undersky7096/morph/releases/download/${tag}`;

/** Plateforme Tauri de chaque installeur (NSIS pour Windows, AppImage pour Linux). */
const PLATFORMS = [
  ["windows-x86_64", (f) => f.endsWith("-setup.exe.sig")],
  ["linux-x86_64", (f) => f.endsWith(".AppImage.sig")],
];

const platforms = {};
for (const file of readdirSync(dir)) {
  const platform = PLATFORMS.find(([, match]) => match(file))?.[0];
  if (!platform) continue;
  platforms[platform] = {
    signature: readFileSync(join(dir, file), "utf8").trim(),
    url: `${base}/${encodeURIComponent(file.replace(/\.sig$/, ""))}`,
  };
}
for (const [platform] of PLATFORMS) {
  if (!platforms[platform]) throw new Error(`Signature manquante pour ${platform}`);
}

const notesFile = join(".github", "release-notes", `${tag}.md`);
const notes = existsSync(notesFile) ? readFileSync(notesFile, "utf8").trim() : "";
console.log(JSON.stringify({ version, notes, pub_date: new Date().toISOString(), platforms }, null, 2));
