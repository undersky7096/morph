// Change la version de Morph partout : `npm run bump -- 0.5.0`
// (package.json, Cargo.toml ; tauri.conf.json lit celle de package.json).
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error("usage : npm run bump -- <x.y.z>");
  process.exit(1);
}

const root = new URL("..", import.meta.url);
const pkgUrl = new URL("package.json", root);
const pkg = JSON.parse(readFileSync(pkgUrl, "utf8"));
pkg.version = version;
writeFileSync(pkgUrl, JSON.stringify(pkg, null, 2) + "\n");

const cargoUrl = new URL("src-tauri/Cargo.toml", root);
const cargo = readFileSync(cargoUrl, "utf8").replace(/^version = ".*"$/m, `version = "${version}"`);
writeFileSync(cargoUrl, cargo);

console.log(`Version ${version}`);
