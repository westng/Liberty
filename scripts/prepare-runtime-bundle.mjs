import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import process from "node:process";

if (process.env.LIBERTY_RUNTIME_BUNDLE_READY === "1") {
  console.log("[prepare-runtime-bundle] skip, bundle already prepared.");
  process.exit(0);
}

const platformId = resolvePlatformId();
const outputDir = `runtime-bundles/${platformId}`;
const ifMissingOnly = process.argv.includes("--if-missing");
const scriptPath = "scripts/prepare_runtime_bundle.py";
const expectedFiles = [
  `${outputDir}/python-runtime.tar.gz`,
  `${outputDir}/ffmpeg-runtime.tar.gz`,
];

if (ifMissingOnly && expectedFiles.every((filePath) => existsSync(filePath))) {
  console.log(`[prepare-runtime-bundle] skip, ${platformId} bundle already exists.`);
  process.exit(0);
}

const pythonLaunchers = process.platform === "win32"
  ? [
      { command: "py", args: ["-3"] },
      { command: "python", args: [] },
      { command: "python3", args: [] },
    ]
  : [
      { command: "python3", args: [] },
      { command: "python", args: [] },
    ];

const launcher = pythonLaunchers.find((candidate) => {
  const result = spawnSync(candidate.command, [...candidate.args, "--version"], {
    stdio: "ignore",
  });
  return result.status === 0;
});

if (!launcher) {
  console.error("[prepare-runtime-bundle] Python 3 launcher not found.");
  process.exit(1);
}

const result = spawnSync(
  launcher.command,
  [
    ...launcher.args,
    scriptPath,
    "--platform-id",
    platformId,
    "--output-dir",
    outputDir,
  ],
  {
    stdio: "inherit",
  },
);

if (typeof result.status === "number") {
  process.exit(result.status);
}

process.exit(1);

function resolvePlatformId() {
  if (process.platform === "darwin" && process.arch === "arm64") {
    return "darwin-aarch64";
  }

  if (process.platform === "darwin" && process.arch === "x64") {
    return "darwin-x64";
  }

  if (process.platform === "win32" && process.arch === "x64") {
    return "windows-x64";
  }

  console.error(
    `[prepare-runtime-bundle] unsupported build host: ${process.platform}/${process.arch}`,
  );
  process.exit(1);
}
