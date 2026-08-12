import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import {
  characterErrorRate,
  detectPlatformId,
  diarizationErrorRate,
  evaluateBenchmarkPair,
  readJson,
  validateBenchmarkManifest,
  wordErrorRate,
} from "./lib/asr-validation.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = resolveOption("--manifest", "benchmarks/asr/manifest.local.json");
const outputPath = resolveOption(
  "--output",
  `artifacts/asr-validation/benchmark-${new Date().toISOString().replaceAll(/[:.]/g, "-")}.json`,
);
const allowDirty = process.argv.includes("--allow-dirty");
const thresholds = await readJson(path.join(repositoryRoot, "benchmarks/asr/thresholds.json"));
const platforms = await readJson(path.join(repositoryRoot, "benchmarks/asr/platforms.json"));
const manifest = await loadManifest();
const manifestErrors = await validateBenchmarkManifest(manifest, manifestPath, thresholds);
if (manifestErrors.length > 0) fail(manifestErrors);

const platformId = detectPlatformId();
const platform = platforms.platforms.find((entry) => entry.platformId === platformId);
if (!platform?.localAsrRequired) fail([`current platform ${platformId ?? "unknown"} is not a required local ASR benchmark target`]);
const commit = git(["rev-parse", "HEAD"]);
const dirty = git(["status", "--porcelain"]).length > 0;
if (dirty && !allowDirty) {
  fail(["worktree is dirty; commit the exact benchmark source or use --allow-dirty for non-acceptable exploratory evidence"]);
}

const manifestRoot = path.dirname(manifestPath);
const device = {
  id: manifest.deviceId,
  platformId,
  operatingSystem: `${os.type()} ${os.release()}`,
  architecture: os.arch(),
  cpuModel: os.cpus()[0]?.model ?? "unknown",
  logicalCores: os.cpus().length,
  totalMemoryBytes: os.totalmem(),
  performanceTierQualified: os.totalmem() >= (platform.minimumMemoryGiB - 0.5) * 1024 ** 3
    && os.totalmem() <= (platform.performanceEvidenceMemoryTierGiB + 0.5) * 1024 ** 3,
};

const engines = [];
for (const engine of manifest.engines) {
  console.log(`Running ${engine.role} engine ${engine.id} on ${manifest.samples.length} samples...`);
  const installRoot = path.resolve(manifestRoot, engine.installRoot);
  const engineEvidence = {
    role: engine.role,
    id: engine.id,
    backend: engine.backend,
    runtimeVersion: engine.runtimeVersion,
    modelSetVersion: engine.modelSetVersion,
    installSizeBytes: await directorySize(installRoot),
    samples: [],
  };
  for (const sample of manifest.samples) {
    engineEvidence.samples.push(await runSample(engine, sample));
  }
  engines.push(engineEvidence);
}

const evidence = {
  schemaVersion: 1,
  evidenceType: "asr-benchmark",
  generatedAt: new Date().toISOString(),
  commit,
  dirty,
  platformId,
  device,
  corpusVersion: manifest.corpusVersion,
  annotationVersion: manifest.annotationVersion,
  engines,
};
const violations = evaluateBenchmarkPair(evidence, thresholds);
if (dirty) violations.push("benchmark evidence was generated from a dirty worktree");
if (!device.performanceTierQualified) {
  violations.push(`device memory does not qualify as the ${platform.performanceEvidenceMemoryTierGiB} GiB performance tier`);
}
evidence.status = violations.length === 0 ? "passed" : "failed";
evidence.approvedForDefaultSwitch = evidence.status === "passed";
evidence.violations = violations;
evidence.attestationDigest = digestEvidence(evidence);
await mkdir(path.dirname(outputPath), { recursive: true });
await writeFile(outputPath, `${JSON.stringify(evidence, null, 2)}\n`, "utf8");
console.log(`Wrote redacted ASR benchmark evidence: ${path.relative(repositoryRoot, outputPath)}`);
if (violations.length > 0) fail(violations);

async function runSample(engine, sample) {
  const sampleRoot = path.join(path.dirname(outputPath), "jobs", safeName(engine.id), safeName(sample.id));
  await mkdir(sampleRoot, { recursive: true });
  const mediaPath = path.resolve(manifestRoot, sample.media.path);
  const annotation = await readJson(path.resolve(manifestRoot, sample.annotation.path));
  const [executable, ...prefixArgs] = engine.command;
  const args = [
    ...prefixArgs,
    "--job-dir", sampleRoot,
    "--input", mediaPath,
    "--lang", sample.language,
    "--speaker", String(sample.speakerRequired),
  ];
  const startedAt = Date.now();
  const child = spawn(executable, args, {
    cwd: engine.workingDirectory ? path.resolve(manifestRoot, engine.workingDirectory) : repositoryRoot,
    env: buildRunnerEnvironment(engine),
    stdio: ["ignore", "pipe", "pipe"],
    shell: false,
  });
  let outputBytes = 0;
  let errorBytes = 0;
  child.stdout.on("data", (chunk) => { outputBytes += chunk.length; });
  child.stderr.on("data", (chunk) => { errorBytes += chunk.length; });
  let peakRssBytes = 0;
  let cpuSeconds = 0;
  let modelReadyAt = null;
  const sampler = setInterval(async () => {
    const usage = processTreeUsage(child.pid);
    peakRssBytes = Math.max(peakRssBytes, usage.rssBytes);
    cpuSeconds = Math.max(cpuSeconds, usage.cpuSeconds);
    if (modelReadyAt === null) {
      try {
        const progress = JSON.parse(await readFile(path.join(sampleRoot, "progress.json"), "utf8"));
        if (progress.progressPercent >= 32) modelReadyAt = Date.now();
      } catch {}
    }
  }, process.platform === "win32" ? 1000 : 250);
  const exit = await new Promise((resolve) => {
    child.on("error", (error) => resolve({ code: null, signal: null, spawnError: error.code ?? "spawn_failed" }));
    child.on("exit", (code, signal) => resolve({ code, signal, spawnError: null }));
  });
  clearInterval(sampler);
  const elapsedMs = Date.now() - startedAt;
  const finalUsage = processTreeUsage(child.pid);
  peakRssBytes = Math.max(peakRssBytes, finalUsage.rssBytes);
  cpuSeconds = Math.max(cpuSeconds, finalUsage.cpuSeconds);
  let result = null;
  let failureCode = exit.spawnError ?? (exit.code === 0 ? null : `runner_exit_${exit.code ?? exit.signal ?? "unknown"}`);
  if (!failureCode) {
    try {
      result = await readJson(path.join(sampleRoot, "result.json"));
      if (!Array.isArray(result.transcriptSegments) || result.transcriptSegments.length === 0) {
        failureCode = "invalid_or_empty_result";
      }
    } catch {
      failureCode = "result_unavailable";
    }
  }
  const hypothesis = result?.transcriptSegments?.map((segment) => segment.text).join(" ") ?? "";
  const labels = result?.speakerSegments?.map((segment) => segment.speaker) ?? [];
  return {
    sampleId: sample.id,
    mediaSha256: sample.media.sha256,
    annotationSha256: sample.annotation.sha256,
    annotationVersion: sample.annotation.version,
    scenarios: sample.scenarios,
    language: sample.language,
    speakerRequired: sample.speakerRequired,
    success: failureCode === null,
    failureCode,
    cer: failureCode === null ? characterErrorRate(annotation.transcript, hypothesis) : null,
    wer: failureCode === null ? wordErrorRate(annotation.transcript, hypothesis) : null,
    der: failureCode === null && sample.speakerRequired
      ? diarizationErrorRate(annotation.speakerTurns, result.speakerSegments)
      : null,
    diarizationStatus: result?.diarizationStatus ?? null,
    syntheticSpeakerLabels: [...new Set(labels.filter((label) => /^(speaker\s+\d+|unknown|default|未知|默认)$/i.test(label)))],
    durationSeconds: sample.media.durationSeconds,
    elapsedMs,
    coldStartMs: modelReadyAt === null ? null : modelReadyAt - startedAt,
    realTimeFactor: elapsedMs / 1000 / sample.media.durationSeconds,
    peakRssBytes,
    cpuSeconds,
    averageCpuPercent: elapsedMs > 0 ? cpuSeconds / (elapsedMs / 1000) * 100 : null,
    protocolOutputBytes: outputBytes,
    diagnosticErrorBytes: errorBytes,
  };
}

function buildRunnerEnvironment(engine) {
  const allowedNames = [
    "PATH", "HOME", "TMPDIR", "TEMP", "TMP", "LANG", "LC_ALL", "SystemRoot", "ComSpec",
    "USERPROFILE", "APPDATA", "LOCALAPPDATA", "PROGRAMDATA",
  ];
  const environment = Object.fromEntries(
    allowedNames.filter((name) => process.env[name] !== undefined).map((name) => [name, process.env[name]]),
  );
  return {
    ...environment,
    ...engine.environment,
    LIBERTY_ASR_BACKEND: engine.backend,
    PYTHONUNBUFFERED: "1",
  };
}

function processTreeUsage(rootPid) {
  if (!Number.isInteger(rootPid)) return { rssBytes: 0, cpuSeconds: 0 };
  if (process.platform === "win32") return windowsProcessTreeUsage(rootPid);
  const result = spawnSync("ps", ["-axo", "pid=,ppid=,rss=,time="], { encoding: "utf8" });
  if (result.status !== 0) return { rssBytes: 0, cpuSeconds: 0 };
  const rows = result.stdout.split("\n").map((line) => line.trim().split(/\s+/)).filter((parts) => parts.length >= 4)
    .map(([pid, parentPid, rssKiB, cpuTime]) => ({
      pid: Number(pid), parentPid: Number(parentPid), rssBytes: Number(rssKiB) * 1024, cpuSeconds: parseCpuTime(cpuTime),
    }));
  return sumProcessTree(rows, rootPid);
}

function windowsProcessTreeUsage(rootPid) {
  const script = "Get-CimInstance Win32_Process | ForEach-Object { $p=Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue; if($p){[pscustomobject]@{pid=$_.ProcessId;ppid=$_.ParentProcessId;rss=$p.WorkingSet64;cpu=$p.CPU}} } | ConvertTo-Json -Compress";
  const result = spawnSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-Command", script], { encoding: "utf8" });
  if (result.status !== 0 || !result.stdout.trim()) return { rssBytes: 0, cpuSeconds: 0 };
  try {
    const parsed = JSON.parse(result.stdout);
    const rows = (Array.isArray(parsed) ? parsed : [parsed]).map((row) => ({
      pid: Number(row.pid), parentPid: Number(row.ppid), rssBytes: Number(row.rss), cpuSeconds: Number(row.cpu),
    }));
    return sumProcessTree(rows, rootPid);
  } catch {
    return { rssBytes: 0, cpuSeconds: 0 };
  }
}

function sumProcessTree(rows, rootPid) {
  const processIds = new Set([rootPid]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const row of rows) {
      if (processIds.has(row.parentPid) && !processIds.has(row.pid)) {
        processIds.add(row.pid);
        changed = true;
      }
    }
  }
  return rows.filter((row) => processIds.has(row.pid)).reduce((total, row) => ({
    rssBytes: total.rssBytes + (Number.isFinite(row.rssBytes) ? row.rssBytes : 0),
    cpuSeconds: total.cpuSeconds + (Number.isFinite(row.cpuSeconds) ? row.cpuSeconds : 0),
  }), { rssBytes: 0, cpuSeconds: 0 });
}

function parseCpuTime(value) {
  const dayParts = value.split("-");
  const clock = dayParts.at(-1).split(":").map(Number);
  const seconds = clock.reduce((total, part) => total * 60 + part, 0);
  return seconds + (dayParts.length > 1 ? Number(dayParts[0]) * 86400 : 0);
}

async function directorySize(root) {
  const metadata = await stat(root);
  if (metadata.isFile()) return metadata.size;
  let total = 0;
  for (const entry of await readdir(root, { withFileTypes: true })) {
    if (entry.isSymbolicLink()) continue;
    total += await directorySize(path.join(root, entry.name));
  }
  return total;
}

async function loadManifest() {
  try {
    return await readJson(manifestPath);
  } catch (error) {
    if (error?.code === "ENOENT") fail([`local benchmark manifest is missing: ${path.relative(repositoryRoot, manifestPath)}`]);
    throw error;
  }
}

function git(args) {
  const result = spawnSync("git", args, { cwd: repositoryRoot, encoding: "utf8" });
  if (result.status !== 0) fail([`git ${args[0]} failed; benchmark evidence requires a repository commit`]);
  return result.stdout.trim();
}

function digestEvidence(evidence) {
  return `sha256:${createHash("sha256").update(JSON.stringify(evidence)).digest("hex")}`;
}

function resolveOption(name, fallback) {
  const index = process.argv.indexOf(name);
  const value = index >= 0 ? process.argv[index + 1] : fallback;
  if (!value || value.startsWith("--")) fail([`${name} requires a path`]);
  return path.resolve(process.cwd(), value);
}

function safeName(value) {
  return String(value).replaceAll(/[^a-zA-Z0-9._-]/g, "_");
}

function fail(errors) {
  console.error(errors.map((error) => `- ${error}`).join("\n"));
  process.exit(1);
}
