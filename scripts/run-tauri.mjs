import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import process from "node:process";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const desktopRoot = path.join(repoRoot, "apps", "desktop");
const tauriBin = resolveTauriBin();
const env = {
  ...process.env,
  SPARKLE_FRAMEWORK_PATH:
    process.env.SPARKLE_FRAMEWORK_PATH ?? path.join(desktopRoot, "src-tauri", "vendor"),
};

if (process.env.LIBERTY_USE_LOCAL_CARGO_HOME === "1") {
  env.CARGO_HOME = path.join(desktopRoot, "src-tauri", ".cargo-home");
}

const result = spawnSync(tauriBin, process.argv.slice(2), {
  env,
  shell: process.platform === "win32",
  stdio: "inherit",
  windowsHide: true,
  cwd: desktopRoot,
});

if (typeof result.status === "number") {
  process.exit(result.status);
}

if (result.error) {
  console.error(`[run-tauri] failed to start ${tauriBin}: ${result.error.message}`);
}

if (result.signal) {
  console.error(`[run-tauri] tauri process exited from signal ${result.signal}`);
}

process.exit(1);

function resolveTauriBin() {
  const binName = process.platform === "win32" ? "tauri.cmd" : "tauri";
  const candidates = [
    path.join(desktopRoot, "node_modules", ".bin", binName),
    path.join(repoRoot, "node_modules", ".bin", binName),
  ];

  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }

  return "tauri";
}
