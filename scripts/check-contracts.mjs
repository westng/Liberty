import { mkdtemp, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const expectedDirectory = path.join(repositoryRoot, "packages/shared-types/src/generated");
const temporaryDirectory = await mkdtemp(path.join(os.tmpdir(), "liberty-contracts-"));
const outputs = ["runner-v2.ts", "meeting-job-v1.ts", "settings-v1.ts", "ai-v1.ts", "runtime-v1.ts"];

try {
  const result = spawnSync(
    process.execPath,
    [path.join(repositoryRoot, "scripts/generate-contracts.mjs"), "--output-dir", temporaryDirectory],
    { cwd: repositoryRoot, encoding: "utf8" },
  );
  if (result.status !== 0) {
    process.stderr.write(result.stderr || result.stdout);
    process.exit(result.status ?? 1);
  }

  for (const output of outputs) {
    const [expected, actual] = await Promise.all([
      readFile(path.join(expectedDirectory, output), "utf8"),
      readFile(path.join(temporaryDirectory, output), "utf8"),
    ]);
    if (expected !== actual) {
      console.error(`${output} is stale. Run \`pnpm contracts:generate\`.`);
      process.exit(1);
    }
  }
  console.log(`All ${outputs.length} contract outputs are current.`);
} finally {
  await rm(temporaryDirectory, { recursive: true, force: true });
}
