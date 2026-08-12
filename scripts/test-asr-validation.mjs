import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  characterErrorRate,
  diarizationErrorRate,
  evaluateBenchmarkPair,
  REQUIRED_SMOKE_CHECKS,
  sha256File,
  validateBenchmarkManifest,
  validateSmokeInput,
  wordErrorRate,
} from "./lib/asr-validation.mjs";

assert.equal(characterErrorRate("你好世界", "你好世"), 0.25);
assert.equal(wordErrorRate("hello liberty team", "hello team"), 1 / 3);
assert.equal(diarizationErrorRate(
  [{ startMs: 0, endMs: 1000, speaker: "A" }, { startMs: 1000, endMs: 2000, speaker: "B" }],
  [{ startMs: 0, endMs: 1000, speaker: "speaker-0" }, { startMs: 1000, endMs: 2000, speaker: "speaker-1" }],
), 0);
assert.equal(diarizationErrorRate(
  [{ startMs: 0, endMs: 1000, speaker: "A" }],
  [],
), 1);

const thresholds = {
  requiredScenarios: ["short_meeting"],
  candidate: {
    maximumNewFailures: 0,
    maximumChineseCerRegression: 0.005,
    maximumDiarizationDerRegression: 0.01,
    maximumP95RealTimeFactor: 1,
    maximumPeakRssBytes: 4096,
    maximumRuntimeMetricRegressionRatio: 1.1,
    maximumInstallSizeIncreaseBytes: 500,
  },
};
const sample = {
  sampleId: "short", language: "zh-CN", speakerRequired: true, success: true,
  mediaSha256: "b".repeat(64), annotationSha256: "c".repeat(64), annotationVersion: "fixture-1",
  cer: 0.1, der: 0.1, realTimeFactor: 0.5, peakRssBytes: 1000, syntheticSpeakerLabels: [],
};
assert.deepEqual(evaluateBenchmarkPair({
  engines: [
    { role: "baseline", installSizeBytes: 1000, samples: [sample] },
    { role: "candidate", installSizeBytes: 1100, samples: [{ ...sample, cer: 0.104, der: 0.109 }] },
  ],
}, thresholds), []);
assert.match(evaluateBenchmarkPair({
  engines: [
    { role: "baseline", installSizeBytes: 1000, samples: [sample] },
    { role: "candidate", installSizeBytes: 1100, samples: [{ ...sample, cer: 0.2 }] },
  ],
}, thresholds).join("\n"), /CER regression/);

assert.match(validateSmokeInput({ schemaVersion: 1, operator: "", deviceId: "device", checks: [] }).join("\n"), /operator/);

const fixtureRoot = await mkdtemp(path.join(tmpdir(), "liberty-asr-validation-"));
const media = path.join(fixtureRoot, "sample.wav");
const annotation = path.join(fixtureRoot, "sample.json");
await writeFile(media, "fixture media");
await writeFile(annotation, JSON.stringify({ schemaVersion: 1, transcript: "脱敏文字" }));
const manifestPath = path.join(fixtureRoot, "manifest.local.json");
const manifest = {
  schemaVersion: 1,
  corpusVersion: "fixture-1",
  annotationVersion: "fixture-1",
  deviceId: "fixture-device",
  samples: [{
    id: "short", scenarios: ["short_meeting"], language: "zh-CN",
    expectedSpeakerCount: 1, speakerRequired: false,
    media: { path: "missing.wav", sha256: await sha256File(media), durationSeconds: 1 },
    annotation: { path: "sample.json", sha256: await sha256File(annotation), version: "fixture-1" },
  }],
  engines: [
    { role: "baseline", id: "base", backend: "funasr", runtimeVersion: "1", modelSetVersion: "1", command: ["python"], installRoot: "." },
    { role: "candidate", id: "next", backend: "funasr", runtimeVersion: "2", modelSetVersion: "2", command: ["python"], installRoot: "." },
  ],
};
const fixtureErrors = await validateBenchmarkManifest(manifest, manifestPath, thresholds);
assert.match(fixtureErrors.join("\n"), /media is unavailable/);

const workflowRoot = await mkdtemp(path.join(tmpdir(), "liberty-asr-workflow-"));
const workflowMedia = path.join(workflowRoot, "sample.wav");
const workflowAnnotation = path.join(workflowRoot, "annotation.json");
const fakeRunner = path.join(workflowRoot, "fake-runner.mjs");
const baselineInstall = path.join(workflowRoot, "baseline-install");
const candidateInstall = path.join(workflowRoot, "candidate-install");
await mkdir(baselineInstall);
await mkdir(candidateInstall);
await writeFile(path.join(baselineInstall, "runtime.bin"), "same-size");
await writeFile(path.join(candidateInstall, "runtime.bin"), "same-size");
await writeFile(workflowMedia, "synthetic non-sensitive media fixture");
await writeFile(workflowAnnotation, JSON.stringify({
  schemaVersion: 1,
  transcript: "脱敏基准内容",
  speakerTurns: [{ startMs: 0, endMs: 1000, speaker: "person-a" }],
}));
await writeFile(fakeRunner, `
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
const option = (name) => process.argv[process.argv.indexOf(name) + 1];
const jobDir = option("--job-dir");
await mkdir(jobDir, { recursive: true });
await writeFile(path.join(jobDir, "progress.json"), JSON.stringify({ progressPercent: 32 }));
await new Promise((resolve) => setTimeout(resolve, 400));
await writeFile(path.join(jobDir, "result.json"), JSON.stringify({
  protocolVersion: 2,
  asrBackend: "funasr",
  diarizationRequested: true,
  diarizationStatus: "completed",
  warnings: [],
  durationMinutes: 1,
  transcriptSegments: [{ id: "1", startMs: 0, endMs: 1000, text: "脱敏基准内容" }],
  speakerSegments: [{ id: "1", startMs: 0, endMs: 1000, text: "脱敏基准内容", speaker: "person-a" }],
}));
`);
const validManifest = {
  schemaVersion: 1,
  corpusVersion: "workflow-fixture-1",
  annotationVersion: "workflow-fixture-1",
  deviceId: "workflow-device",
  samples: [{
    id: "all-scenarios",
    scenarios: ["short_meeting", "long_meeting", "overlapping_speakers", "noise", "mixed_zh_en", "chinese_numbers"],
    language: "zh-CN",
    expectedSpeakerCount: 1,
    speakerRequired: true,
    media: { path: "sample.wav", sha256: await sha256File(workflowMedia), durationSeconds: 1 },
    annotation: { path: "annotation.json", sha256: await sha256File(workflowAnnotation), version: "workflow-fixture-1" },
  }],
  engines: [
    { role: "baseline", id: "baseline-funasr", backend: "funasr", runtimeVersion: "1", modelSetVersion: "1", command: [process.execPath, fakeRunner], installRoot: "baseline-install" },
    { role: "candidate", id: "candidate-funasr", backend: "funasr", runtimeVersion: "2", modelSetVersion: "2", command: [process.execPath, fakeRunner], installRoot: "candidate-install" },
  ],
};
const validManifestPath = path.join(workflowRoot, "manifest.local.json");
const benchmarkOutput = path.join(workflowRoot, "benchmark.json");
await writeFile(validManifestPath, JSON.stringify(validManifest));
const fixtureCheck = runScript("scripts/check-asr-fixtures.mjs", ["--manifest", validManifestPath]);
assert.equal(fixtureCheck.status, 0, fixtureCheck.stderr);
const benchmarkRun = runScript("scripts/run-asr-benchmark.mjs", [
  "--manifest", validManifestPath,
  "--output", benchmarkOutput,
  "--allow-dirty",
]);
assert.notEqual(benchmarkRun.status, 0, "exploratory dirty/16 GiB evidence must not pass acceptance");
const benchmarkEvidence = JSON.parse(await readFile(benchmarkOutput, "utf8"));
assert.equal(benchmarkEvidence.engines.length, 2);
assert.equal(benchmarkEvidence.engines.every((engine) => engine.samples[0].success), true);
assert.equal(benchmarkEvidence.engines.every((engine) => engine.samples[0].cer === 0), true);
assert.match(benchmarkEvidence.violations.join("\n"), /dirty worktree|performance tier/);

const acceptedEvidenceRoot = path.join(workflowRoot, "accepted");
await mkdir(acceptedEvidenceRoot);
const evidenceCommit = "a".repeat(40);
for (const platformId of ["darwin-aarch64", "darwin-x64", "windows-x64"]) {
  const smoke = signEvidence({
    schemaVersion: 1,
    evidenceType: "platform-smoke",
    generatedAt: "2026-08-13T00:00:00.000Z",
    operator: "fixture-operator",
    commit: evidenceCommit,
    dirty: false,
    platformId,
    runtimeVersion: "2",
    modelSetVersion: "2",
    device: { id: `${platformId}-device`, totalMemoryBytes: 8 * 1024 ** 3 },
    checks: REQUIRED_SMOKE_CHECKS.map((id) => ({ id, status: "passed", evidence: "fixture evidence" })),
    notes: "synthetic fixture",
    status: "passed",
  });
  const benchmark = signEvidence({
    schemaVersion: 1,
    evidenceType: "asr-benchmark",
    generatedAt: "2026-08-13T00:00:00.000Z",
    commit: evidenceCommit,
    dirty: false,
    platformId,
    device: { id: `${platformId}-device`, totalMemoryBytes: 8 * 1024 ** 3 },
    corpusVersion: "fixture-1",
    annotationVersion: "fixture-1",
    engines: [
      { role: "baseline", id: "funasr-production", backend: "funasr", runtimeVersion: "1", modelSetVersion: "1", installSizeBytes: 1000, samples: [sample] },
      { role: "candidate", id: "funasr-candidate", backend: "funasr", runtimeVersion: "2", modelSetVersion: "2", installSizeBytes: 1000, samples: [sample] },
    ],
    status: "passed",
    approvedForDefaultSwitch: true,
    violations: [],
  });
  await writeFile(path.join(acceptedEvidenceRoot, `${platformId}-smoke.json`), JSON.stringify(smoke));
  await writeFile(path.join(acceptedEvidenceRoot, `${platformId}-benchmark.json`), JSON.stringify(benchmark));
}
const evidenceCheck = runScript("scripts/check-asr-evidence.mjs", ["--evidence-dir", acceptedEvidenceRoot]);
assert.equal(evidenceCheck.status, 0, evidenceCheck.stderr);
const tamperedPath = path.join(acceptedEvidenceRoot, "windows-x64-smoke.json");
const tampered = JSON.parse(await readFile(tamperedPath, "utf8"));
tampered.checks[0].status = "failed";
await writeFile(tamperedPath, JSON.stringify(tampered));
const tamperedCheck = runScript("scripts/check-asr-evidence.mjs", ["--evidence-dir", acceptedEvidenceRoot]);
assert.notEqual(tamperedCheck.status, 0);
assert.match(tamperedCheck.stderr, /digest does not match|not passed/);

console.log("ASR validation metrics and blocking fixtures passed.");

function runScript(script, args) {
  return spawnSync(process.execPath, [script, ...args], {
    cwd: path.resolve(path.dirname(fileURLToPath(import.meta.url)), ".."),
    encoding: "utf8",
  });
}

function signEvidence(evidence) {
  return {
    ...evidence,
    attestationDigest: `sha256:${createHash("sha256").update(JSON.stringify(evidence)).digest("hex")}`,
  };
}
