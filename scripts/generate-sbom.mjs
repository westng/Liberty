import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputIndex = process.argv.indexOf("--output");
const outputPath = outputIndex >= 0
  ? path.resolve(process.cwd(), process.argv[outputIndex + 1])
  : path.join(repositoryRoot, "artifacts/sbom/liberty.cdx.json");

const components = new Map();
const rootPackage = JSON.parse(await readFile(path.join(repositoryRoot, "package.json"), "utf8"));
const add = (ecosystem, name, version) => {
  if (!name || !version) return;
  const type = ecosystem === "cargo" ? "cargo" : ecosystem === "npm" ? "npm" : "pypi";
  components.set(`${type}:${name}@${version}`, {
    type: "library",
    name,
    version,
    purl: `pkg:${type}/${encodePurlName(type, name)}@${encodeURIComponent(version)}`,
  });
};

const cargoLock = await readFile(path.join(repositoryRoot, "Cargo.lock"), "utf8");
for (const block of cargoLock.split("[[package]]").slice(1)) {
  add("cargo", block.match(/\nname = "([^"]+)"/)?.[1], block.match(/\nversion = "([^"]+)"/)?.[1]);
}

const pnpmLock = await readFile(path.join(repositoryRoot, "pnpm-lock.yaml"), "utf8");
for (const match of pnpmLock.matchAll(/^  (?:'([^']+)'|([^:\s]+)):\n    resolution:/gm)) {
  const key = match[1] ?? match[2];
  const separator = key.lastIndexOf("@");
  if (separator > 0) add("npm", key.slice(0, separator), key.slice(separator + 1));
}

for (const lock of [
  "python/funasr-runner/requirements-lock-darwin-aarch64.txt",
  "python/funasr-runner/requirements-lock-darwin-x64.txt",
  "python/funasr-runner/requirements-lock-windows-x64.txt",
]) {
  const content = await readFile(path.join(repositoryRoot, lock), "utf8");
  for (const match of content.matchAll(/^([A-Za-z0-9_.-]+)==([^\s\\]+)/gm)) {
    add("pypi", match[1], match[2]);
  }
}

const document = {
  bomFormat: "CycloneDX",
  specVersion: "1.5",
  serialNumber: `urn:uuid:${crypto.randomUUID()}`,
  version: 1,
  metadata: {
    timestamp: new Date().toISOString(),
    tools: [{ vendor: "Liberty", name: "scripts/generate-sbom.mjs", version: "1" }],
    component: {
      type: "application",
      name: "Liberty",
      version: rootPackage.version,
      purl: `pkg:generic/liberty@${encodeURIComponent(rootPackage.version)}`,
    },
    properties: [
      { name: "liberty:sbom:node-version", value: process.version },
      { name: "liberty:sbom:source-locks", value: "Cargo.lock,pnpm-lock.yaml,requirements-lock-*" },
    ],
  },
  components: [...components.values()].sort((left, right) => left.purl.localeCompare(right.purl)),
};
const serialized = `${JSON.stringify(document, null, 2)}\n`;
if (serialized.includes(repositoryRoot) || containsAbsolutePath(document) || /api[_-]?key|bearer\s/i.test(serialized)) {
  throw new Error("SBOM contains a local path or credential-like text");
}
await mkdir(path.dirname(outputPath), { recursive: true });
await writeFile(outputPath, serialized, "utf8");
console.log(`Generated ${path.relative(repositoryRoot, outputPath)} with ${document.components.length} components.`);

function encodePurlName(type, name) {
  if (type === "npm" && name.startsWith("@") && name.includes("/")) {
    const [scope, packageName] = name.split("/", 2);
    return `${encodeURIComponent(scope)}/${encodeURIComponent(packageName)}`;
  }
  return encodeURIComponent(name);
}

function containsAbsolutePath(value) {
  if (typeof value === "string") return path.isAbsolute(value) || /^[A-Za-z]:[\\/]/.test(value);
  if (Array.isArray(value)) return value.some(containsAbsolutePath);
  return value && typeof value === "object" && Object.values(value).some(containsAbsolutePath);
}
