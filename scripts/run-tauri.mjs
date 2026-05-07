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
  stdio: "inherit",
  windowsHide: true,
});

if (typeof result.status === "number") {
  process.exit(result.status);
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
