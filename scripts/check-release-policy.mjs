import { readFileSync } from "node:fs";

const buildWorkflow = readFileSync(".github/workflows/build-desktop.yml", "utf8");
const qualityWorkflow = readFileSync(".github/workflows/quality.yml", "utf8");
const rustToolchain = readFileSync("rust-toolchain.toml", "utf8");
const errors = [];

for (const [workflowName, workflow] of [
  ["build-desktop.yml", buildWorkflow],
  ["quality.yml", qualityWorkflow],
]) {
  for (const match of workflow.matchAll(/^\s*uses:\s*([^\s#]+)/gm)) {
    const reference = match[1];
    if (reference.startsWith("./")) {
      continue;
    }
    if (!/@[a-f0-9]{40}$/.test(reference)) {
      errors.push(`${workflowName} must pin external action ${reference} to a full commit SHA`);
    }
  }
}

if (!/^channel\s*=\s*"1\.94\.0"\s*$/m.test(rustToolchain)) {
  errors.push("rust-toolchain.toml must pin Rust 1.94.0 exactly");
}
for (const [workflowName, workflow] of [
  ["build-desktop.yml", buildWorkflow],
  ["quality.yml", qualityWorkflow],
]) {
  if (!/toolchain:\s*1\.94\.0\b/.test(workflow)) {
    errors.push(`${workflowName} must install the pinned Rust 1.94.0 toolchain`);
  }
}

for (const [pattern, message] of [
  [/force\s*=\s*true/i, "release workflow must never force-update a tag"],
  [/gh\s+release\s+delete/i, "release workflow must never delete an existing Release"],
  [/--clobber\b/i, "release workflow must never overwrite an existing asset"],
  [/git\/refs\/tags|git\/refs[^\n]*-f\s+ref=["']?refs\/tags/i, "release workflow must never create or update tag refs"],
  [/git\s+(?:tag|push)\b/i, "release workflow must not mutate Git refs"],
  [/Apply Release Version/i, "build workflow must use the version committed in source"],
]) {
  if (pattern.test(buildWorkflow)) {
    errors.push(message);
  }
}

for (const forbiddenInput of ["publish_release", "version_mode", "INPUT_VERSION"]) {
  if (buildWorkflow.includes(forbiddenInput)) {
    errors.push(`manual builds must not expose ${forbiddenInput}`);
  }
}

assertMatch(
  buildWorkflow,
  /push:\s*\n\s+tags:\s*\n\s+- ["']v\*["']/,
  "desktop workflow must publish only from a pushed v* tag",
);
assertMatch(
  buildWorkflow,
  /workflow_dispatch:\s*(?:\n|$)/,
  "desktop workflow must retain a manual build-only trigger",
);
assertMatch(
  buildWorkflow,
  /quality:\s*\n(?:.|\n)*?uses:\s+\.\/\.github\/workflows\/quality\.yml/,
  "desktop workflow must call the reusable quality gate",
);

const buildJob = readJob(buildWorkflow, "build");
const releaseJob = readJob(buildWorkflow, "release");
for (const [jobName, job] of [["build", buildJob], ["release", releaseJob]]) {
  if (!/needs:\s*(?:\n\s+-[^\n]*)*\n\s+- quality\b/.test(job)
      && !/needs:\s*\[[^\]]*\bquality\b[^\]]*\]/.test(job)) {
    errors.push(`${jobName} job must explicitly depend on the quality gate`);
  }
}

assertMatch(
  releaseJob,
  /github\.event_name\s*==\s*'push'\s*&&\s*github\.ref_type\s*==\s*'tag'/,
  "release job must reject workflow_dispatch publication",
);
assertMatch(
  releaseJob,
  /node scripts\/check-release-state\.mjs/,
  "release job must verify the immutable tag and absent Release",
);
assertMatch(releaseJob, /gh release create[^\n]*|gh release create/, "release job must create a Release");
assertMatch(releaseJob, /--draft\b/, "Release must remain a draft until every asset is verified");
assertMatch(
  releaseJob,
  /--require-draft-release-id\b/,
  "release job must recheck the immutable tag and exact draft before publication",
);

const releaseSteps = [
  "Assemble and Validate Release Assets",
  "Resolve Draft Release",
  "Plan Draft Asset Upload",
  "Upload Missing Draft Assets",
  "Verify Uploaded Assets",
  "Publish Draft Release",
];
let previousIndex = -1;
for (const stepName of releaseSteps) {
  const stepIndex = releaseJob.indexOf(`name: ${stepName}`);
  if (stepIndex === -1) {
    errors.push(`release job is missing step: ${stepName}`);
  } else if (stepIndex <= previousIndex) {
    errors.push(`release step is out of order: ${stepName}`);
  }
  previousIndex = stepIndex;
}

for (const trigger of ["workflow_call:", "pull_request:", "push:"]) {
  if (!qualityWorkflow.includes(trigger)) {
    errors.push(`quality workflow is missing ${trigger}`);
  }
}
if ((qualityWorkflow.match(/- main\b/g) ?? []).length < 2) {
  errors.push("quality workflow must run for pull requests and pushes to main");
}

for (const command of [
  "pnpm desktop:typecheck",
  "pnpm rust:fmt:check",
  "cargo test --workspace --locked",
  "cargo clippy --workspace --all-targets --locked -- -D warnings",
  "pnpm release:check",
  "node scripts/check-release-policy.mjs",
  "node scripts/test-release-checks.mjs",
]) {
  if (!qualityWorkflow.includes(command)) {
    errors.push(`quality workflow is missing command: ${command}`);
  }
}

for (const recoveryGuard of [
  "--allow-resumable-draft",
  "plan-draft-upload",
  "draft-upload-list.txt",
  "liberty-release-commit:",
]) {
  if (!releaseJob.includes(recoveryGuard)) {
    errors.push(`release workflow is missing draft recovery guard: ${recoveryGuard}`);
  }
}

if (errors.length > 0) {
  console.error(errors.map((error) => `- ${error}`).join("\n"));
  process.exit(1);
}

console.log("Release workflow enforces immutable tags, non-overwrite publication, and quality gates.");

function assertMatch(contents, pattern, message) {
  if (!pattern.test(contents)) {
    errors.push(message);
  }
}

function readJob(contents, jobName) {
  const lines = contents.split("\n");
  const start = lines.findIndex((line) => line === `  ${jobName}:`);
  if (start === -1) {
    errors.push(`desktop workflow is missing ${jobName} job`);
    return "";
  }

  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^  [A-Za-z0-9_-]+:$/.test(lines[index])) {
      end = index;
      break;
    }
  }
  return lines.slice(start, end).join("\n");
}
