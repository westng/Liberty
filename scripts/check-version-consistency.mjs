import { readFileSync } from "node:fs";

const rootPackage = JSON.parse(readFileSync("package.json", "utf8"));
const desktopPackage = JSON.parse(readFileSync("apps/desktop/package.json", "utf8"));
const tauriConfig = JSON.parse(readFileSync("apps/desktop/src-tauri/tauri.conf.json", "utf8"));
const cargoToml = readFileSync("apps/desktop/src-tauri/Cargo.toml", "utf8");
const cargoVersion = cargoToml.match(/^version = "([^"]+)"$/m)?.[1];

const errors = [];

if (!cargoVersion) {
  errors.push("Unable to read version from apps/desktop/src-tauri/Cargo.toml.");
}

const releaseVersions = new Set([
  desktopPackage.version,
  tauriConfig.version,
  cargoVersion,
].filter(Boolean));

if (releaseVersions.size !== 1) {
  errors.push(
    `Desktop release versions must match. Found package=${desktopPackage.version}, tauri=${tauriConfig.version}, cargo=${cargoVersion}.`,
  );
}

if (rootPackage.packageManager !== "pnpm@10.30.3") {
  errors.push("Root packageManager must remain pinned to pnpm@10.30.3.");
}

if (errors.length > 0) {
  console.error(errors.map((error) => `- ${error}`).join("\n"));
  process.exit(1);
}

console.log("Version metadata is consistent.");
