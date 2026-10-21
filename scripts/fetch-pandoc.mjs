// Télécharge pandoc (release officielle, version et empreinte figées) et le place dans
// src-tauri/binaries/ sous le nom attendu par Tauri pour un sidecar :
// pandoc-<target-triple>[.exe]. Sa licence (GPL) est copiée dans src-tauri/pandoc/.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const VERSION = "3.12.1";
const ARCHIVES = {
  win32: {
    name: `pandoc-${VERSION}-windows-x86_64.zip`,
    sha256: "6ef431cb20b2c24a0371fc19200dc49d452be54b1b5c9239f3462841ef53e587",
    bin: join(`pandoc-${VERSION}`, "pandoc.exe"),
    license: join(`pandoc-${VERSION}`, "COPYING.rtf"),
  },
  linux: {
    name: `pandoc-${VERSION}-linux-amd64.tar.gz`,
    sha256: "d0c90410e90204c9ca83b8539fac5c7aed01fd537207e4585849f8abc5df20b8",
    bin: join(`pandoc-${VERSION}`, "bin", "pandoc"),
    license: null,
  },
};

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const isWin = process.platform === "win32";
const archive = ARCHIVES[process.platform];
if (!archive) throw new Error(`Plateforme non supportée : ${process.platform}`);
const triple = execFileSync("rustc", ["-vV"], { encoding: "utf8" }).match(/^host: (\S+)/m)[1];
const outDir = join(root, "src-tauri", "binaries");
const outFile = join(outDir, `pandoc-${triple}${isWin ? ".exe" : ""}`);

if (existsSync(outFile) && !process.argv.includes("--force")) {
  console.log(`pandoc déjà présent : ${outFile} (utiliser --force pour le remplacer)`);
  process.exit(0);
}

const url = `https://github.com/jgm/pandoc/releases/download/${VERSION}/${archive.name}`;
console.log(`Téléchargement de ${archive.name}…`);
const bytes = Buffer.from(await (await fetch(url)).arrayBuffer());
const digest = createHash("sha256").update(bytes).digest("hex");
if (digest !== archive.sha256) throw new Error(`Empreinte inattendue pour ${archive.name} : ${digest}`);

const work = mkdtempSync(join(tmpdir(), "morph-pandoc-"));
const file = join(work, archive.name);
writeFileSync(file, bytes);
const tar = isWin ? join(process.env.SystemRoot ?? String.raw`C:\Windows`, "System32", "tar.exe") : "tar";
execFileSync(tar, ["-xf", file, "-C", work], { stdio: "inherit" });

mkdirSync(outDir, { recursive: true });
copyFileSync(join(work, archive.bin), outFile);
const licenseDir = join(root, "src-tauri", "pandoc");
mkdirSync(licenseDir, { recursive: true });
// L'archive Linux ne contient pas la licence : c'est la GPL-2.0-or-later (voir pandoc.org).
if (archive.license) copyFileSync(join(work, archive.license), join(licenseDir, "LICENSE-pandoc.rtf"));
rmSync(work, { recursive: true, force: true });
console.log(`pandoc ${VERSION} installé : ${outFile}`);
