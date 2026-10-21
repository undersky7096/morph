// Télécharge pdfium (moteur PDF de Chromium, builds bblanchon/pdfium-binaries, version et
// empreinte figées) et le place dans src-tauri/pdfium/, embarqué par l'installeur (voir
// tauri.windows.conf.json / tauri.linux.conf.json). Morph le charge à la demande pour lire,
// assembler et fusionner les PDF. La version suit celle que vise la crate `pdfium-render`.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const VERSION = "7881";
const ARCHIVES = {
  win32: {
    name: "pdfium-win-x64.tgz",
    sha256: "73cc0de638ac2095e7445bf56a38200a5b7c7ca0e9f4ba144598f2457377ac08",
    lib: join("bin", "pdfium.dll"),
    as: "pdfium.dll",
  },
  linux: {
    name: "pdfium-linux-x64.tgz",
    sha256: "1470e21b8b4a3b4ad7f85684e2da11d94f3b69a86d81dee11b9b6709d927ac1d",
    lib: join("lib", "libpdfium.so"),
    as: "libpdfium.so",
  },
};

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const archive = ARCHIVES[process.platform];
if (!archive) throw new Error(`Plateforme non supportée : ${process.platform}`);
const outDir = join(root, "src-tauri", "pdfium");
const outFile = join(outDir, archive.as);

if (existsSync(outFile) && !process.argv.includes("--force")) {
  console.log(`pdfium déjà présent : ${outFile} (utiliser --force pour le remplacer)`);
  process.exit(0);
}

const url = `https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/${VERSION}/${archive.name}`;
console.log(`Téléchargement de ${archive.name} (chromium/${VERSION})…`);
const bytes = Buffer.from(await (await fetch(url)).arrayBuffer());
const digest = createHash("sha256").update(bytes).digest("hex");
if (digest !== archive.sha256) throw new Error(`Empreinte inattendue pour ${archive.name} : ${digest}`);

const work = mkdtempSync(join(tmpdir(), "morph-pdfium-"));
const file = join(work, archive.name);
writeFileSync(file, bytes);
const tar = process.platform === "win32" ? join(process.env.SystemRoot ?? String.raw`C:\Windows`, "System32", "tar.exe") : "tar";
execFileSync(tar, ["-xf", file, "-C", work], { stdio: "inherit" });

mkdirSync(outDir, { recursive: true });
copyFileSync(join(work, archive.lib), outFile);
copyFileSync(join(work, "LICENSE"), join(outDir, "LICENSE-pdfium.txt"));
rmSync(work, { recursive: true, force: true });
console.log(`pdfium chromium/${VERSION} installé : ${outFile}`);
