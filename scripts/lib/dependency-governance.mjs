import { readFile } from "node:fs/promises";

export const CARGO_DENY_SECURITY_CHECKS = Object.freeze(["advisories", "bans", "sources"]);

export const OSV_LOCK_FILES = Object.freeze([
  "pnpm-lock.yaml",
  "python/funasr-runner/requirements-lock-darwin-aarch64.txt",
  "python/funasr-runner/requirements-lock-darwin-x64.txt",
  "python/funasr-runner/requirements-lock-windows-x64.txt",
]);

export const OSV_EXCEPTION_CONFIG = "python/funasr-runner/osv-scanner.toml";

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

export function assertOsvExceptionsAreGoverned(content, now = new Date()) {
  if (/^\s*(?:ignore|vulnerability\.ignore)\s*=\s*true\s*$/m.test(content)) {
    throw new Error("OSV policy must not ignore an entire package");
  }

  const entries = content.split(/^\[\[IgnoredVulns\]\]\s*$/m).slice(1);
  if (entries.length === 0) {
    throw new Error("OSV policy contains no vulnerability-specific exceptions");
  }

  const seenIds = new Set();
  for (const entry of entries) {
    const id = entry.match(/^id\s*=\s*"([^"]+)"\s*$/m)?.[1];
    const reason = entry.match(/^reason\s*=\s*"([^"]+)"\s*$/m)?.[1];
    const expiry = entry.match(/^ignoreUntil\s*=\s*(\d{4}-\d{2}-\d{2})\s*$/m)?.[1];
    if (!id || !/^(?:GHSA|PYSEC)-[A-Za-z0-9-]+$/.test(id)) {
      throw new Error("Every OSV exception must identify one GHSA or PYSEC vulnerability");
    }
    if (seenIds.has(id)) {
      throw new Error(`OSV policy contains duplicate exception ${id}`);
    }
    seenIds.add(id);
    if (!reason || reason.trim().length < 20) {
      throw new Error(`OSV exception ${id} must include a meaningful reason`);
    }
    if (!expiry) {
      throw new Error(`OSV exception ${id} must include ignoreUntil`);
    }
    const expiryInstant = new Date(`${expiry}T23:59:59Z`);
    if (Number.isNaN(expiryInstant.getTime()) || expiryInstant <= now) {
      throw new Error(`OSV exception ${id} expired on ${expiry}`);
    }
  }
}

export async function readPolicy(path) {
  return JSON.parse(await readFile(path, "utf8"));
}
