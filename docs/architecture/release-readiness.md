# Release Readiness

Last verified: 2026-08-27 against commit `1563af7a` and the root `package.json` scripts.

## Required Platforms

Every release candidate must account for these desktop targets:

| Platform | Platform ID | Rust Target | Validation |
| --- | --- | --- | --- |
| macOS Apple Silicon | `darwin-aarch64` | `aarch64-apple-darwin` | primary |
| macOS Intel | `darwin-x64` | `x86_64-apple-darwin` | primary |
| Windows x64 | `windows-x64` | `x86_64-pc-windows-msvc` | primary |
| Windows x86 | `windows-x86` | `i686-pc-windows-msvc` | extended |

Windows x86 remains a supported target, but it must be validated independently
because runtime dependencies and model performance differ from x64.

## Automated Checks

Run before release:

```bash
pnpm check
```

This includes:

- generated-contract drift checks
- frontend type checking, unit tests, and production build
- Python Ruff and pytest
- Rust formatting check
- Rust tests
- Rust clippy with warnings denied
- release version, platform-matrix, Tauri security-baseline, and runtime-lock checks
- dependency-governance and ASR-validation blocking fixtures

`pnpm check` validates the governance code and its blocking fixtures, but it does
not replace a real dependency scan, controlled-media benchmark, native platform
smoke, signing, notarization, installer test, or published-asset verification.

## External Evidence Gates

Run the real scanners in an environment with the pinned prerequisites:

```bash
pnpm security:check
pnpm licenses:check
```

Scanner exit code `2` means the required tool is unavailable, not that the scan
passed. When the ASR engine, runtime, model, or supported-platform assumptions
change, also run the controlled fixture, benchmark, platform-smoke, and accepted
evidence workflow documented under `benchmarks/asr/`; missing media, annotations,
registered devices, or digest-bound evidence remains blocking.

## Manual Desktop Workflow Checks

- Start the native development or release candidate, confirm the Liberty process
  and native window, and avoid running a second instance against the same SQLite
  database. A Vite URL alone is not desktop acceptance evidence.
- Confirm the dashboard loads aggregate metrics, trends, completeness, recent
  jobs, and local resource state without loading every full job result.
- Confirm completed jobs open the scoped `job-workbench` directly, while active
  or failed jobs open details; legacy result routes must redirect to the
  completed-job queue.
- Confirm a workbench can access only its bound source and job, rejects an invalid
  or replaced scope token, and can request AI-summary or meeting-notes windows
  only for that same job. The main window remains the actual WebView creator.
- Confirm `completed`, `unavailable`, `failed`, and `legacy_unverified`
  diarization states remain distinct. Missing diarization must preserve the
  transcript and must not synthesize a speaker label in UI, AI prompts, or
  exports.
- Confirm Settings can independently display and operate Python, ffmpeg, and
  model resources, including source selection, progress, failure, repair, and
  validation states where supported.

## Manual Release Checks

- Confirm `runtime-manifest.json` has explicit entries for each supported platform ID.
- Confirm configured Python and ffmpeg asset URLs are reachable with a cheap HEAD or byte-range probe before publishing them to users.
- Confirm placeholder or unavailable runtime sources are not advertised in the client.
- Confirm macOS Intel and Apple Silicon artifacts are separate.
- Confirm Windows x86 and x64 artifacts do not share runtime assumptions.
- Confirm Windows x86 unsupported runtime-download messaging is still accurate when the platform remains listed.
- Confirm the Environment & Models settings flow can list configured download sources and surface runtime-install logs.
- Confirm diagnostics report shows the expected current platform and schema version.
- Confirm no API keys, tokens, or local source paths appear in exported diagnostics.
- Confirm desktop-pet diagnostic log export redacts sensitive paths and includes useful runtime/resource state.
