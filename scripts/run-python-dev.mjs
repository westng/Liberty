import { existsSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const pythonPath = path.join(
  repositoryRoot,
  ".venv-dev",
  process.platform === "win32" ? "Scripts/python.exe" : "bin/python",
);
const command = process.argv[2];

if (!existsSync(pythonPath)) {
  console.error("Missing .venv-dev. Run `pnpm python:bootstrap` first.");
  process.exit(1);
}

const commands = {
  lint: ["-m", "ruff", "check", "python/funasr-runner"],
  test: ["-m", "pytest", "python/funasr-runner/tests"],
};
const args = commands[command];
if (!args) {
  console.error("Usage: node scripts/run-python-dev.mjs <lint|test>");
  process.exit(2);
}

const result = spawnSync(pythonPath, args, { cwd: repositoryRoot, stdio: "inherit" });
if (result.error) throw result.error;
process.exit(result.status ?? 1);
