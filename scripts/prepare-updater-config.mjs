import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const desktopRoot = path.join(repoRoot, "apps", "desktop");

const repo = process.env.LIBERTY_UPDATER_REPOSITORY?.trim() || "westng/Liberty";
const tauriPubkey = process.env.LIBERTY_TAURI_UPDATER_PUBLIC_KEY?.trim() || "__LIBERTY_TAURI_UPDATER_PUBLIC_KEY__";
const sparklePubkey = process.env.LIBERTY_SPARKLE_PUBLIC_KEY?.trim() || "__LIBERTY_SPARKLE_PUBLIC_KEY__";

const tauriConfigPath = path.join(desktopRoot, "src-tauri", "tauri.conf.json");
const infoPlistPath = path.join(desktopRoot, "src-tauri", "Info.plist");

const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, "utf8"));
tauriConfig.plugins ??= {};
tauriConfig.plugins.updater ??= {};
tauriConfig.plugins.updater.endpoints = [
  `https://github.com/${repo}/releases/latest/download/latest.json`,
];
tauriConfig.plugins.updater.pubkey = tauriPubkey;
fs.writeFileSync(tauriConfigPath, `${JSON.stringify(tauriConfig, null, 2)}\n`);

const infoPlist = fs.readFileSync(infoPlistPath, "utf8")
  .replace(
    /<key>SUFeedURL<\/key>\s*<string>.*?<\/string>/s,
    `<key>SUFeedURL</key>\n  <string>https://github.com/${repo}/releases/latest/download/appcast.xml</string>`,
  )
  .replace(
    /<key>SUPublicEDKey<\/key>\s*<string>.*?<\/string>/s,
    `<key>SUPublicEDKey</key>\n  <string>${sparklePubkey}</string>`,
  );
fs.writeFileSync(infoPlistPath, infoPlist);

console.log(`[prepare-updater-config] repo=${repo}`);
