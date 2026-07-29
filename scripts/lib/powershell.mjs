import { execFileSync } from "node:child_process";
import { Buffer } from "node:buffer";
import { resolve } from "node:path";
import process from "node:process";

const assetPathVariable = "LIBERTY_RELEASE_ASSET_PATH";

export function runPowerShellWithAsset(script, assetPath) {
  const encodedCommand = Buffer.from(script, "utf16le").toString("base64");
  return execFileSync(
    "powershell.exe",
    ["-NoLogo", "-NoProfile", "-NonInteractive", "-EncodedCommand", encodedCommand],
    {
      encoding: "utf8",
      env: {
        ...process.env,
        [assetPathVariable]: resolve(assetPath),
      },
      windowsHide: true,
    },
  ).trim();
}
