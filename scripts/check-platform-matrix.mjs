import { readFileSync } from "node:fs";

const expectedPlatforms = new Map([
  ["darwin-aarch64", "aarch64-apple-darwin"],
  ["darwin-x64", "x86_64-apple-darwin"],
  ["windows-x64", "x86_64-pc-windows-msvc"],
  ["windows-x86", "i686-pc-windows-msvc"],
]);

const manifest = JSON.parse(
  readFileSync("apps/desktop/src-tauri/resources/runtime-manifest.json", "utf8"),
);
const workflow = readFileSync(".github/workflows/build-desktop.yml", "utf8");

const manifestPlatformIds = new Set(
  (manifest.platforms ?? []).map((platform) => platform.platformId),
);

const errors = [];

for (const [platformId, rustTarget] of expectedPlatforms) {
  if (!manifestPlatformIds.has(platformId)) {
    errors.push(`runtime manifest is missing ${platformId}`);
  }

  if (!workflow.includes(`platform_id: ${platformId}`)) {
    errors.push(`GitHub Actions matrix is missing platform_id ${platformId}`);
  }

  if (!workflow.includes(`rust_target: ${rustTarget}`)) {
    errors.push(`GitHub Actions matrix is missing rust target ${rustTarget}`);
  }
}

for (const platformId of manifestPlatformIds) {
  if (!expectedPlatforms.has(platformId)) {
    errors.push(`runtime manifest contains unknown platform ${platformId}`);
  }
}

const windowsX86 = (manifest.platforms ?? []).find(
  (platform) => platform.platformId === "windows-x86",
);
if (windowsX86?.asrBackend !== "sherpa-onnx") {
  errors.push("windows-x86 must use the separately validated sherpa-onnx backend");
}

if (errors.length > 0) {
  console.error(errors.map((error) => `- ${error}`).join("\n"));
  process.exit(1);
}

console.log("Platform matrix is consistent.");
