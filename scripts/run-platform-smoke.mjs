import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { detectPlatformId, readJson, validateSmokeInput } from "./lib/asr-validation.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const inputPath = resolveOption("--input", "benchmarks/asr/smoke.local.json");
const outputPath = resolveOption(
  "--output",
  `artifacts/asr-validation/smoke-${new Date().toISOString().replaceAll(/[:.]/g, "-")}.json`,
);
const allowDirty = process.argv.includes("--allow-dirty");
const platforms = await readJson(path.join(repositoryRoot, "benchmarks/asr/platforms.json"));
let input;
try {
  input = await readJson(inputPath);
} catch (error) {
  if (error?.code === "ENOENT") fail([`local smoke input is missing: ${path.relative(repositoryRoot, inputPath)}`]);
  throw error;
}
const errors = validateSmokeInput(input);
if (errors.length > 0) fail(errors);
const platformId = detectPlatformId();
const platform = platforms.platforms.find((entry) => entry.platformId === platformId);
if (!platform?.localAsrRequired) fail([`current platform ${platformId ?? "unknown"} is not a required local ASR smoke target`]);
const commit = git(["rev-parse", "HEAD"]);
const dirty = git(["status", "--porcelain"]).length > 0;
if (dirty && !allowDirty) {
  fail(["worktree is dirty; commit the exact smoke source or use --allow-dirty for non-acceptable exploratory evidence"]);
}
const evidence = {
  schemaVersion: 1,
  evidenceType: "platform-smoke",
  generatedAt: new Date().toISOString(),
  operator: input.operator,
  commit,
  dirty,
  platformId,
  runtimeVersion: input.runtimeVersion,
  modelSetVersion: input.modelSetVersion,
  device: {
    id: input.deviceId,
    operatingSystem: `${os.type()} ${os.release()}`,
    architecture: os.arch(),
    cpuModel: os.cpus()[0]?.model ?? "unknown",
    logicalCores: os.cpus().length,
    totalMemoryBytes: os.totalmem(),
  },
  checks: input.checks,
  notes: input.notes ?? "",
};
const failed = evidence.checks.filter((check) => check.status !== "passed");
evidence.status = failed.length === 0 && !dirty ? "passed" : "failed";
evidence.attestationDigest = `sha256:${createHash("sha256").update(JSON.stringify(evidence)).digest("hex")}`;
await mkdir(path.dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(evidence, null, 2)}\n`, "utf8");
console.log(`Wrote platform smoke evidence: ${path.relative(repositoryRoot, outputPath)}`);
if (dirty) errors.push("smoke evidence was generated from a dirty worktree");
for (const check of failed) errors.push(`${check.id} is ${check.status}: ${check.evidence}`);
if (errors.length > 0) fail(errors);

function git(args) {
  const result = spawnSync("git", args, { cwd: repositoryRoot, encoding: "utf8" });
  if (result.status !== 0) fail([`git ${args[0]} failed; smoke evidence requires a repository commit`]);
  return result.stdout.trim();
}

function resolveOption(name, fallback) {
  const index = process.argv.indexOf(name);
  const value = index >= 0 ? process.argv[index + 1] : fallback;
  if (!value || value.startsWith("--")) fail([`${name} requires a path`]);
  return path.resolve(process.cwd(), value);
}

function fail(values) {
  console.error(values.map((value) => `- ${value}`).join("\n"));
  process.exit(1);
}
