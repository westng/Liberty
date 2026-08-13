import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  CARGO_DENY_SECURITY_CHECKS,
  OSV_EXCEPTION_CONFIG,
  OSV_LOCK_FILES,
  assertNodeLockHasIntegrity,
  assertOsvExceptionsAreGoverned,
  assertPythonLockHasHashes,
  evaluateDependencyFindings,
  readPolicy,
} from "./lib/dependency-governance.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const policy = await readPolicy(path.join(repositoryRoot, "config/dependency-governance.json"));
const osvExceptions = await readFile(path.join(repositoryRoot, OSV_EXCEPTION_CONFIG), "utf8");
const violations = evaluateDependencyFindings(policy, [
  { kind: "license", package: "fixture-license", license: "GPL-3.0-only" },
  { kind: "vulnerability", package: "fixture-vulnerable", id: "OSV-TEST-1", severity: "HIGH" },
  { kind: "source", package: "fixture-source", sourceKind: "git-unpinned" },
]);
assert.equal(violations.length, 3);
assert.throws(() => assertPythonLockHasHashes("demo==1.0.0\n", "fixture-lock"), /unhashed/);
assert.throws(
  () => assertNodeLockHasIntegrity("packages:\n\n  demo@1.0.0:\n    resolution: {}\n\nsnapshots:\n"),
  /without integrity/,
);
assert.ok(CARGO_DENY_SECURITY_CHECKS.includes("advisories"));
assert.ok(OSV_LOCK_FILES.includes("pnpm-lock.yaml"));
assert.ok(OSV_LOCK_FILES.some((lockFile) => lockFile.includes("requirements-lock-")));
assert.ok(!OSV_LOCK_FILES.includes("Cargo.lock"));
assert.doesNotThrow(() => assertOsvExceptionsAreGoverned(osvExceptions, new Date("2026-08-13T00:00:00Z")));
assert.throws(
  () => assertOsvExceptionsAreGoverned('[[PackageOverrides]]\nname = "torch"\nignore = true\n'),
  /entire package/,
);
assert.throws(
  () => assertOsvExceptionsAreGoverned('[[IgnoredVulns]]\nid = "PYSEC-1"\nreason = "Temporary compatibility exception."\n'),
  /ignoreUntil/,
);
assert.throws(
  () => assertOsvExceptionsAreGoverned('[[IgnoredVulns]]\nid = "PYSEC-1"\nignoreUntil = 2026-01-01\nreason = "Temporary compatibility exception."\n', new Date("2026-08-13T00:00:00Z")),
  /expired/,
);
console.log("Dependency governance blocking fixtures passed.");
