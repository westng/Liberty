import { readFileSync } from "node:fs";

const requirementsPath = "python/funasr-runner/requirements.txt";
const lockFiles = [
  "python/funasr-runner/requirements-lock-darwin-aarch64.txt",
  "python/funasr-runner/requirements-lock-darwin-x64.txt",
  "python/funasr-runner/requirements-lock-windows-x64.txt",
];
const runtimeSource = readFileSync("apps/desktop/src-tauri/src/local_runtime.rs", "utf8");
const errors = [];
const directPins = parseDirectPins(readFileSync(requirementsPath, "utf8"), errors);

for (const lockFile of lockFiles) {
  const locked = parseHashLock(lockFile, readFileSync(lockFile, "utf8"), errors);
  for (const [name, version] of directPins) {
    if (locked.get(name) !== version) {
      errors.push(`${lockFile} must contain ${name}==${version}`);
    }
  }
}

for (const lockFile of lockFiles) {
  const fileName = lockFile.split("/").at(-1);
  if (!runtimeSource.includes(`"${fileName}"`)) {
    errors.push(`local_runtime.rs does not select ${fileName}`);
  }
}
for (const requiredFlag of ["--require-hashes", "--no-build-isolation"]) {
  if (!runtimeSource.includes(`.arg("${requiredFlag}")`)) {
    errors.push(`Python dependency installation must use ${requiredFlag}`);
  }
}
if (/MODELSCOPE_MODEL_REVISION|"master"/.test(runtimeSource)) {
  errors.push("ModelScope revisions must be immutable commit hashes, not movable branches");
}
const pinnedModelCommits = new Set(
  [...runtimeSource.matchAll(/"([a-f0-9]{40})"/g)].map((match) => match[1]),
);
if (pinnedModelCommits.size < 4) {
  errors.push("Every bundled ModelScope model must pin an immutable 40-character commit hash");
}

if (errors.length > 0) {
  console.error(errors.map((error) => `- ${error}`).join("\n"));
  process.exit(1);
}

console.log(`Runtime dependencies are hash-locked for ${lockFiles.length} supported platforms.`);

function parseDirectPins(contents, errors) {
  const pins = new Map();
  for (const rawLine of contents.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) {
      continue;
    }
    const match = line.match(/^([A-Za-z0-9_.-]+)==([A-Za-z0-9_.+!-]+)$/);
    if (!match) {
      errors.push(`${requirementsPath} contains a non-exact dependency: ${line}`);
      continue;
    }
    const name = normalizePackageName(match[1]);
    if (pins.has(name)) {
      errors.push(`${requirementsPath} contains duplicate dependency ${name}`);
    }
    pins.set(name, match[2]);
  }
  if (pins.size === 0) {
    errors.push(`${requirementsPath} contains no direct dependencies`);
  }
  return pins;
}

function parseHashLock(fileName, contents, errors) {
  const packages = new Map();
  let currentPackage = null;
  let currentHasHash = false;

  const finishPackage = () => {
    if (currentPackage && !currentHasHash) {
      errors.push(`${fileName} package ${currentPackage} has no SHA-256 hash`);
    }
  };

  for (const rawLine of contents.split(/\r?\n/)) {
    if (!rawLine.trim() || rawLine.trimStart().startsWith("#")) {
      continue;
    }
    if (/^\s/.test(rawLine)) {
      if (!currentPackage || !/^\s+--hash=sha256:[a-f0-9]{64}(?:\s+\\)?$/.test(rawLine)) {
        errors.push(`${fileName} contains an invalid hash continuation: ${rawLine.trim()}`);
      } else {
        currentHasHash = true;
      }
      continue;
    }

    finishPackage();
    const match = rawLine.match(/^([A-Za-z0-9_.-]+)==([^\s\\]+)\s+\\$/);
    if (!match) {
      errors.push(`${fileName} contains a non-exact or unhashed entry: ${rawLine}`);
      currentPackage = null;
      currentHasHash = false;
      continue;
    }
    const name = normalizePackageName(match[1]);
    if (packages.has(name)) {
      errors.push(`${fileName} contains duplicate package ${name}`);
    }
    packages.set(name, match[2]);
    currentPackage = name;
    currentHasHash = false;
  }
  finishPackage();
  if (packages.size === 0) {
    errors.push(`${fileName} contains no locked packages`);
  }
  return packages;
}

function normalizePackageName(value) {
  return value.toLowerCase().replaceAll(/[_.]+/g, "-");
}
