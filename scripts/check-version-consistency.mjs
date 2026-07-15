import { readFileSync, writeFileSync } from "node:fs";

const args = parseArgs(process.argv.slice(2));

const rootPackage = JSON.parse(readFileSync("package.json", "utf8"));
const desktopPackage = JSON.parse(readFileSync("apps/desktop/package.json", "utf8"));
const tauriConfig = JSON.parse(readFileSync("apps/desktop/src-tauri/tauri.conf.json", "utf8"));
const cargoToml = readFileSync("apps/desktop/src-tauri/Cargo.toml", "utf8");
const cargoVersion = cargoToml.match(/^version = "([^"]+)"$/m)?.[1];
const cargoLock = readFileSync("Cargo.lock", "utf8");
const cargoLockVersion = readCargoLockVersion(cargoLock, "liberty");
const semverPattern = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

const errors = [];

if (!cargoVersion) {
  errors.push("Unable to read version from apps/desktop/src-tauri/Cargo.toml.");
}

if (!cargoLockVersion) {
  errors.push("Unable to read the liberty package version from Cargo.lock.");
}

const versions = {
  rootPackage: rootPackage.version,
  desktopPackage: desktopPackage.version,
  tauriConfig: tauriConfig.version,
  cargoToml: cargoVersion,
  cargoLock: cargoLockVersion,
};
const releaseVersions = new Set([
  rootPackage.version,
  desktopPackage.version,
  tauriConfig.version,
  cargoVersion,
  cargoLockVersion,
].filter(Boolean));

if (releaseVersions.size !== 1) {
  errors.push(
    `Release versions must match. Found ${Object.entries(versions)
      .map(([source, version]) => `${source}=${version ?? "missing"}`)
      .join(", ")}.`,
  );
}

const releaseVersion = [...releaseVersions][0];
if (releaseVersion && !semverPattern.test(releaseVersion)) {
  errors.push(`Release version must be valid SemVer. Found ${releaseVersion}.`);
}

if (rootPackage.packageManager !== "pnpm@10.30.3") {
  errors.push("Root packageManager must remain pinned to pnpm@10.30.3.");
}

let changelogSection;
if (args.tag) {
  const expectedTag = releaseVersion ? `v${releaseVersion}` : undefined;
  if (args.tag !== expectedTag) {
    errors.push(`Release tag ${args.tag} must exactly match source version ${expectedTag ?? "unknown"}.`);
  }

  if (!/^v/.test(args.tag) || !semverPattern.test(args.tag.slice(1))) {
    errors.push(`Release tag must use the strict v<semver> form. Found ${args.tag}.`);
  }

  if (releaseVersion) {
    changelogSection = readChangelogSection(releaseVersion);
    if (!changelogSection) {
      errors.push(`CHANGELOG.md must contain a non-empty section for ${releaseVersion}.`);
    }
  }
}

if (errors.length > 0) {
  console.error(errors.map((error) => `- ${error}`).join("\n"));
  process.exit(1);
}

if (args.changelogOutput) {
  if (!args.tag) {
    console.error("- --changelog-output requires --tag.");
    process.exit(1);
  }
  writeFileSync(args.changelogOutput, `${changelogSection}\n`);
}

if (args.printVersion) {
  process.stdout.write(releaseVersion);
} else {
  console.log(
    args.tag
      ? `Version metadata, tag ${args.tag}, and changelog are consistent.`
      : "Version metadata is consistent.",
  );
}

function parseArgs(values) {
  const parsed = {
    tag: undefined,
    changelogOutput: undefined,
    printVersion: false,
  };

  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === "--print-version") {
      parsed.printVersion = true;
    } else if (value === "--tag" || value === "--changelog-output") {
      const nextValue = values[index + 1];
      if (!nextValue) {
        console.error(`- ${value} requires a value.`);
        process.exit(1);
      }
      parsed[value === "--tag" ? "tag" : "changelogOutput"] = nextValue;
      index += 1;
    } else {
      console.error(`- Unknown argument: ${value}`);
      process.exit(1);
    }
  }

  return parsed;
}

function readCargoLockVersion(contents, packageName) {
  const blocks = contents.split(/\n(?=\[\[package\]\]\n)/);
  const packageBlock = blocks.find((block) =>
    new RegExp(`^name = "${escapeRegExp(packageName)}"$`, "m").test(block),
  );
  return packageBlock?.match(/^version = "([^"]+)"$/m)?.[1];
}

function readChangelogSection(version) {
  const changelog = readFileSync("CHANGELOG.md", "utf8");
  const heading = new RegExp(
    `^##\\s+v?${escapeRegExp(version)}(?:\\s+-\\s+.*)?\\s*$`,
    "m",
  );
  const match = changelog.match(heading);
  if (!match || match.index === undefined) {
    return undefined;
  }

  const start = match.index;
  const contentAfterHeading = changelog.slice(start + match[0].length);
  const nextHeadingOffset = contentAfterHeading.search(/^##\s+/m);
  const end = nextHeadingOffset === -1
    ? changelog.length
    : start + match[0].length + nextHeadingOffset;
  const section = changelog.slice(start, end).trim();
  return section === match[0].trim() ? undefined : section;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
