import { readFileSync } from "node:fs";

const buildMatrix = [
  {
    name: "macOS Apple Silicon",
    os: "macos-latest",
    platform_id: "darwin-aarch64",
    rust_target: "aarch64-apple-darwin",
    asset_kinds: "dmg",
    artifact_name: "liberty-darwin-aarch64",
    architecture: "arm64",
  },
  {
    name: "macOS Intel",
    os: "macos-15-intel",
    platform_id: "darwin-x64",
    rust_target: "x86_64-apple-darwin",
    asset_kinds: "dmg",
    artifact_name: "liberty-darwin-x64",
    architecture: "x86_64",
  },
  {
    name: "Windows x64",
    os: "windows-2022",
    platform_id: "windows-x64",
    rust_target: "x86_64-pc-windows-msvc",
    asset_kinds: "msi,nsis",
    artifact_name: "liberty-windows-x64",
    architecture: "x86_64",
  },
  {
    name: "Windows x86",
    os: "windows-2022",
    platform_id: "windows-x86",
    rust_target: "i686-pc-windows-msvc",
    asset_kinds: "msi,nsis",
    artifact_name: "liberty-windows-x86",
    architecture: "x86",
  },
];

if (process.argv.includes("--print-matrix")) {
  process.stdout.write(JSON.stringify({ include: buildMatrix }));
  process.exit(0);
}

const manifest = JSON.parse(
  readFileSync("apps/desktop/src-tauri/resources/runtime-manifest.json", "utf8"),
);
const workflow = readFileSync(".github/workflows/build-desktop.yml", "utf8");
const errors = [];

assertUnique(buildMatrix, "platform_id", errors);
assertUnique(buildMatrix, "rust_target", errors);
assertUnique(buildMatrix, "artifact_name", errors);

const expectedByPlatform = new Map(buildMatrix.map((entry) => [entry.platform_id, entry]));
const manifestPlatforms = manifest.platforms ?? [];
const manifestPlatformIds = new Set(manifestPlatforms.map((platform) => platform.platformId));

if (manifestPlatformIds.size !== manifestPlatforms.length) {
  errors.push("runtime manifest contains duplicate platform IDs");
}

for (const entry of buildMatrix) {
  if (!manifestPlatformIds.has(entry.platform_id)) {
    errors.push(`runtime manifest is missing ${entry.platform_id}`);
  }
  if (!entry.artifact_name.endsWith(entry.platform_id)) {
    errors.push(`${entry.platform_id} artifact name is not bound to its platform ID`);
  }
  const kinds = entry.asset_kinds.split(",").filter(Boolean);
  const expectedKinds = entry.platform_id.startsWith("darwin-")
    ? ["dmg"]
    : ["msi", "nsis"];
  if (!sameValues(kinds, expectedKinds)) {
    errors.push(`${entry.platform_id} has invalid asset kinds: ${entry.asset_kinds}`);
  }
}

for (const platformId of manifestPlatformIds) {
  if (!expectedByPlatform.has(platformId)) {
    errors.push(`runtime manifest contains unknown platform ${platformId}`);
  }
}

const windowsX86 = manifestPlatforms.find(
  (platform) => platform.platformId === "windows-x86",
);
if (!windowsX86?.unsupportedReason && windowsX86?.asrBackend !== "sherpa-onnx") {
  errors.push("windows-x86 must use the separately validated sherpa-onnx backend when it is enabled");
}

if (!workflow.includes("matrix: ${{ fromJSON(needs.prepare.outputs.build_matrix) }}")) {
  errors.push("desktop workflow must consume the validated structured build matrix output");
}
if (!workflow.includes("node scripts/check-platform-matrix.mjs --print-matrix")) {
  errors.push("desktop workflow must generate its matrix from check-platform-matrix.mjs");
}
if (/^\s+platform_id:\s/m.test(workflow) || /^\s+rust_target:\s/m.test(workflow)) {
  errors.push("desktop workflow must not duplicate platform/target pairs outside the structured matrix");
}

if (errors.length > 0) {
  console.error(errors.map((error) => `- ${error}`).join("\n"));
  process.exit(1);
}

console.log("Platform matrix is consistent.");

function assertUnique(entries, key, errors) {
  const values = entries.map((entry) => entry[key]);
  if (new Set(values).size !== values.length) {
    errors.push(`build matrix contains duplicate ${key} values`);
  }
}

function sameValues(left, right) {
  return left.length === right.length && [...left].sort().every((value, index) => value === [...right].sort()[index]);
}
