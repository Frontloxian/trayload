// Sets up Trayload's release configuration. Safe to run more than once.
//   node scripts/m13-config.mjs --repo YOURNAME/trayload [--version 1.0.0]
// Needs the updater public key made with `npm run tauri signer generate -- -w <path>` (see the M13 steps).
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const args = Object.fromEntries(
  process.argv.slice(2).reduce((acc, a, i, all) => {
    if (a.startsWith("--")) acc.push([a.slice(2), all[i + 1]]);
    return acc;
  }, []),
);
const repo = args.repo;
if (!repo || !/^[\w.-]+\/[\w.-]+$/.test(repo)) {
  console.error("Missing or invalid --repo. Example: --repo YOURNAME/trayload");
  process.exit(1);
}
const keyPub = path.resolve(args.pub ?? path.join(os.homedir(), ".tauri", "trayload.key.pub"));
if (!fs.existsSync(keyPub)) {
  console.error(`Public key not found: ${keyPub}\nCreate it first (M13 step 3).`);
  process.exit(1);
}
const pubkey = fs.readFileSync(keyPub, "utf8").trim();

const confPath = "src-tauri/tauri.conf.json";
const conf = JSON.parse(fs.readFileSync(confPath, "utf8"));
const version = args.version ?? conf.version;
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`Version must look like 1.0.0, got "${version}"`);
  process.exit(1);
}

conf.version = version;
conf.bundle = conf.bundle ?? {};
conf.bundle.active = true;
conf.bundle.targets = ["nsis"];
conf.bundle.publisher = "Trayload contributors";
conf.bundle.copyright = "Copyright (c) 2026 Trayload contributors";
conf.bundle.category = "Game";
conf.bundle.shortDescription = "Emulator launcher for Windows";
conf.bundle.licenseFile = "../LICENSE";
conf.bundle.createUpdaterArtifacts = true;
conf.bundle.windows = conf.bundle.windows ?? {};
conf.bundle.windows.nsis = { ...(conf.bundle.windows.nsis ?? {}), installMode: "currentUser" };
conf.plugins = conf.plugins ?? {};
conf.plugins.updater = {
  pubkey,
  endpoints: [`https://github.com/${repo}/releases/latest/download/latest.json`],
};
fs.writeFileSync(confPath, JSON.stringify(conf, null, 2) + "\n");

// Keep package.json and Cargo.toml on the same version.
const pkgPath = "package.json";
const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
pkg.version = version;
fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

const cargoPath = "src-tauri/Cargo.toml";
let cargo = fs.readFileSync(cargoPath, "utf8");
cargo = cargo.replace(/(\[package\][\s\S]*?\nversion\s*=\s*")[^"]+(")/, `$1${version}$2`);
fs.writeFileSync(cargoPath, cargo);

console.log(`Done. Version ${version}, updates from github.com/${repo}.`);
