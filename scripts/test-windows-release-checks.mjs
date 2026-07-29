import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runPowerShellWithAsset } from "./lib/powershell.mjs";

const root = mkdtempSync(join(tmpdir(), "liberty-windows-release-checks-"));
const asset = join(root, "asset & 'quoted path'.msi");
const expected = "release-check-path-ok";

try {
  writeFileSync(asset, expected);
  const actual = runPowerShellWithAsset(
    "[System.IO.File]::ReadAllText($env:LIBERTY_RELEASE_ASSET_PATH)",
    asset,
  );
  if (actual !== expected) {
    throw new Error(`PowerShell asset path transport returned ${JSON.stringify(actual)}.`);
  }
  console.log("Windows release PowerShell transport self-test passed.");
} finally {
  rmSync(root, { recursive: true, force: true });
}
