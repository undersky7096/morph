// Télécharge ONNX Runtime (release officielle Microsoft, version et empreinte figées) pour le
// détourage, et le place dans src-tauri/onnxruntime/, embarqué par l'installeur
// (voir tauri.windows.conf.json / tauri.linux.conf.json). Morph le charge à la demande.
// Sous Windows, c'est la version DirectML (calcul sur la carte graphique, paquets NuGet
// officiels Microsoft.ML.OnnxRuntime.DirectML et Microsoft.AI.DirectML), avec le runtime
// Visual C++ (redistribuable) dont la DLL a besoin, pris dans l'installation de Visual
// Studio (Build Tools).
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const VERSION = "1.31.0";
const NUGET = "https://api.nuget.org/v3-flatcontainer";
/** Fichiers à extraire de chaque archive : chemin dans l'archive → nom dans onnxruntime/. */
const ARCHIVES = {
  win32: [
    {
      url: `${NUGET}/microsoft.ml.onnxruntime.directml/1.24.4/microsoft.ml.onnxruntime.directml.1.24.4.nupkg`,
      sha256: "57e9f11b73437bef7a309496135d4c1f96b1a8e9ddba60013fa27bfc1d788681",
      files: { "runtimes/win-x64/native/onnxruntime.dll": "onnxruntime.dll", LICENSE: "LICENSE-onnxruntime.txt" },
    },
    {
      url: `${NUGET}/microsoft.ai.directml/1.15.4/microsoft.ai.directml.1.15.4.nupkg`,
      sha256: "4e7cb7ddce8cf837a7a75dc029209b520ca0101470fcdf275c1f49736a3615b9",
      files: { "bin/x64-win/DirectML.dll": "DirectML.dll", "LICENSE.txt": "LICENSE-directml.txt" },
    },
  ],
  linux: [
    {
      url: `https://github.com/microsoft/onnxruntime/releases/download/v${VERSION}/onnxruntime-linux-x64-${VERSION}.tgz`,
      sha256: "cc5c72baf5ae5c8238a6841f0897227be2d02826b9cf98eaf02fdefaeeb03a57",
      files: {
        [`onnxruntime-linux-x64-${VERSION}/lib/libonnxruntime.so.${VERSION}`]: "libonnxruntime.so",
        [`onnxruntime-linux-x64-${VERSION}/LICENSE`]: "LICENSE-onnxruntime.txt",
      },
    },
  ],
};
/** DLL du runtime Visual C++ importées par onnxruntime.dll. */
const VC_RUNTIME = ["msvcp140.dll", "msvcp140_1.dll", "vcruntime140.dll", "vcruntime140_1.dll"];

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const archives = ARCHIVES[process.platform];
if (!archives) throw new Error(`Plateforme non supportée : ${process.platform}`);
const outDir = join(root, "src-tauri", "onnxruntime");
const outFile = join(outDir, process.platform === "win32" ? "DirectML.dll" : "libonnxruntime.so");

if (existsSync(outFile) && !process.argv.includes("--force")) {
  console.log(`ONNX Runtime déjà présent : ${outFile} (utiliser --force pour le remplacer)`);
  process.exit(0);
}

/** Téléchargement, réessayé deux fois (connexion parfois capricieuse vers NuGet). */
async function download(url) {
  for (let attempt = 1; ; attempt++) {
    try {
      return Buffer.from(await (await fetch(url)).arrayBuffer());
    } catch (e) {
      if (attempt === 3) throw e;
      console.log(`Nouvel essai (${e.cause?.code ?? e.message})…`);
    }
  }
}

const tar = process.platform === "win32" ? join(process.env.SystemRoot ?? String.raw`C:\Windows`, "System32", "tar.exe") : "tar";
mkdirSync(outDir, { recursive: true });
for (const archive of archives) {
  const name = archive.url.split("/").pop();
  console.log(`Téléchargement de ${name}…`);
  const bytes = await download(archive.url);
  const digest = createHash("sha256").update(bytes).digest("hex");
  if (digest !== archive.sha256) throw new Error(`Empreinte inattendue pour ${name} : ${digest}`);

  const work = mkdtempSync(join(tmpdir(), "morph-ort-"));
  const file = join(work, name);
  writeFileSync(file, bytes);
  const content = join(work, "content");
  mkdirSync(content);
  // Un .nupkg est une archive ZIP, que tar lit aussi.
  execFileSync(tar, ["-xf", file, "-C", content], { stdio: "inherit" });
  for (const [from, to] of Object.entries(archive.files)) copyFileSync(join(content, from), join(outDir, to));
  rmSync(work, { recursive: true, force: true });
}

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
console.log(`ONNX Runtime installé : ${outDir}`);
