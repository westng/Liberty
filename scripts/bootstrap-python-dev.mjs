import { existsSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const environmentRoot = path.join(repositoryRoot, ".venv-dev");
const environmentPython = path.join(
  environmentRoot,
  process.platform === "win32" ? "Scripts/python.exe" : "bin/python",
);
const lockPath = path.join(repositoryRoot, "python/funasr-runner/requirements-dev-lock.txt");

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: repositoryRoot, stdio: "inherit", ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function probe(command, prefixArgs = []) {
  const result = spawnSync(
    command,
    [...prefixArgs, "-c", "import sys; print('.'.join(map(str, sys.version_info[:3])))"],
    { cwd: repositoryRoot, encoding: "utf8" },
  );
  if (result.status !== 0) return null;
  const version = result.stdout.trim().split(".").map(Number);
  return version[0] === 3 && version[1] >= 10 ? { command, prefixArgs, version } : null;
}

if (!existsSync(lockPath)) {
  console.error(`Missing hash-locked development requirements: ${lockPath}`);
  process.exit(1);
}

if (!existsSync(environmentPython)) {
  const configured = process.env.LIBERTY_DEV_PYTHON?.trim();
  const candidates = [
    ...(configured ? [[configured, []]] : []),
    ["python3", []],
    ["python", []],
    ...(process.platform === "win32" ? [["py", ["-3"]]] : []),
  ];
  const selected = candidates.map(([command, args]) => probe(command, args)).find(Boolean);
  if (!selected) {
    console.error("Python 3.10 or newer is required. Set LIBERTY_DEV_PYTHON if it is not on PATH.");
    process.exit(1);
  }
  console.log(`Creating .venv-dev with Python ${selected.version.join(".")}.`);
  run(selected.command, [...selected.prefixArgs, "-m", "venv", environmentRoot]);
}

run(environmentPython, ["-m", "pip", "install", "--require-hashes", "--requirement", lockPath]);
console.log("Python development environment is ready.");
