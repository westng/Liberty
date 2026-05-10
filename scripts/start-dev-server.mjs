import net from "node:net";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import process from "node:process";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const desktopRoot = path.join(repoRoot, "apps", "desktop");
const host = process.env.TAURI_DEV_HOST || "127.0.0.1";
const port = 5173;

if (await isPortOpen(host, port)) {
  console.log(`[start-dev-server] reuse existing Vite server at http://${host}:${port}`);
  process.exit(0);
}

const child = spawn(resolveViteBin(), ["--host", host], {
  stdio: "inherit",
  shell: process.platform === "win32",
  windowsHide: true,
  env: process.env,
  cwd: desktopRoot,
});

child.on("exit", (code) => {
  process.exit(code ?? 0);
});

child.on("error", (error) => {
  console.error(`[start-dev-server] failed to start dev server: ${error}`);
  process.exit(1);
});

function resolveViteBin() {
  const binName = process.platform === "win32" ? "vite.cmd" : "vite";
  const candidates = [
    path.join(desktopRoot, "node_modules", ".bin", binName),
    path.join(repoRoot, "node_modules", ".bin", binName),
  ];

  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }

  return binName;
}

function isPortOpen(host, port) {
  return new Promise((resolve) => {
    const socket = net.createConnection({ host, port });

    socket.once("connect", () => {
      socket.destroy();
      resolve(true);
    });

    socket.once("error", () => {
      socket.destroy();
      resolve(false);
    });

    socket.setTimeout(800, () => {
      socket.destroy();
      resolve(false);
    });
  });
}
