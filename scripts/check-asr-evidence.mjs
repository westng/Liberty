import { createHash } from "node:crypto";
import { readdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import {
  evaluateBenchmarkPair,
  readJson,
  REQUIRED_SMOKE_CHECKS,
} from "./lib/asr-validation.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const evidenceDirectory = resolveOption("--evidence-dir", "artifacts/asr-validation/accepted");
const thresholds = await readJson(path.join(repositoryRoot, "benchmarks/asr/thresholds.json"));
const platformRegistry = await readJson(path.join(repositoryRoot, "benchmarks/asr/platforms.json"));
const requiredPlatforms = platformRegistry.platforms.filter((platform) => platform.localAsrRequired);
let fileNames;
try {
  fileNames = (await readdir(evidenceDirectory, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && entry.name.endsWith(".json"))
    .map((entry) => entry.name);
} catch (error) {
  if (error?.code === "ENOENT") fail([`accepted ASR evidence directory is missing: ${path.relative(repositoryRoot, evidenceDirectory)}`]);
  throw error;
}
const documents = [];
for (const fileName of fileNames) {
  try {
    documents.push(await readJson(path.join(evidenceDirectory, fileName)));
  } catch (error) {
    fail([`${fileName} is not valid JSON: ${error.message}`]);
  }
}
const errors = [];
const accepted = [];
for (const platform of requiredPlatforms) {
  const smokes = documents.filter((document) => document.evidenceType === "platform-smoke" && document.platformId === platform.platformId);
  const benchmarks = documents.filter((document) => document.evidenceType === "asr-benchmark" && document.platformId === platform.platformId);
  if (smokes.length !== 1) errors.push(`${platform.platformId} requires exactly one platform smoke evidence file`);
  if (benchmarks.length !== 1) errors.push(`${platform.platformId} requires exactly one ASR benchmark evidence file`);
  if (smokes.length !== 1 || benchmarks.length !== 1) continue;
  const smoke = smokes[0];
  const benchmark = benchmarks[0];
  validateCommonEvidence(smoke, `${platform.platformId} smoke`, errors);
  validateCommonEvidence(benchmark, `${platform.platformId} benchmark`, errors);
  if (smoke.commit !== benchmark.commit) errors.push(`${platform.platformId} smoke and benchmark commits differ`);
  const candidate = benchmark.engines?.find((engine) => engine.role === "candidate");
  if (!candidate
      || smoke.runtimeVersion !== candidate.runtimeVersion
      || smoke.modelSetVersion !== candidate.modelSetVersion) {
    errors.push(`${platform.platformId} smoke runtime/model version differs from the benchmark candidate`);
  }
  if (smoke.status !== "passed") errors.push(`${platform.platformId} smoke status is not passed`);
  for (const checkId of REQUIRED_SMOKE_CHECKS) {
    if (smoke.checks?.find((check) => check.id === checkId)?.status !== "passed") {
      errors.push(`${platform.platformId} smoke check is not passed: ${checkId}`);
    }
  }
  if (benchmark.status !== "passed" || benchmark.approvedForDefaultSwitch !== true) {
    errors.push(`${platform.platformId} benchmark is not approved by its local gate`);
  }
  for (const violation of evaluateBenchmarkPair(benchmark, thresholds)) {
    errors.push(`${platform.platformId}: ${violation}`);
  }
  const memoryGiB = benchmark.device?.totalMemoryBytes / 1024 ** 3;
  if (!Number.isFinite(memoryGiB)
      || memoryGiB < platform.minimumMemoryGiB - 0.5
      || memoryGiB > platform.performanceEvidenceMemoryTierGiB + 0.5) {
    errors.push(`${platform.platformId} benchmark is not from the required ${platform.performanceEvidenceMemoryTierGiB} GiB performance tier`);
  }
  accepted.push({ platformId: platform.platformId, smoke, benchmark });
}
const commits = new Set(accepted.flatMap(({ smoke, benchmark }) => [smoke.commit, benchmark.commit]));
if (commits.size > 1) errors.push("accepted cross-platform evidence does not share one commit");
for (const role of ["baseline", "candidate"]) {
  const engineIdentities = new Set(accepted.map(({ benchmark }) => {
    const engine = benchmark.engines?.find((entry) => entry.role === role);
    return engine ? `${engine.id}:${engine.backend}:${engine.runtimeVersion}:${engine.modelSetVersion}` : "missing";
  }));
  if (engineIdentities.size > 1 || engineIdentities.has("missing")) {
    errors.push(`${role} engine identity differs across platforms`);
  }
}
const corpusIdentities = new Set(accepted.map(({ benchmark }) => (
  `${benchmark.corpusVersion}:${benchmark.annotationVersion}`
)));
if (corpusIdentities.size > 1) errors.push("benchmark corpus or annotation version differs across platforms");
const sampleIdentities = new Set(accepted.map(({ benchmark }) => JSON.stringify(
  (benchmark.engines?.find((engine) => engine.role === "baseline")?.samples ?? [])
    .map((sample) => ({
      sampleId: sample.sampleId,
      mediaSha256: sample.mediaSha256,
      annotationSha256: sample.annotationSha256,
      annotationVersion: sample.annotationVersion,
      scenarios: sample.scenarios,
    }))
    .sort((left, right) => left.sampleId.localeCompare(right.sampleId)),
)));
if (sampleIdentities.size > 1 || [...sampleIdentities][0] === "[]") {
  errors.push("benchmark sample hashes differ across platforms or are missing");
}
if (errors.length > 0) fail(errors);
console.log(`ASR evidence passed for ${requiredPlatforms.length} physical platforms at commit ${[...commits][0]}.`);

function validateCommonEvidence(evidence, label, errors) {
  if (evidence.schemaVersion !== 1) errors.push(`${label} schemaVersion must be 1`);
  if (!/^[a-f0-9]{40}$/.test(evidence.commit ?? "")) errors.push(`${label} commit is invalid`);
  if (evidence.dirty !== false) errors.push(`${label} was generated from a dirty worktree`);
  const statedDigest = evidence.attestationDigest;
  const unsigned = { ...evidence };
  delete unsigned.attestationDigest;
  const actualDigest = `sha256:${createHash("sha256").update(JSON.stringify(unsigned)).digest("hex")}`;
  if (statedDigest !== actualDigest) errors.push(`${label} attestation digest does not match its contents`);
  if (!Number.isFinite(Date.parse(evidence.generatedAt))) errors.push(`${label} generatedAt is invalid`);
  if (!evidence.device?.id) errors.push(`${label} device ID is missing`);
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
