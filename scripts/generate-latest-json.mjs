import fs from "node:fs";
import path from "node:path";

const [, , releaseDirArg] = process.argv;
const releaseDir = path.resolve(releaseDirArg || "release-upload");
const releaseTag = process.env.RELEASE_TAG?.trim();
const notesPath = process.env.RELEASE_NOTES_PATH?.trim() || "release-notes.md";
const repo = process.env.GITHUB_REPOSITORY?.trim() || "westng/Liberty";

if (!releaseTag) {
  throw new Error("RELEASE_TAG is required.");
}

const notes = fs.existsSync(notesPath) ? fs.readFileSync(notesPath, "utf8").trim() : "";
const files = fs.readdirSync(releaseDir).sort();
const platforms = {};

for (const fileName of files) {
  const fullPath = path.join(releaseDir, fileName);
  const stat = fs.statSync(fullPath);
  if (!stat.isFile()) {
    continue;
  }

  if (!fileName.endsWith(".exe") && !fileName.endsWith(".msi")) {
    continue;
  }

  const signaturePath = `${fullPath}.sig`;
  if (!fs.existsSync(signaturePath)) {
    throw new Error(`Missing signature file for ${fileName}`);
  }

  const signature = fs.readFileSync(signaturePath, "utf8").trim();
  const target = fileName.endsWith(".exe") ? "windows-x86_64-nsis" : "windows-x86_64-msi";
  platforms[target] = {
    signature,
    url: `https://github.com/${repo}/releases/latest/download/${encodeURIComponent(fileName)}`,
  };
}

if (Object.keys(platforms).length === 0) {
  throw new Error(`No Windows updater assets found in ${releaseDir}`);
}

const manifest = {
  version: releaseTag.replace(/^v/, ""),
  notes,
  pub_date: new Date().toISOString(),
  platforms,
};

fs.writeFileSync(
  path.join(releaseDir, "latest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);

console.log("[generate-latest-json] wrote latest.json");
