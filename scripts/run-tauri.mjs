import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const tauriBin = resolveTauriBin();

const result = spawnSync(tauriBin, process.argv.slice(2), {
  env: {
    ...process.env,
    CARGO_HOME: path.join(process.cwd(), "src-tauri", ".cargo-home"),
  },
  shell: process.platform === "win32",
  stdio: "inherit",
  windowsHide: true,
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
  const candidate = path.join(process.cwd(), "node_modules", ".bin", binName);

  if (existsSync(candidate)) {
    return candidate;
  }

  return "tauri";
}
