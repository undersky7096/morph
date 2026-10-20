// Télécharge un binaire ffmpeg statique (builds BtbN, branche stable GPL) et le place
// dans src-tauri/binaries/ sous le nom attendu par Tauri pour un sidecar :
// ffmpeg-<target-triple>[.exe]
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const isWin = process.platform === "win32";
const platform = isWin ? "win64" : process.platform === "linux" ? "linux64" : null;
if (!platform) throw new Error(`Plateforme non supportée : ${process.platform}`);

const triple = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (\S+)/m)[1];
const outDir = join(root, "src-tauri", "binaries");
const outFile = join(outDir, `ffmpeg-${triple}${isWin ? ".exe" : ""}`);

if (existsSync(outFile) && !process.argv.includes("--force")) {
  console.log(`ffmpeg déjà présent : ${outFile} (utiliser --force pour le remplacer)`);
  process.exit(0);
}

// En CI, le jeton évite la limite de 60 requêtes/h des appels anonymes à l'API GitHub.
const headers = process.env.GITHUB_TOKEN ? { Authorization: `Bearer ${process.env.GITHUB_TOKEN}` } : {};
const release = await (await fetch("https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest", { headers })).json();
const pattern = new RegExp(`^ffmpeg-n[\\d.]+-latest-${platform}-gpl-[\\d.]+\\.(zip|tar\\.xz)$`);
const asset = release.assets
  .filter((a) => pattern.test(a.name))
  .sort((a, b) => b.name.localeCompare(a.name, undefined, { numeric: true }))[0];
if (!asset) throw new Error(`Aucun build ffmpeg trouvé pour ${platform}`);

console.log(`Téléchargement de ${asset.name}…`);
const work = mkdtempSync(join(tmpdir(), "morph-ffmpeg-"));
const archive = join(work, asset.name);
writeFileSync(archive, Buffer.from(await (await fetch(asset.browser_download_url)).arrayBuffer()));

const extracted = join(work, "x");
mkdirSync(extracted);
const tar = isWin ? join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe") : "tar";
execFileSync(tar, ["-xf", archive, "-C", extracted], { stdio: "inherit" });

const findBin = (dir) => {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      const found = findBin(p);
      if (found) return found;
    } else if (name === (isWin ? "ffmpeg.exe" : "ffmpeg")) {
      return p;
    }
  }
  return null;
};
const bin = findBin(extracted);
if (!bin) throw new Error("ffmpeg introuvable dans l'archive");

mkdirSync(outDir, { recursive: true });
copyFileSync(bin, outFile);
rmSync(work, { recursive: true, force: true });
console.log(`ffmpeg installé : ${outFile}`);
