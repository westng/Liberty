import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  closeSync,
  copyFileSync,
  ftruncateSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import process from "node:process";

const script = resolve("scripts/check-release-assets.mjs");
const source = readFileSync(script, "utf8");
const root = mkdtempSync(join(tmpdir(), "liberty-release-checks-"));
const local = join(root, "local");
const existing = join(root, "existing");
const manifestPath = join(root, "release-manifest.json");
const uploadList = join(root, "draft-upload-list.txt");
const version = "9.8.7";
const sbomPath = join(root, `liberty-${version}.cdx.json`);
const largeFileSize = 256 * 1024 * 1024;
const maxRssMib = Number(process.env.RELEASE_HASH_MAX_RSS_MIB ?? "192");

try {
  if (!source.includes("createReadStream") || /update\(readFileSync\(/.test(source)) {
    throw new Error("Release asset hashing must use a read stream, not a whole-file buffer.");
  }

  mkdirSync(local, { recursive: true });
  mkdirSync(existing, { recursive: true });

  const assetDefinitions = [
    ["darwin-aarch64", "dmg", "arm64", true],
    ["darwin-x64", "dmg", "x86_64", false],
    ["windows-x64", "msi", "x86_64", false],
    ["windows-x64", "nsis", "x86_64", false],
    ["windows-x86", "msi", "x86", false],
    ["windows-x86", "nsis", "x86", false],
  ];
  const zeroChunk = Buffer.alloc(1024 * 1024);
  const largeDigest = createHash("sha256");
  for (let offset = 0; offset < largeFileSize; offset += zeroChunk.length) {
    largeDigest.update(zeroChunk);
  }

  let assets = assetDefinitions.map(([platformId, kind, architecture, isLarge], index) => {
    const extension = kind === "nsis" ? "exe" : kind;
    const name = `liberty-${version}-${platformId}.${extension}`;
    const path = join(local, name);
    if (isLarge) {
      const descriptor = openSync(path, "w");
      ftruncateSync(descriptor, largeFileSize);
      closeSync(descriptor);
      return {
        name,
        kind,
        platformId,
        architecture,
        size: largeFileSize,
        sha256: largeDigest.digest("hex"),
      };
    }

    const contents = Buffer.from(`release-asset-${index}\n`);
    writeFileSync(path, contents);
    return {
      name,
      kind,
      platformId,
      architecture,
      size: contents.length,
      sha256: createHash("sha256").update(contents).digest("hex"),
    };
  });

  writeFileSync(
    manifestPath,
    `${JSON.stringify({ schemaVersion: 1, tag: `v${version}`, version, assets }, null, 2)}\n`,
  );
  writeFileSync(sbomPath, `${JSON.stringify({
    bomFormat: "CycloneDX",
    specVersion: "1.5",
    metadata: { component: { type: "application", name: "Liberty", version } },
    components: [],
  }, null, 2)}\n`);
  run(process.execPath, [
    script,
    "add-sbom",
    "--input",
    sbomPath,
    "--output",
    local,
    "--manifest",
    manifestPath,
  ]);
  assets = JSON.parse(readFileSync(manifestPath, "utf8")).assets;
  writeFileSync(
    join(local, "SHA256SUMS.txt"),
    `${assets.map(({ name, sha256 }) => `${sha256}  ${name}`).join("\n")}\n`,
  );

  const verification = run(process.execPath, [
    script,
    "verify-directory",
    "--input",
    local,
    "--manifest",
    manifestPath,
    "--allow-checksums",
    "--report-memory",
  ]);
  const maxRssMatch = verification.stdout.match(/release-check-max-rss-kib=(\d+)/);
  if (!maxRssMatch) {
    throw new Error("Streaming hash self-test did not report peak RSS.");
  }
  const measuredRssMib = Number(maxRssMatch[1]) / 1024;
  if (measuredRssMib >= maxRssMib) {
    throw new Error(
      `Streaming hash peak RSS was ${measuredRssMib.toFixed(1)} MiB; expected less than ${maxRssMib} MiB.`,
    );
  }

  const invalidArchitectureManifest = {
    schemaVersion: 1,
    tag: `v${version}`,
    version,
    assets: assets.map((asset, index) => (
      index === 0 ? { ...asset, architecture: "x86_64" } : asset
    )),
  };
  writeFileSync(manifestPath, `${JSON.stringify(invalidArchitectureManifest, null, 2)}\n`);
  const architectureMismatch = spawnSync(process.execPath, [
    script,
    "verify-directory",
    "--input",
    local,
    "--manifest",
    manifestPath,
    "--allow-checksums",
  ], { encoding: "utf8" });
  if (architectureMismatch.status === 0 || !architectureMismatch.stderr.includes("invalid metadata")) {
    throw new Error("Release verification must reject an asset with mismatched architecture metadata.");
  }
  writeFileSync(
    manifestPath,
    `${JSON.stringify({ schemaVersion: 1, tag: `v${version}`, version, assets }, null, 2)}\n`,
  );

  copyFileSync(join(local, assets[1].name), join(existing, assets[1].name));
  run(process.execPath, [
    script,
    "plan-draft-upload",
    "--local",
    local,
    "--existing",
    existing,
    "--manifest",
    manifestPath,
    "--output",
    uploadList,
  ]);
  const missingAssets = readFileSync(uploadList, "utf8").trim().split("\n").filter(Boolean);
  if (missingAssets.length !== assets.length || missingAssets.includes(assets[1].name)) {
    throw new Error("Draft recovery did not reuse the matching existing asset safely.");
  }

  writeFileSync(join(existing, assets[1].name), "tampered\n");
  const mismatch = spawnSync(process.execPath, [
    script,
    "plan-draft-upload",
    "--local",
    local,
    "--existing",
    existing,
    "--manifest",
    manifestPath,
    "--output",
    uploadList,
  ], { encoding: "utf8" });
  if (mismatch.status === 0 || !mismatch.stderr.includes("refusing to overwrite")) {
    throw new Error("Draft recovery must reject an existing asset with different bytes.");
  }

  console.log(
    `Release checks self-test passed; 256 MiB hash peak RSS ${measuredRssMib.toFixed(1)} MiB.`,
  );
} finally {
  rmSync(root, { recursive: true, force: true });
}

function run(executable, args) {
  const result = spawnSync(executable, args, {
    encoding: "utf8",
    maxBuffer: 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(
      `${executable} ${args.join(" ")} failed:\n${result.stderr || result.stdout}`,
    );
  }
  return result;
}
