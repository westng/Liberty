import process from "node:process";

const args = parseArgs(process.argv.slice(2));
const token = process.env.GH_TOKEN ?? process.env.GITHUB_TOKEN;
const apiUrl = (process.env.GITHUB_API_URL ?? "https://api.github.com").replace(/\/$/, "");
const errors = [];

if (!/^[^/]+\/[^/]+$/.test(args.repository)) {
  errors.push(`Repository must use owner/name form. Found ${args.repository}.`);
}
if (!/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(args.tag)) {
  errors.push(`Release tag must use the strict v<semver> form. Found ${args.tag}.`);
}
if (!/^[0-9a-f]{40}$/i.test(args.expectedCommit)) {
  errors.push(`Expected commit must be a full 40-character SHA. Found ${args.expectedCommit}.`);
}

if (errors.length > 0) {
  fail(errors);
}

const tagRef = await request(`/repos/${args.repository}/git/ref/tags/${encodeURIComponent(args.tag)}`);
if (!tagRef) {
  if (!args.allowAbsentTag) {
    fail([`Immutable tag ${args.tag} does not exist in ${args.repository}.`]);
  }
}

let target = tagRef?.object;
const visitedTags = new Set();
while (target?.type === "tag") {
  if (visitedTags.has(target.sha) || visitedTags.size >= 10) {
    fail([`Tag ${args.tag} could not be resolved safely to a commit.`]);
  }
  visitedTags.add(target.sha);
  const annotatedTag = await request(`/repos/${args.repository}/git/tags/${target.sha}`);
  if (!annotatedTag) {
    fail([`Annotated tag object ${target.sha} no longer exists.`]);
  }
  target = annotatedTag.object;
}

if (tagRef && target?.type !== "commit") {
  fail([`Tag ${args.tag} resolves to ${target?.type ?? "nothing"}, not a commit.`]);
}
if (tagRef && target.sha.toLowerCase() !== args.expectedCommit.toLowerCase()) {
  fail([
    `Immutable tag ${args.tag} resolves to ${target.sha}, expected ${args.expectedCommit}.`,
    "Refusing to move or replace the tag.",
  ]);
}

let release;
if (args.requireAbsentRelease || args.allowResumableDraft || args.requiredDraftReleaseId) {
  const releases = await listReleases(args.repository);
  const matchingReleases = releases.filter((release) => release.tag_name === args.tag);
  const directRelease = args.requireAbsentRelease
    ? await request(`/repos/${args.repository}/releases/tags/${encodeURIComponent(args.tag)}`)
    : undefined;
  release = matchingReleases[0] ?? directRelease;

  if (matchingReleases.length > 1) {
    fail([`Expected at most one Release for ${args.tag}; found ${matchingReleases.length}.`]);
  }
  if (!tagRef && release) {
    fail([`Release ${args.tag} exists without its immutable tag; refusing to adopt it.`]);
  }

  if (args.requireAbsentRelease && release) {
    const assets = release.assets ?? [];
    fail([
      `Release ${args.tag} already exists (id=${release.id}, draft=${Boolean(release.draft)}).`,
      assets.length > 0
        ? `It already contains ${assets.length} asset(s): ${assets.map((asset) => asset.name).join(", ")}.`
        : "Refusing to delete or replace the existing Release.",
    ]);
  }

  if (args.requiredDraftReleaseId) {
    if (matchingReleases.length !== 1) {
      fail([`Expected exactly one draft Release for ${args.tag}; found ${matchingReleases.length}.`]);
    }
    if (String(release.id) !== args.requiredDraftReleaseId || release.draft !== true) {
      fail([
        `Release ${args.tag} must remain draft id ${args.requiredDraftReleaseId}.`,
        `Found id=${release.id}, draft=${Boolean(release.draft)}.`,
      ]);
    }
    assertResumableDraft(release);
  }

  if (args.allowResumableDraft && release) {
    assertResumableDraft(release);
  }

  if (args.printReleaseId && release) {
    process.stdout.write(String(release.id));
  }
}

if (!args.printReleaseId) {
  console.log(
    !tagRef
      ? `Release tag ${args.tag} is available and the Release slot is unused.`
      : args.requiredDraftReleaseId
      ? `Immutable tag ${args.tag} still resolves to ${target.sha}; draft Release ${args.requiredDraftReleaseId} is intact.`
      : args.allowResumableDraft && release
        ? `Immutable tag ${args.tag} resolves to ${target.sha}; draft Release ${release.id} can be resumed without overwrites.`
        : `Immutable tag ${args.tag} resolves to ${target.sha}; the Release slot is unused.`,
  );
}

function assertResumableDraft(release) {
  const marker = `<!-- liberty-release-commit:${args.expectedCommit.toLowerCase()} -->`;
  if (release.draft !== true) {
    fail([`Release ${args.tag} already exists and is public; refusing to replace it.`]);
  }
  if (release.name !== `Liberty ${args.tag}` || !(release.body ?? "").includes(marker)) {
    fail([
      `Draft Release ${args.tag} was not created for commit ${args.expectedCommit}.`,
      "Refusing to adopt, edit, or delete the existing draft.",
    ]);
  }
}

async function listReleases(repository) {
  const releases = [];
  for (let page = 1; page <= 100; page += 1) {
    const batch = await request(`/repos/${repository}/releases?per_page=100&page=${page}`);
    if (!Array.isArray(batch)) {
      fail(["GitHub returned an invalid releases response."]);
    }
    releases.push(...batch);
    if (batch.length < 100) {
      break;
    }
  }
  return releases;
}

async function request(path) {
  const headers = {
    Accept: "application/vnd.github+json",
    "User-Agent": "liberty-release-check",
    "X-GitHub-Api-Version": "2022-11-28",
  };
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  let response;
  try {
    response = await fetch(`${apiUrl}${path}`, { headers });
  } catch (error) {
    fail([`GitHub API request failed: ${error.message}`]);
  }

  if (response.status === 404) {
    return undefined;
  }
  if (!response.ok) {
    const body = await response.text();
    fail([`GitHub API ${response.status} for ${path}: ${body.slice(0, 500)}`]);
  }
  return response.json();
}

function parseArgs(values) {
  const parsed = {
    repository: undefined,
    tag: undefined,
    expectedCommit: undefined,
    requireAbsentRelease: false,
    allowResumableDraft: false,
    printReleaseId: false,
    requiredDraftReleaseId: undefined,
    allowAbsentTag: false,
  };

  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === "--require-absent-release") {
      parsed.requireAbsentRelease = true;
      continue;
    }
    if (value === "--allow-resumable-draft") {
      parsed.allowResumableDraft = true;
      continue;
    }
    if (value === "--print-release-id") {
      parsed.printReleaseId = true;
      continue;
    }
    if (value === "--allow-absent-tag") {
      parsed.allowAbsentTag = true;
      continue;
    }

    const key = {
      "--repository": "repository",
      "--tag": "tag",
      "--expected-commit": "expectedCommit",
      "--require-draft-release-id": "requiredDraftReleaseId",
    }[value];
    if (!key || !values[index + 1]) {
      fail([key ? `${value} requires a value.` : `Unknown argument: ${value}`]);
    }
    parsed[key] = values[index + 1];
    index += 1;
  }

  for (const key of ["repository", "tag", "expectedCommit"]) {
    if (!parsed[key]) {
      fail([`Missing required argument: ${key}.`]);
    }
  }
  const releaseModes = [
    parsed.requireAbsentRelease,
    parsed.allowResumableDraft,
    Boolean(parsed.requiredDraftReleaseId),
  ].filter(Boolean);
  if (releaseModes.length > 1) {
    fail([
      "--require-absent-release, --allow-resumable-draft, and --require-draft-release-id are mutually exclusive.",
    ]);
  }
  if (parsed.printReleaseId && !parsed.allowResumableDraft) {
    fail(["--print-release-id requires --allow-resumable-draft."]);
  }
  if (parsed.allowAbsentTag && parsed.requiredDraftReleaseId) {
    fail(["--allow-absent-tag cannot be used with --require-draft-release-id."]);
  }
  return parsed;
}

function fail(messages) {
  console.error(messages.map((message) => `- ${message}`).join("\n"));
  process.exit(1);
}
