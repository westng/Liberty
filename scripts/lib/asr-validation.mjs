import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";

export const REQUIRED_SCENARIOS = [
  "short_meeting",
  "long_meeting",
  "overlapping_speakers",
  "noise",
  "mixed_zh_en",
  "chinese_numbers",
];

export const REQUIRED_SMOKE_CHECKS = [
  "native_install_start",
  "single_instance",
  "runtime_install",
  "audio_transcription",
  "degraded_diarization",
  "ai_transcript_only",
  "export",
  "legacy_database_upgrade",
  "credential_store",
];

export async function readJson(file) {
  return JSON.parse(await readFile(file, "utf8"));
}

export async function sha256File(file) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest("hex");
}

export async function validateBenchmarkManifest(manifest, manifestPath, thresholds) {
  const errors = [];
  if (manifest?.schemaVersion !== 1) errors.push("manifest schemaVersion must be 1");
  for (const key of ["corpusVersion", "annotationVersion", "deviceId"]) {
    if (!isNonEmpty(manifest?.[key])) errors.push(`manifest ${key} is required`);
  }
  const samples = Array.isArray(manifest?.samples) ? manifest.samples : [];
  if (samples.length === 0) errors.push("manifest must contain at least one sample");
  const sampleIds = new Set();
  const coveredScenarios = new Set();
  const manifestRoot = path.dirname(manifestPath);
  for (const sample of samples) {
    if (!isIdentifier(sample?.id)) {
      errors.push("every sample must have a safe non-empty id");
      continue;
    }
    if (sampleIds.has(sample.id)) errors.push(`duplicate sample id: ${sample.id}`);
    sampleIds.add(sample.id);
    if (!Array.isArray(sample.scenarios) || sample.scenarios.length === 0) {
      errors.push(`${sample.id} must declare at least one scenario`);
    } else {
      for (const scenario of sample.scenarios) coveredScenarios.add(scenario);
    }
    if (!isNonEmpty(sample.language)) errors.push(`${sample.id} language is required`);
    if (!Number.isInteger(sample.expectedSpeakerCount) || sample.expectedSpeakerCount < 1) {
      errors.push(`${sample.id} expectedSpeakerCount must be a positive integer`);
    }
    if (typeof sample.speakerRequired !== "boolean") {
      errors.push(`${sample.id} speakerRequired must be boolean`);
    }
    if (!isPositiveNumber(sample.media?.durationSeconds)) {
      errors.push(`${sample.id} media.durationSeconds must be greater than zero`);
    }
    if (!isNonEmpty(sample.annotation?.version)) {
      errors.push(`${sample.id} annotation.version is required`);
    }
    await validateHashedFile(sample.media, `${sample.id} media`, manifestRoot, errors);
    await validateHashedFile(sample.annotation, `${sample.id} annotation`, manifestRoot, errors);
    const annotationPath = resolveLocalPath(manifestRoot, sample.annotation?.path);
    if (annotationPath) {
      try {
        const annotation = await readJson(annotationPath);
        validateAnnotation(annotation, sample, errors);
      } catch (error) {
        errors.push(`${sample.id} annotation is not valid JSON: ${error.message}`);
      }
    }
  }
  for (const scenario of thresholds.requiredScenarios ?? REQUIRED_SCENARIOS) {
    if (!coveredScenarios.has(scenario)) errors.push(`required scenario is missing: ${scenario}`);
  }

  const engines = Array.isArray(manifest?.engines) ? manifest.engines : [];
  for (const role of ["baseline", "candidate"]) {
    const matching = engines.filter((engine) => engine?.role === role);
    if (matching.length !== 1) errors.push(`manifest must contain exactly one ${role} engine`);
  }
  for (const engine of engines) validateEngine(engine, manifestRoot, errors);
  return errors;
}

export function characterErrorRate(reference, hypothesis) {
  return tokenErrorRate(normalizeCharacters(reference), normalizeCharacters(hypothesis));
}

export function wordErrorRate(reference, hypothesis) {
  return tokenErrorRate(normalizeWords(reference), normalizeWords(hypothesis));
}

export function diarizationErrorRate(referenceTurns, hypothesisTurns) {
  if (!Array.isArray(referenceTurns) || referenceTurns.length === 0) return null;
  const references = normalizeTurns(referenceTurns);
  const hypotheses = normalizeTurns(hypothesisTurns ?? []);
  const referenceLabels = [...new Set(references.map((turn) => turn.speaker))];
  const hypothesisLabels = [...new Set(hypotheses.map((turn) => turn.speaker))];
  const overlap = referenceLabels.map(() => hypothesisLabels.map(() => 0));
  const boundaries = [...new Set(
    [...references, ...hypotheses].flatMap((turn) => [turn.startMs, turn.endMs]),
  )].sort((left, right) => left - right);
  let referenceDuration = 0;
  const intervals = [];
  for (let index = 1; index < boundaries.length; index += 1) {
    const startMs = boundaries[index - 1];
    const endMs = boundaries[index];
    const duration = endMs - startMs;
    if (duration <= 0) continue;
    const midpoint = startMs + duration / 2;
    const referenceActive = activeLabels(references, midpoint);
    const hypothesisActive = activeLabels(hypotheses, midpoint);
    referenceDuration += referenceActive.size * duration;
    intervals.push({ duration, referenceActive, hypothesisActive });
    for (const reference of referenceActive) {
      for (const hypothesis of hypothesisActive) {
        overlap[referenceLabels.indexOf(reference)][hypothesisLabels.indexOf(hypothesis)] += duration;
      }
    }
  }
  if (referenceDuration === 0) return null;
  const mapping = maximumOverlapMapping(referenceLabels, hypothesisLabels, overlap);
  let errorDuration = 0;
  for (const { duration, referenceActive, hypothesisActive } of intervals) {
    const mappedHypotheses = new Set(
      [...hypothesisActive].map((label) => mapping.get(label)).filter(Boolean),
    );
    const correct = [...referenceActive].filter((label) => mappedHypotheses.has(label)).length;
    const missed = Math.max(0, referenceActive.size - hypothesisActive.size);
    const falseAlarm = Math.max(0, hypothesisActive.size - referenceActive.size);
    const confusion = Math.max(0, Math.min(referenceActive.size, hypothesisActive.size) - correct);
    errorDuration += (missed + falseAlarm + confusion) * duration;
  }
  return errorDuration / referenceDuration;
}

export function percentile(values, quantile) {
  const sorted = values.filter(Number.isFinite).sort((left, right) => left - right);
  if (sorted.length === 0) return null;
  const index = Math.max(0, Math.ceil(sorted.length * quantile) - 1);
  return sorted[index];
}

export function median(values) {
  const sorted = values.filter(Number.isFinite).sort((left, right) => left - right);
  if (sorted.length === 0) return null;
  const midpoint = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[midpoint - 1] + sorted[midpoint]) / 2
    : sorted[midpoint];
}

export function evaluateBenchmarkPair(benchmark, thresholds) {
  const policy = thresholds.candidate;
  const violations = [];
  const baseline = benchmark.engines?.find((engine) => engine.role === "baseline");
  const candidate = benchmark.engines?.find((engine) => engine.role === "candidate");
  if (!baseline || !candidate) return ["benchmark must contain baseline and candidate engines"];
  const baselineSamples = new Map((baseline.samples ?? []).map((sample) => [sample.sampleId, sample]));
  const candidateSamples = new Map((candidate.samples ?? []).map((sample) => [sample.sampleId, sample]));
  const baselineFailures = [...baselineSamples.values()].filter((sample) => !sample.success).length;
  const candidateFailures = [...candidateSamples.values()].filter((sample) => !sample.success).length;
  if (candidateFailures - baselineFailures > policy.maximumNewFailures) {
    violations.push(`candidate introduces ${candidateFailures - baselineFailures} new sample failure(s)`);
  }
  const comparable = [...candidateSamples.values()].filter((sample) => (
    sample.success && baselineSamples.get(sample.sampleId)?.success
  ));
  const chinese = comparable.filter((sample) => String(sample.language).toLowerCase().includes("zh"));
  compareRegression(
    "median Chinese CER",
    median(chinese.map((sample) => baselineSamples.get(sample.sampleId).cer)),
    median(chinese.map((sample) => sample.cer)),
    policy.maximumChineseCerRegression,
    violations,
  );
  const diarization = comparable.filter((sample) => sample.speakerRequired);
  compareRegression(
    "median diarization DER",
    median(diarization.map((sample) => baselineSamples.get(sample.sampleId).der)),
    median(diarization.map((sample) => sample.der)),
    policy.maximumDiarizationDerRegression,
    violations,
  );
  if (candidateSamples.size !== baselineSamples.size) {
    violations.push("baseline and candidate sample sets differ");
  }
  if ([...candidateSamples.values()].some((sample) => sample.syntheticSpeakerLabels?.length > 0)) {
    violations.push("candidate produced synthetic-looking speaker labels");
  }
  const candidateRtf = percentile(comparable.map((sample) => sample.realTimeFactor), 0.95);
  const baselineRtf = percentile(
    comparable.map((sample) => baselineSamples.get(sample.sampleId).realTimeFactor),
    0.95,
  );
  compareMaximum("candidate P95 real-time factor", candidateRtf, policy.maximumP95RealTimeFactor, violations);
  compareRatio("P95 real-time factor", baselineRtf, candidateRtf, policy.maximumRuntimeMetricRegressionRatio, violations);
  const candidatePeakRss = maximum(comparable.map((sample) => sample.peakRssBytes));
  const baselinePeakRss = maximum(
    comparable.map((sample) => baselineSamples.get(sample.sampleId).peakRssBytes),
  );
  compareMaximum("candidate peak RSS", candidatePeakRss, policy.maximumPeakRssBytes, violations);
  compareRatio("peak RSS", baselinePeakRss, candidatePeakRss, policy.maximumRuntimeMetricRegressionRatio, violations);
  if (!Number.isFinite(baseline.installSizeBytes) || !Number.isFinite(candidate.installSizeBytes)) {
    violations.push("both engines must report install size");
  } else if (candidate.installSizeBytes - baseline.installSizeBytes > policy.maximumInstallSizeIncreaseBytes) {
    violations.push("candidate install size increase exceeds the allowed budget");
  }
  return violations;
}

export function validateSmokeInput(input) {
  const errors = [];
  if (input?.schemaVersion !== 1) errors.push("smoke input schemaVersion must be 1");
  if (!isNonEmpty(input?.operator)) errors.push("smoke input operator is required");
  if (!isIdentifier(input?.deviceId)) errors.push("smoke input deviceId is required");
  if (!isNonEmpty(input?.runtimeVersion)) errors.push("smoke input runtimeVersion is required");
  if (!isNonEmpty(input?.modelSetVersion)) errors.push("smoke input modelSetVersion is required");
  const checks = Array.isArray(input?.checks) ? input.checks : [];
  for (const checkId of REQUIRED_SMOKE_CHECKS) {
    const matching = checks.filter((check) => check?.id === checkId);
    if (matching.length !== 1) errors.push(`smoke input must contain exactly one ${checkId} check`);
  }
  for (const check of checks) {
    if (!REQUIRED_SMOKE_CHECKS.includes(check?.id)) errors.push(`unknown smoke check: ${check?.id}`);
    if (!["passed", "failed", "blocked"].includes(check?.status)) {
      errors.push(`${check?.id ?? "unknown"} has an invalid status`);
    }
    if (!isNonEmpty(check?.evidence)) errors.push(`${check?.id ?? "unknown"} evidence is required`);
  }
  return errors;
}

export function detectPlatformId(platform = process.platform, architecture = process.arch) {
  const mappings = new Map([
    ["darwin:arm64", "darwin-aarch64"],
    ["darwin:x64", "darwin-x64"],
    ["win32:x64", "windows-x64"],
    ["win32:ia32", "windows-x86"],
  ]);
  return mappings.get(`${platform}:${architecture}`) ?? null;
}

function normalizeCharacters(value) {
  return [...String(value ?? "").normalize("NFKC").toLowerCase()]
    .filter((character) => /[\p{L}\p{N}]/u.test(character));
}

function normalizeWords(value) {
  return String(value ?? "").normalize("NFKC").toLowerCase()
    .match(/[\p{L}\p{N}]+/gu) ?? [];
}

function tokenErrorRate(reference, hypothesis) {
  if (reference.length === 0) return hypothesis.length === 0 ? 0 : 1;
  const previous = Array.from({ length: hypothesis.length + 1 }, (_, index) => index);
  for (let referenceIndex = 1; referenceIndex <= reference.length; referenceIndex += 1) {
    const current = [referenceIndex];
    for (let hypothesisIndex = 1; hypothesisIndex <= hypothesis.length; hypothesisIndex += 1) {
      current[hypothesisIndex] = Math.min(
        current[hypothesisIndex - 1] + 1,
        previous[hypothesisIndex] + 1,
        previous[hypothesisIndex - 1] + (reference[referenceIndex - 1] === hypothesis[hypothesisIndex - 1] ? 0 : 1),
      );
    }
    previous.splice(0, previous.length, ...current);
  }
  return previous[hypothesis.length] / reference.length;
}

async function validateHashedFile(file, label, root, errors) {
  if (!isNonEmpty(file?.path) || !/^[a-f0-9]{64}$/.test(file?.sha256 ?? "")) {
    errors.push(`${label} requires path and lowercase SHA-256`);
    return;
  }
  const resolved = resolveLocalPath(root, file.path);
  try {
    const metadata = await stat(resolved);
    if (!metadata.isFile()) throw new Error("not a regular file");
    const digest = await sha256File(resolved);
    if (digest !== file.sha256) errors.push(`${label} SHA-256 mismatch`);
  } catch (error) {
    errors.push(`${label} is unavailable: ${error.message}`);
  }
}

function validateAnnotation(annotation, sample, errors) {
  if (annotation?.schemaVersion !== 1 || !isNonEmpty(annotation?.transcript)) {
    errors.push(`${sample.id} annotation requires schemaVersion=1 and transcript`);
  }
  if (sample.speakerRequired) {
    if (!Array.isArray(annotation?.speakerTurns) || annotation.speakerTurns.length === 0) {
      errors.push(`${sample.id} requires speakerTurns in its annotation`);
    } else if (new Set(annotation.speakerTurns.map((turn) => turn.speaker)).size !== sample.expectedSpeakerCount) {
      errors.push(`${sample.id} annotation speaker count does not match expectedSpeakerCount`);
    }
  }
  for (const turn of annotation?.speakerTurns ?? []) {
    if (!isNonEmpty(turn?.speaker) || !Number.isFinite(turn?.startMs) || !Number.isFinite(turn?.endMs)
        || turn.startMs < 0 || turn.endMs <= turn.startMs) {
      errors.push(`${sample.id} contains an invalid speaker turn`);
      break;
    }
  }
}

function validateEngine(engine, root, errors) {
  if (!isIdentifier(engine?.id)) errors.push("every engine must have a safe id");
  if (!["funasr", "sherpa-onnx"].includes(engine?.backend)) errors.push(`${engine?.id} backend is invalid`);
  for (const key of ["runtimeVersion", "modelSetVersion", "installRoot"]) {
    if (!isNonEmpty(engine?.[key])) errors.push(`${engine?.id ?? "engine"} ${key} is required`);
  }
  if (!Array.isArray(engine?.command) || engine.command.length === 0
      || engine.command.some((part) => !isNonEmpty(part))) {
    errors.push(`${engine?.id ?? "engine"} command must be a non-empty argument array`);
  }
  const forbiddenEnvironment = Object.keys(engine?.environment ?? {})
    .filter((name) => /(TOKEN|SECRET|PASSWORD|API_?KEY|CREDENTIAL)/i.test(name));
  if (forbiddenEnvironment.length > 0) {
    errors.push(`${engine?.id ?? "engine"} environment must not contain credential-like variables`);
  }
  for (const key of ["workingDirectory", "installRoot"]) {
    if (isNonEmpty(engine?.[key])) resolveLocalPath(root, engine[key]);
  }
}

function resolveLocalPath(root, value) {
  if (!isNonEmpty(value)) return null;
  return path.resolve(root, value);
}

function normalizeTurns(turns) {
  return turns.filter((turn) => isNonEmpty(turn?.speaker)
      && Number.isFinite(turn?.startMs) && Number.isFinite(turn?.endMs) && turn.endMs > turn.startMs)
    .map((turn) => ({ startMs: turn.startMs, endMs: turn.endMs, speaker: String(turn.speaker) }));
}

function activeLabels(turns, midpoint) {
  return new Set(turns.filter((turn) => turn.startMs <= midpoint && midpoint < turn.endMs)
    .map((turn) => turn.speaker));
}

function maximumOverlapMapping(referenceLabels, hypothesisLabels, overlap) {
  if (referenceLabels.length === 0 || hypothesisLabels.length === 0) return new Map();
  if (referenceLabels.length > 20 || hypothesisLabels.length > 20) {
    return greedyOverlapMapping(referenceLabels, hypothesisLabels, overlap);
  }
  const memo = new Map();
  const solve = (hypothesisIndex, usedMask) => {
    if (hypothesisIndex === hypothesisLabels.length) return { score: 0, assignments: [] };
    const key = `${hypothesisIndex}:${usedMask}`;
    if (memo.has(key)) return memo.get(key);
    let best = solve(hypothesisIndex + 1, usedMask);
    best = { score: best.score, assignments: [-1, ...best.assignments] };
    for (let referenceIndex = 0; referenceIndex < referenceLabels.length; referenceIndex += 1) {
      const bit = 1 << referenceIndex;
      if ((usedMask & bit) !== 0) continue;
      const next = solve(hypothesisIndex + 1, usedMask | bit);
      const score = overlap[referenceIndex][hypothesisIndex] + next.score;
      if (score > best.score) best = { score, assignments: [referenceIndex, ...next.assignments] };
    }
    memo.set(key, best);
    return best;
  };
  const result = solve(0, 0);
  return new Map(hypothesisLabels.map((label, index) => [
    label,
    result.assignments[index] >= 0 ? referenceLabels[result.assignments[index]] : null,
  ]));
}

function greedyOverlapMapping(referenceLabels, hypothesisLabels, overlap) {
  const pairs = referenceLabels.flatMap((reference, referenceIndex) => (
    hypothesisLabels.map((hypothesis, hypothesisIndex) => ({
      reference, hypothesis, score: overlap[referenceIndex][hypothesisIndex],
    }))
  )).sort((left, right) => right.score - left.score);
  const usedReferences = new Set();
  const mapping = new Map();
  for (const pair of pairs) {
    if (mapping.has(pair.hypothesis) || usedReferences.has(pair.reference)) continue;
    mapping.set(pair.hypothesis, pair.reference);
    usedReferences.add(pair.reference);
  }
  return mapping;
}

function compareRegression(label, baseline, candidate, allowed, violations) {
  if (!Number.isFinite(baseline) || !Number.isFinite(candidate)) {
    violations.push(`${label} is missing comparable measurements`);
  } else if (candidate - baseline > allowed) {
    violations.push(`${label} regression ${(candidate - baseline).toFixed(6)} exceeds ${allowed}`);
  }
}

function compareMaximum(label, candidate, maximumAllowed, violations) {
  if (!Number.isFinite(candidate)) violations.push(`${label} is missing`);
  else if (candidate > maximumAllowed) violations.push(`${label} exceeds ${maximumAllowed}`);
}

function compareRatio(label, baseline, candidate, maximumRatio, violations) {
  if (!Number.isFinite(baseline) || !Number.isFinite(candidate) || baseline <= 0) {
    violations.push(`${label} lacks a valid baseline comparison`);
  } else if (candidate / baseline > maximumRatio) {
    violations.push(`${label} ratio ${(candidate / baseline).toFixed(4)} exceeds ${maximumRatio}`);
  }
}

function maximum(values) {
  const finite = values.filter(Number.isFinite);
  return finite.length > 0 ? Math.max(...finite) : null;
}

function isNonEmpty(value) {
  return typeof value === "string" && value.trim().length > 0;
}

function isIdentifier(value) {
  return isNonEmpty(value) && /^[a-z0-9][a-z0-9._-]*$/.test(value);
}

function isPositiveNumber(value) {
  return Number.isFinite(value) && value > 0;
}
