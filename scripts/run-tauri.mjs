import { execFileSync, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { arch, platform } from "node:os";
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

if (process.argv[2] === "dev") {
  stopStaleDesktopProcesses();
}

if (!process.env.RUSTUP_TOOLCHAIN && platform() === "darwin" && arch() === "arm64") {
  env.RUSTUP_TOOLCHAIN = "stable-aarch64-apple-darwin";
}

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

function stopStaleDesktopProcesses() {
  if (process.platform === "win32") {
    return;
  }

  const selfPid = String(process.pid);
  let output = "";
  try {
    output = execFileSync("ps", ["-axo", "pid=,ppid=,command="], { encoding: "utf8" });
  } catch {
    return;
  }

  const stalePids = output
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const match = line.match(/^(\d+)\s+(\d+)\s+(.+)$/);
      if (!match) {
        return null;
      }
      return { pid: match[1], ppid: match[2], command: match[3] };
    })
    .filter((entry) => {
      if (!entry || entry.pid === selfPid || entry.ppid === selfPid) {
        return false;
      }
      return isStaleLibertyDevCommand(entry.command);
    })
    .map((entry) => entry.pid);

  if (stalePids.length === 0) {
    return;
  }

  try {
    execFileSync("kill", stalePids, { stdio: "ignore" });
    console.warn(`[run-tauri] stopped stale Liberty dev process(es): ${stalePids.join(", ")}`);
  } catch {
    // Best-effort cleanup; Tauri will still start normally if this fails.
  }
}

function isStaleLibertyDevCommand(command) {
  const desktopTauriCli = path.join(repoRoot, "node_modules", ".bin", "..", "@tauri-apps", "cli", "tauri.js");
  const desktopVite = path.join(repoRoot, "node_modules", ".bin", "..", "vite", "bin", "vite.js");
  const desktopBinary = path.join(repoRoot, "target", "debug", "liberty");

  return command === desktopBinary
    || command.startsWith(`${desktopBinary} `)
    || command === "node scripts/run-tauri.mjs dev"
    || command.includes(`${desktopTauriCli} dev`)
    || command.includes(`${desktopVite} --host 127.0.0.1`)
    || (command.includes("cargo run --no-default-features --color always --")
      && command.includes(repoRoot));
}
