import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  closeSync,
  createReadStream,
  mkdirSync,
  openSync,
  readSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, extname, join, resolve } from "node:path";
import process from "node:process";
import { runPowerShellWithAsset } from "./lib/powershell.mjs";

const platformMatrix = new Map([
  ["darwin-aarch64", { kinds: ["dmg"], architecture: "arm64" }],
  ["darwin-x64", { kinds: ["dmg"], architecture: "x86_64" }],
  ["windows-x64", { kinds: ["msi", "nsis"], architecture: "x86_64" }],
  ["windows-x86", { kinds: ["msi", "nsis"], architecture: "x86" }],
]);
const kindExtensions = new Map([
  ["dmg", ".dmg"],
  ["msi", ".msi"],
  ["nsis", ".exe"],
]);
const [command, ...rawArgs] = process.argv.slice(2);
const args = parseOptions(rawArgs);

try {
  if (command === "prepare-platform") {
    await preparePlatform(args);
  } else if (command === "assemble") {
    await assemble(args);
  } else if (command === "add-sbom") {
    await addSbom(args);
  } else if (command === "verify-directory") {
    await verifyDirectory(args);
  } else if (command === "plan-draft-upload") {
    await planDraftUpload(args);
  } else {
    fail(`Unknown command: ${command ?? "missing"}.`);
  }

  if (args["report-memory"] === "true") {
    console.log(`release-check-max-rss-kib=${process.resourceUsage().maxRSS}`);
  }
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}

async function preparePlatform(options) {
  requireOptions(options, ["platform", "asset-kinds", "version", "bundle-root", "output"]);
  const platform = platformMatrix.get(options.platform);
  if (!platform) {
    fail(`Unknown release platform: ${options.platform}.`);
  }

  const requestedKinds = options["asset-kinds"].split(",").filter(Boolean);
  if (!sameValues(requestedKinds, platform.kinds)) {
    fail(
      `${options.platform} must produce ${platform.kinds.join(",")}; received ${requestedKinds.join(",")}.`,
    );
  }

  const bundleRoot = resolve(options["bundle-root"]);
  const files = walkFiles(bundleRoot);
  validatePlatformBundle(options.platform, options.version, platform.architecture, bundleRoot, files);
  mkdirSync(options.output, { recursive: true });

  const assets = [];
  for (const kind of platform.kinds) {
    const extension = kindExtensions.get(kind);
    const candidates = files.filter((file) =>
      extname(file).toLowerCase() === extension && matchesBundleKind(file, kind),
    );
    if (candidates.length !== 1) {
      fail(
        `${options.platform} must contain exactly one ${kind} asset; found ${candidates.length}.`,
      );
    }

    const name = `liberty-${options.version}-${options.platform}.${kind === "nsis" ? "exe" : kind}`;
    const destination = join(options.output, name);
    copyFileSync(candidates[0], destination);
    assets.push(await describeAsset(destination, kind, platform.architecture));
  }

  const manifest = {
    schemaVersion: 1,
    platformId: options.platform,
    version: options.version,
    assets,
  };
  writeFileSync(
    join(options.output, `manifest-${options.platform}.json`),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
  console.log(`Validated ${options.platform} ${options.version}: ${assets.map(({ name }) => name).join(", ")}`);
}

async function assemble(options) {
  requireOptions(options, ["input", "output", "manifest", "tag", "version"]);
  if (options.tag !== `v${options.version}`) {
    fail(`Tag ${options.tag} must exactly match asset version v${options.version}.`);
  }

  const inputFiles = walkFiles(resolve(options.input));
  const manifestFiles = inputFiles.filter((file) => /^manifest-.+\.json$/.test(basename(file)));
  if (manifestFiles.length !== platformMatrix.size) {
    fail(`Expected ${platformMatrix.size} platform manifests; found ${manifestFiles.length}.`);
  }

  const manifests = manifestFiles.map((file) => JSON.parse(readFileSync(file, "utf8")));
  const platformIds = manifests.map(({ platformId }) => platformId);
  if (!sameValues(platformIds, [...platformMatrix.keys()])) {
    fail(`Platform manifests must be exactly: ${[...platformMatrix.keys()].join(", ")}.`);
  }

  mkdirSync(options.output, { recursive: true });
  const combinedAssets = [];
  const seenNames = new Set();
  for (const manifest of manifests) {
    validateManifest(manifest, options.version);
    for (const asset of manifest.assets) {
      if (seenNames.has(asset.name)) {
        fail(`Duplicate release asset name: ${asset.name}.`);
      }
      seenNames.add(asset.name);
      const candidates = inputFiles.filter((file) => basename(file) === asset.name);
      if (candidates.length !== 1) {
        fail(`Expected exactly one downloaded copy of ${asset.name}; found ${candidates.length}.`);
      }
      await verifyAsset(candidates[0], asset);
      const destination = join(options.output, asset.name);
      copyFileSync(candidates[0], destination);
      combinedAssets.push({ ...asset, platformId: manifest.platformId });
    }
  }

  const releaseManifest = {
    schemaVersion: 1,
    tag: options.tag,
    version: options.version,
    assets: combinedAssets.sort((left, right) => left.name.localeCompare(right.name)),
  };
  writeFileSync(options.manifest, `${JSON.stringify(releaseManifest, null, 2)}\n`);
  console.log(`Validated complete release matrix with ${combinedAssets.length} installer assets.`);
}

async function verifyDirectory(options) {
  requireOptions(options, ["input", "manifest"]);
  const manifest = JSON.parse(readFileSync(options.manifest, "utf8"));
  validateReleaseManifest(manifest);
  const files = readdirSync(options.input, { withFileTypes: true });
  if (files.some((entry) => !entry.isFile())) {
    fail(`${options.input} must contain files only.`);
  }

  const expectedNames = manifest.assets.map(({ name }) => name);
  if (options["allow-checksums"] === "true") {
    expectedNames.push("SHA256SUMS.txt");
  }
  const actualNames = files.map(({ name }) => name);
  if (!sameValues(actualNames, expectedNames)) {
    fail(
      `Asset directory mismatch. Expected ${expectedNames.sort().join(", ")}; found ${actualNames.sort().join(", ")}.`,
    );
  }

  for (const asset of manifest.assets) {
    await verifyAsset(join(options.input, asset.name), asset);
  }
  if (options["allow-checksums"] === "true") {
    verifyChecksumFile(join(options.input, "SHA256SUMS.txt"), manifest.assets);
  }
  console.log(`Verified ${manifest.assets.length} release assets and their SHA-256 digests.`);
}

async function addSbom(options) {
  requireOptions(options, ["input", "output", "manifest"]);
  const manifest = JSON.parse(readFileSync(options.manifest, "utf8"));
  validateInstallerManifest(manifest);
  const expectedName = `liberty-${manifest.version}.cdx.json`;
  if (basename(options.input) !== expectedName) {
    fail(`SBOM must be named ${expectedName}.`);
  }
  const sbom = JSON.parse(readFileSync(options.input, "utf8"));
  if (sbom.bomFormat !== "CycloneDX" || sbom.specVersion !== "1.5"
      || sbom.metadata?.component?.version !== manifest.version) {
    fail("SBOM format or application version does not match the Release.");
  }
  const serialized = JSON.stringify(sbom);
  if (containsAbsolutePath(sbom) || /api[_-]?key|bearer\s/i.test(serialized)) {
    fail("SBOM contains a local absolute path or credential-like text.");
  }
  mkdirSync(options.output, { recursive: true });
  const destination = join(options.output, expectedName);
  copyFileSync(options.input, destination);
  manifest.assets.push({
    ...await describeAsset(destination, "sbom", "universal"),
    platformId: "all",
  });
  manifest.assets.sort((left, right) => left.name.localeCompare(right.name));
  writeFileSync(options.manifest, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`Attached validated CycloneDX SBOM ${expectedName}.`);
}

async function planDraftUpload(options) {
  requireOptions(options, ["local", "existing", "manifest", "output"]);
  const manifest = JSON.parse(readFileSync(options.manifest, "utf8"));
  await verifyDirectory({
    input: options.local,
    manifest: options.manifest,
    "allow-checksums": "true",
  });

  const expectedNames = new Set([
    ...manifest.assets.map(({ name }) => name),
    "SHA256SUMS.txt",
  ]);
  const existingEntries = readdirSync(options.existing, { withFileTypes: true });
  if (existingEntries.some((entry) => !entry.isFile())) {
    fail(`${options.existing} must contain files only.`);
  }

  for (const { name } of existingEntries) {
    if (!expectedNames.has(name)) {
      fail(`Draft Release contains unexpected existing asset ${name}; refusing to delete it.`);
    }

    const localAsset = await describeAsset(join(options.local, name), "existing");
    const existingAsset = await describeAsset(join(options.existing, name), "existing");
    if (localAsset.size !== existingAsset.size || localAsset.sha256 !== existingAsset.sha256) {
      fail(`Existing draft asset ${name} differs from the validated build; refusing to overwrite it.`);
    }
  }

  const existingNames = new Set(existingEntries.map(({ name }) => name));
  const missingNames = [...expectedNames]
    .filter((name) => !existingNames.has(name))
    .sort();
  writeFileSync(options.output, missingNames.length > 0 ? `${missingNames.join("\n")}\n` : "");
  console.log(
    `Draft asset plan: reuse ${existingNames.size}, upload ${missingNames.length}, overwrite 0.`,
  );
}

function validatePlatformBundle(platformId, expectedVersion, expectedArchitecture, bundleRoot, files) {
  if (platformId.startsWith("darwin-")) {
    const plistFiles = files.filter((file) => file.endsWith(".app/Contents/Info.plist"));
    if (plistFiles.length !== 1) {
      fail(`${platformId} must contain exactly one application Info.plist; found ${plistFiles.length}.`);
    }
    for (const key of ["CFBundleShortVersionString", "CFBundleVersion"]) {
      const actualVersion = execFileSync(
        "/usr/bin/plutil",
        ["-extract", key, "raw", "-o", "-", plistFiles[0]],
        { encoding: "utf8" },
      ).trim();
      assertVersion(expectedVersion, actualVersion, `${platformId} ${key}`);
    }
    const executableName = execFileSync(
      "/usr/bin/plutil",
      ["-extract", "CFBundleExecutable", "raw", "-o", "-", plistFiles[0]],
      { encoding: "utf8" },
    ).trim();
    const appRoot = dirname(dirname(plistFiles[0]));
    const executable = join(appRoot, "Contents", "MacOS", executableName);
    const architectures = execFileSync("/usr/bin/lipo", ["-archs", executable], {
      encoding: "utf8",
    }).trim().split(/\s+/).filter(Boolean);
    if (!sameValues(architectures, [expectedArchitecture])) {
      fail(`${platformId} app architecture must be ${expectedArchitecture}; found ${architectures.join(",")}.`);
    }
    return;
  }

  const msi = exactlyOne(files, (file) => file.endsWith(".msi") && matchesBundleKind(file, "msi"), "MSI");
  const nsis = exactlyOne(files, (file) => file.endsWith(".exe") && matchesBundleKind(file, "nsis"), "NSIS");
  assertVersion(expectedVersion, readMsiVersion(msi), `${platformId} MSI ProductVersion`);
  assertVersion(expectedVersion, readExeVersion(nsis), `${platformId} NSIS ProductVersion`);
  const template = readMsiTemplate(msi).toLowerCase();
  const expectedTemplate = expectedArchitecture === "x86_64" ? "x64" : "intel";
  if (!template.split(";").map((value) => value.trim()).includes(expectedTemplate)) {
    fail(`${platformId} MSI architecture must be ${expectedTemplate}; found ${template}.`);
  }
  validatePeArchitecture(join(dirname(bundleRoot), "liberty.exe"), expectedArchitecture);
}

function readMsiVersion(file) {
  const script = [
    "$ErrorActionPreference = 'Stop'",
    "$installer = New-Object -ComObject WindowsInstaller.Installer",
    "$database = $installer.GetType().InvokeMember('OpenDatabase', 'InvokeMethod', $null, $installer, @($env:LIBERTY_RELEASE_ASSET_PATH, 0))",
    "$view = $database.GetType().InvokeMember('OpenView', 'InvokeMethod', $null, $database, @(\"SELECT `Value` FROM `Property` WHERE `Property` = 'ProductVersion'\"))",
    "$view.GetType().InvokeMember('Execute', 'InvokeMethod', $null, $view, $null) | Out-Null",
    "$record = $view.GetType().InvokeMember('Fetch', 'InvokeMethod', $null, $view, $null)",
    "$record.GetType().InvokeMember('StringData', 'GetProperty', $null, $record, 1)",
  ].join("\n");
  return runPowerShellWithAsset(script, file);
}

function readMsiTemplate(file) {
  const script = [
    "$ErrorActionPreference = 'Stop'",
    "$installer = New-Object -ComObject WindowsInstaller.Installer",
    "$database = $installer.GetType().InvokeMember('OpenDatabase', 'InvokeMethod', $null, $installer, @($env:LIBERTY_RELEASE_ASSET_PATH, 0))",
    "$summary = $database.GetType().InvokeMember('SummaryInformation', 'GetProperty', $null, $database, @(0))",
    "$summary.GetType().InvokeMember('Property', 'GetProperty', $null, $summary, 7)",
  ].join("\n");
  return runPowerShellWithAsset(script, file);
}

function validatePeArchitecture(file, expectedArchitecture) {
  const descriptor = openSync(file, "r");
  try {
    const dosHeader = Buffer.alloc(64);
    if (readSync(descriptor, dosHeader, 0, dosHeader.length, 0) !== dosHeader.length
        || dosHeader.toString("ascii", 0, 2) !== "MZ") {
      fail(`${file} is not a valid PE executable.`);
    }
    const peOffset = dosHeader.readUInt32LE(0x3c);
    const peHeader = Buffer.alloc(6);
    if (readSync(descriptor, peHeader, 0, peHeader.length, peOffset) !== peHeader.length
        || peHeader.toString("ascii", 0, 4) !== "PE\0\0") {
      fail(`${file} has an invalid PE header.`);
    }
    const actualArchitecture = new Map([
      [0x014c, "x86"],
      [0x8664, "x86_64"],
      [0xaa64, "arm64"],
    ]).get(peHeader.readUInt16LE(4));
    if (actualArchitecture !== expectedArchitecture) {
      fail(`${file} architecture must be ${expectedArchitecture}; found ${actualArchitecture ?? "unknown"}.`);
    }
  } finally {
    closeSync(descriptor);
  }
}

function readExeVersion(file) {
  return runPowerShellWithAsset(
    "(Get-Item -LiteralPath $env:LIBERTY_RELEASE_ASSET_PATH).VersionInfo.ProductVersion",
    file,
  );
}

function validateManifest(manifest, expectedVersion) {
  const platform = platformMatrix.get(manifest.platformId);
  if (manifest.schemaVersion !== 1 || !platform) {
    fail(`Invalid platform manifest for ${manifest.platformId ?? "unknown"}.`);
  }
  if (manifest.version !== expectedVersion) {
    fail(`${manifest.platformId} manifest version ${manifest.version} does not match ${expectedVersion}.`);
  }
  if (!Array.isArray(manifest.assets) || !sameValues(manifest.assets.map(({ kind }) => kind), platform.kinds)) {
    fail(`${manifest.platformId} manifest has an invalid asset set.`);
  }
  for (const asset of manifest.assets) {
    validateAssetMetadata(asset, manifest.platformId, platform.architecture);
    const expectedExtension = kindExtensions.get(asset.kind);
    if (extname(asset.name).toLowerCase() !== expectedExtension) {
      fail(`${asset.name} does not match asset kind ${asset.kind}.`);
    }
  }
}

function validateReleaseManifest(manifest) {
  validateInstallerManifest(manifest);
  const sbomName = `liberty-${manifest.version}.cdx.json`;
  const sbom = manifest.assets.find(({ name }) => name === sbomName);
  if (!sbom || sbom.kind !== "sbom" || sbom.platformId !== "all") {
    fail(`Release manifest is missing ${sbomName}.`);
  }
  validateAssetMetadata(sbom, "SBOM", "universal");
  if (manifest.assets.length !== installerAssets(manifest.version).length + 1) {
    fail("Release manifest must contain only the platform installers and one SBOM.");
  }
}

function validateInstallerManifest(manifest) {
  if (manifest.schemaVersion !== 1 || manifest.tag !== `v${manifest.version}`) {
    fail("Release manifest tag and version are invalid.");
  }

  const expectedAssets = installerAssets(manifest.version);
  const installers = manifest.assets.filter(({ kind }) => kind !== "sbom");
  if (!Array.isArray(manifest.assets) || installers.length !== expectedAssets.length) {
    fail(`Release manifest must contain exactly ${expectedAssets.length} installer assets.`);
  }

  for (const expected of expectedAssets) {
    const asset = installers.find(({ name }) => name === expected.name);
    if (!asset || asset.kind !== expected.kind || asset.platformId !== expected.platformId) {
      fail(`Release manifest is missing ${expected.name} with the expected platform and kind.`);
    }
    validateAssetMetadata(asset, expected.platformId, expected.architecture);
  }
}

function installerAssets(version) {
  return [...platformMatrix].flatMap(([platformId, platform]) =>
    platform.kinds.map((kind) => ({
      name: `liberty-${version}-${platformId}.${kind === "nsis" ? "exe" : kind}`,
      kind,
      platformId,
      architecture: platform.architecture,
    })),
  );
}

function validateAssetMetadata(asset, context, expectedArchitecture) {
  if (
    typeof asset.name !== "string"
    || basename(asset.name) !== asset.name
    || !/^[A-Za-z0-9._-]+$/.test(asset.name)
    || !/^[a-f0-9]{64}$/.test(asset.sha256)
    || !Number.isSafeInteger(asset.size)
    || asset.size <= 0
    || asset.architecture !== expectedArchitecture
  ) {
    fail(`${context} contains invalid metadata for ${asset.name ?? "an unnamed asset"}.`);
  }
}

async function verifyAsset(file, expected) {
  const actual = await describeAsset(file, expected.kind);
  if (actual.name !== expected.name || actual.size !== expected.size || actual.sha256 !== expected.sha256) {
    fail(`Asset integrity mismatch for ${expected.name}.`);
  }
}

function verifyChecksumFile(file, assets) {
  const expectedLines = assets
    .map(({ name, sha256 }) => `${sha256}  ${name}`)
    .sort();
  const actualLines = readFileSync(file, "utf8")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .sort();
  if (!sameValues(actualLines, expectedLines)) {
    fail("SHA256SUMS.txt does not match the validated release manifest.");
  }
}

async function describeAsset(file, kind, architecture) {
  return {
    name: basename(file),
    kind,
    ...(architecture ? { architecture } : {}),
    size: statSync(file).size,
    sha256: await hashFile(file),
  };
}

async function hashFile(file) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(file, { highWaterMark: 1024 * 1024 })) {
    hash.update(chunk);
  }
  return hash.digest("hex");
}

function walkFiles(directory) {
  const entries = readdirSync(directory, { withFileTypes: true });
  return entries.flatMap((entry) => {
    const entryPath = join(directory, entry.name);
    if (entry.isDirectory()) {
      return walkFiles(entryPath);
    }
    return entry.isFile() ? [entryPath] : [];
  });
}

function matchesBundleKind(file, kind) {
  const normalized = file.replaceAll("\\", "/");
  return normalized.includes(`/bundle/${kind}/`);
}

function exactlyOne(values, predicate, label) {
  const matches = values.filter(predicate);
  if (matches.length !== 1) {
    fail(`Expected exactly one ${label} file; found ${matches.length}.`);
  }
  return matches[0];
}

function assertVersion(expected, actual, source) {
  if (actual !== expected && actual !== `${expected}.0`) {
    fail(`${source} is ${actual}; expected ${expected}.`);
  }
}

function sameValues(actual, expected) {
  return actual.length === expected.length
    && [...actual].sort().every((value, index) => value === [...expected].sort()[index]);
}

function containsAbsolutePath(value) {
  if (typeof value === "string") return resolve(value) === value || /^[A-Za-z]:[\\/]/.test(value);
  if (Array.isArray(value)) return value.some(containsAbsolutePath);
  return value && typeof value === "object" && Object.values(value).some(containsAbsolutePath);
}

function parseOptions(values) {
  const options = {};
  for (let index = 0; index < values.length; index += 2) {
    const name = values[index];
    if (!name?.startsWith("--")) {
      fail(`Invalid option: ${name ?? "missing"}.`);
    }
    const key = name.slice(2);
    if (key === "allow-checksums" || key === "report-memory") {
      options[key] = "true";
      index -= 1;
      continue;
    }
    const value = values[index + 1];
    if (!value || value.startsWith("--")) {
      fail(`${name} requires a value.`);
    }
    if (options[key] !== undefined) {
      fail(`Duplicate option: ${name}.`);
    }
    options[key] = value;
  }
  return options;
}

function requireOptions(options, names) {
  for (const name of names) {
    if (!options[name]) {
      fail(`Missing required option: --${name}.`);
    }
  }
}

function fail(message) {
  console.error(`- ${message}`);
  process.exit(1);
}
