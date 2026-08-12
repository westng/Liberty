import { readFile } from "node:fs/promises";

export function evaluateDependencyFindings(policy, findings) {
  const deniedLicenses = new Set(policy.deniedLicenses);
  const allowedSources = new Set(policy.allowedSourceKinds);
  const blockedSeverities = new Set(policy.vulnerabilitySeverityBlocklist);
  const violations = [];
  for (const finding of findings) {
    if (finding.kind === "license" && deniedLicenses.has(finding.license)) {
      violations.push(`denied license ${finding.license}: ${finding.package}`);
    }
    if (finding.kind === "source" && !allowedSources.has(finding.sourceKind)) {
      violations.push(`unlocked source ${finding.sourceKind}: ${finding.package}`);
    }
    if (finding.kind === "vulnerability" && blockedSeverities.has(finding.severity)) {
      violations.push(`known vulnerability ${finding.id}: ${finding.package}`);
    }
  }
  return violations;
}

export function assertPythonLockHasHashes(content, fileName) {
  const entries = content
    .split("\n")
    .filter((line) => /^[A-Za-z0-9_.-]+==[^\s]+/.test(line));
  if (entries.length === 0) {
    throw new Error(`${fileName} contains no pinned packages`);
  }
  for (const entry of entries) {
    const start = content.indexOf(entry);
    const nextEntry = content.slice(start + entry.length).search(/\n[A-Za-z0-9_.-]+==/);
    const block = nextEntry < 0
      ? content.slice(start)
      : content.slice(start, start + entry.length + nextEntry);
    if (!block.includes("--hash=sha256:")) {
      throw new Error(`${fileName} has an unhashed entry: ${entry.split(" ")[0]}`);
    }
  }
}

export function assertNodeLockHasIntegrity(content) {
  const packageSection = content.split("\nsnapshots:")[0] ?? content;
  const packageEntries = packageSection.match(/^  [^\s].*:\n(?:    .*\n)*/gm) ?? [];
  const missing = packageEntries
    .filter((entry) => /resolution:/.test(entry))
    .filter((entry) => !/integrity:/.test(entry) && !/tarball:/.test(entry));
  if (missing.length > 0) {
    throw new Error(`pnpm lock has ${missing.length} resolved package(s) without integrity`);
  }
}

export async function readPolicy(path) {
  return JSON.parse(await readFile(path, "utf8"));
}
