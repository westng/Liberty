import { access, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  assertNodeLockHasIntegrity,
  assertPythonLockHasHashes,
  readPolicy,
} from "./lib/dependency-governance.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const policy = await readPolicy(path.join(repositoryRoot, "config/dependency-governance.json"));
const mode = process.argv[2] ?? "security";

assertNodeLockHasIntegrity(await readFile(path.join(repositoryRoot, "pnpm-lock.yaml"), "utf8"));
if (policy.requireHashesForPythonLocks) {
  const locks = [
    "python/funasr-runner/requirements-dev-lock.txt",
    "python/funasr-runner/requirements-lock-darwin-aarch64.txt",
    "python/funasr-runner/requirements-lock-darwin-x64.txt",
    "python/funasr-runner/requirements-lock-windows-x64.txt",
  ];
  for (const lock of locks) {
    assertPythonLockHasHashes(await readFile(path.join(repositoryRoot, lock), "utf8"), lock);
  }
}

if (mode === "locks") {
  console.log("Dependency lock integrity policy passed.");
  process.exit(0);
}

const requiredCommands = mode === "licenses" ? ["cargo-deny"] : ["cargo-deny", "osv-scanner"];
for (const command of requiredCommands) {
  const lookup = spawnSync("sh", ["-c", `command -v ${command}`], { encoding: "utf8" });
  if (lookup.status !== 0) {
    console.error(`${command} is required. Install the pinned CI version before running ${mode}:check.`);
    process.exit(2);
  }
}

assertToolVersion("cargo-deny", policy.tools.cargoDeny);
if (mode === "security") assertToolVersion("osv-scanner", policy.tools.osvScanner);

const cargoDenyChecks = mode === "licenses" ? ["licenses"] : ["advisories", "bans", "sources"];
const cargoDeny = spawnSync("cargo-deny", ["check", ...cargoDenyChecks], {
  cwd: repositoryRoot,
  stdio: "inherit",
});
if (cargoDeny.status !== 0) {
  process.exit(cargoDeny.status ?? 1);
}

if (mode === "security") {
  const lockFiles = [
    "pnpm-lock.yaml",
    "Cargo.lock",
    "python/funasr-runner/requirements-lock-darwin-aarch64.txt",
    "python/funasr-runner/requirements-lock-darwin-x64.txt",
    "python/funasr-runner/requirements-lock-windows-x64.txt",
  ];
  for (const lockFile of lockFiles) {
    await access(path.join(repositoryRoot, lockFile));
    const scan = spawnSync("osv-scanner", ["scan", "-L", lockFile], {
      cwd: repositoryRoot,
      stdio: "inherit",
    });
    if (scan.status !== 0) {
      process.exit(scan.status ?? 1);
    }
  }
}

console.log(`${mode} dependency governance passed.`);

function assertToolVersion(command, expectedVersion) {
  const result = spawnSync(command, ["--version"], { encoding: "utf8" });
  const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  if (result.status !== 0 || !new RegExp(`(^|[^0-9])${escapeRegex(expectedVersion)}([^0-9]|$)`).test(output)) {
    console.error(`${command} must be pinned to ${expectedVersion}; received ${output.trim() || "no version"}.`);
    process.exit(2);
  }
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
