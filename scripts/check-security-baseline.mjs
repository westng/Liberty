import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const tauriConfig = JSON.parse(
  readFileSync("apps/desktop/src-tauri/tauri.conf.json", "utf8"),
);
const capabilitiesDir = "apps/desktop/src-tauri/capabilities";
const capabilities = readdirSync(capabilitiesDir)
  .filter((fileName) => fileName.endsWith(".json"))
  .map((fileName) => ({
    fileName,
    data: JSON.parse(readFileSync(join(capabilitiesDir, fileName), "utf8")),
  }));

const errors = [];
const csp = tauriConfig?.app?.security?.csp;

if (typeof csp !== "string" || csp.trim().length === 0) {
  errors.push("Tauri CSP must be a non-empty string.");
}

if (csp === null) {
  errors.push("Tauri CSP must not be null.");
}

if (capabilities.length < 2) {
  errors.push("Tauri capabilities must be split by window role, not kept as one broad capability.");
}

const capabilitiesWithFsWrite = capabilities.filter(({ data }) =>
  (data.permissions ?? []).includes("fs:allow-write-text-file"),
);

if (capabilitiesWithFsWrite.length === 0) {
  errors.push("At least one Tauri capability must define controlled text export permissions.");
}

for (const { fileName, data } of capabilities) {
  const permissions = data.permissions ?? [];
  const hasFsPermission = permissions.some(
    (permission) => typeof permission === "string" && permission.startsWith("fs:"),
  );
  const hasFsScope = permissions.some(
    (permission) =>
      typeof permission === "object" &&
      permission !== null &&
      permission.identifier === "fs:scope" &&
      Array.isArray(permission.allow) &&
      permission.allow.length > 0,
  );

  if (hasFsPermission && !hasFsScope) {
    errors.push(`${fileName}: file-system permissions must define an explicit fs:scope.`);
  }

  if (permissions.includes("fs:allow-write")) {
    errors.push(`${fileName}: broad fs:allow-write is not allowed.`);
  }

  if (fileName !== "main.json" && permissions.includes("core:webview:allow-create-webview-window")) {
    errors.push(`${fileName}: child-window creation must stay limited to the main window.`);
  }

  if (fileName !== "main.json" && hasFsPermission) {
    errors.push(`${fileName}: file-system permissions must stay limited to the main window.`);
  }
}

if (errors.length > 0) {
  console.error(errors.map((error) => `- ${error}`).join("\n"));
  process.exit(1);
}

console.log("Security baseline is configured.");
