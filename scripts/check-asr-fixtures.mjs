import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { readJson, validateBenchmarkManifest } from "./lib/asr-validation.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = resolveOption("--manifest", "benchmarks/asr/manifest.local.json");
const thresholds = await readJson(path.join(repositoryRoot, "benchmarks/asr/thresholds.json"));

try {
  const manifest = await readJson(manifestPath);
  const errors = await validateBenchmarkManifest(manifest, manifestPath, thresholds);
  if (errors.length > 0) fail(errors);
  console.log(`ASR fixture manifest passed: ${manifest.samples.length} samples, six required scenarios.`);
} catch (error) {
  if (error?.code === "ENOENT") {
    fail([`local benchmark manifest is missing: ${path.relative(repositoryRoot, manifestPath)}`]);
  }
  throw error;
}

function resolveOption(name, fallback) {
  const index = process.argv.indexOf(name);
  const value = index >= 0 ? process.argv[index + 1] : fallback;
  if (!value || value.startsWith("--")) fail([`${name} requires a path`]);
  return path.resolve(process.cwd(), value);
}

function fail(errors) {
  console.error(errors.map((error) => `- ${error}`).join("\n"));
  process.exit(1);
}
