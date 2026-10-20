// Télécharge ONNX Runtime (release officielle Microsoft, version et empreinte figées) pour le
// détourage, et le place dans src-tauri/onnxruntime/, embarqué par l'installeur
// (voir tauri.windows.conf.json / tauri.linux.conf.json). Morph le charge à la demande.
// Sous Windows, ajoute le runtime Visual C++ (redistribuable) dont la DLL a besoin, pris
// dans l'installation de Visual Studio (Build Tools).
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const VERSION = "1.31.0";
const ARCHIVES = {
  win32: {
    name: `onnxruntime-win-x64-${VERSION}.zip`,
    sha256: "3958d8a44984160692c1b4ffd0de3cb48f8c39f9d34bb31e433db06e30b82195",
    lib: "onnxruntime.dll",
  },
  linux: {
    name: `onnxruntime-linux-x64-${VERSION}.tgz`,
    sha256: "cc5c72baf5ae5c8238a6841f0897227be2d02826b9cf98eaf02fdefaeeb03a57",
    lib: `libonnxruntime.so.${VERSION}`,
    as: "libonnxruntime.so",
  },
};
/** DLL du runtime Visual C++ importées par onnxruntime.dll. */
const VC_RUNTIME = ["msvcp140.dll", "msvcp140_1.dll", "vcruntime140.dll", "vcruntime140_1.dll"];

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const archive = ARCHIVES[process.platform];
if (!archive) throw new Error(`Plateforme non supportée : ${process.platform}`);
const outDir = join(root, "src-tauri", "onnxruntime");
const outFile = join(outDir, archive.as ?? archive.lib);

if (existsSync(outFile) && !process.argv.includes("--force")) {
  console.log(`ONNX Runtime déjà présent : ${outFile} (utiliser --force pour le remplacer)`);
  process.exit(0);
}

const url = `https://github.com/microsoft/onnxruntime/releases/download/v${VERSION}/${archive.name}`;
console.log(`Téléchargement de ${archive.name}…`);
const bytes = Buffer.from(await (await fetch(url)).arrayBuffer());
const digest = createHash("sha256").update(bytes).digest("hex");
if (digest !== archive.sha256) throw new Error(`Empreinte inattendue pour ${archive.name} : ${digest}`);

const work = mkdtempSync(join(tmpdir(), "morph-ort-"));
const file = join(work, archive.name);
writeFileSync(file, bytes);
const tar = process.platform === "win32" ? join(process.env.SystemRoot ?? String.raw`C:\Windows`, "System32", "tar.exe") : "tar";
execFileSync(tar, ["-xf", file, "-C", work], { stdio: "inherit" });

mkdirSync(outDir, { recursive: true });
const folder = archive.name.replace(/\.(zip|tgz)$/, "");
copyFileSync(join(work, folder, "lib", archive.lib), outFile);
copyFileSync(join(work, folder, "LICENSE"), join(outDir, "LICENSE-onnxruntime.txt"));
rmSync(work, { recursive: true, force: true });

if (process.platform === "win32") {
  const vswhere = join(process.env["ProgramFiles(x86)"] ?? String.raw`C:\Program Files (x86)`, "Microsoft Visual Studio", "Installer", "vswhere.exe");
  // Redistribuable « bureau » x64 (pas les variantes onecore, debug ou arm64).
  const found = execFileSync(vswhere, ["-latest", "-products", "*", "-find", String.raw`VC\Redist\MSVC\**\msvcp140.dll`], { encoding: "utf8" })
    .split(/\r?\n/)
    .filter((p) => /\\x64\\Microsoft\.VC\d+\.CRT\\/i.test(p) && !/onecore|debug/i.test(p));
  if (!found.length) throw new Error("Runtime Visual C++ introuvable (installer les Build Tools de Visual Studio)");
  const crt = found[found.length - 1].replace(/\\msvcp140\.dll$/i, "");
  for (const dll of VC_RUNTIME) copyFileSync(join(crt, dll), join(outDir, dll));
  console.log(`Runtime Visual C++ : ${crt}`);
}
console.log(`ONNX Runtime ${VERSION} installé : ${outFile}`);
