import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  assertNodeLockHasIntegrity,
  assertPythonLockHasHashes,
  evaluateDependencyFindings,
  readPolicy,
} from "./lib/dependency-governance.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const policy = await readPolicy(path.join(repositoryRoot, "config/dependency-governance.json"));
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
console.log("Dependency governance blocking fixtures passed.");
