# Liberty Managed Local Runtime Bootstrap Design

## Overview

Liberty currently depends on a user-provided local Python path for desktop
transcription. This works for development and advanced users, but it does not
match the expected behavior of a consumer desktop application. The current
setup forces users to understand Python versions, executable paths, missing
dependencies, and model installation before they can process a meeting.

Liberty will move to a managed local runtime model. The application will own a
desktop runtime directory under its local app data folder and will install the
required local stack on first use:

- Python 3.9 runtime
- pinned Python package dependencies
- default FunASR models
- Liberty runner integration metadata

The install flow will download platform-specific resources from domestic
mirrors after app installation. The bundled desktop app will ship the installer
logic and runtime manifest, but it will not embed the full Python runtime and
models inside the installer package.

## Goals

- Remove the default requirement for users to manually configure a Python path
- Make desktop local transcription usable after a guided first-run install
- Keep one consistent runtime experience across macOS and Windows
- Support domestic download sources for runtime packages and model artifacts
- Preserve manual Python path override as an advanced fallback
- Surface runtime install state, progress, and repair entry points in the UI
- Make local task execution prefer the managed runtime automatically

## Non-Goals

- Linux runtime support in this phase
- Fully offline installation in this phase
- Packaging the Python runtime and models inside the app installer
- GPU-specific model packs or optional model families in the first milestone
- Background silent updates of the runtime without user action
- Multi-version runtime coexistence in the first milestone

## Option Summary

1. Managed runtime owned by Liberty and installed on first run
   - Recommended
   - Best desktop UX and least user confusion
   - Requires installer flow, runtime state persistence, and download logic
2. Keep manual `pythonPath` as the primary setup and only add helper tooling
   - Smaller code change
   - Still leaves users exposed to environment problems
   - Not recommended
3. Ship Python 3.9 and models directly inside the desktop installer
   - Simplest runtime behavior after install
   - Very large installers and heavier CI/release cost
   - Not recommended for this phase

## Selected Approach

Liberty will introduce a managed local runtime subsystem. On startup, the app
checks whether the managed runtime is complete and usable for the current
platform. If it is missing, outdated, or broken, the app exposes an install or
repair flow in system settings and any local-processing entry points.

The managed runtime becomes the default execution target for local jobs.
Manually configured `pythonPath` remains available as an advanced override and
recovery path, but it is no longer the primary user journey.

The app installer remains small. The desktop application ships:

- a runtime manifest describing downloadable assets
- installer orchestration logic
- platform-specific extraction and validation logic
- the existing Liberty Python runner script

The app does not ship:

- the full embedded Python runtime archive
- preinstalled FunASR models
- the full pip environment

## Runtime Architecture

### Runtime Root

The managed runtime should live under the app local data directory, for example:

- macOS: `<app_local_data_dir>/runtime/darwin-aarch64` or
  `<app_local_data_dir>/runtime/darwin-x64`
- Windows: `<app_local_data_dir>/runtime/windows-x64`

The runtime root contains:

- downloaded archives or extracted directories
- the managed Python executable
- installed Python dependencies
- model directories
- a Liberty-managed manifest file recording runtime version and install state
- install logs and temporary download files

### Runtime State Model

Rust should own a persisted runtime state record, stored in SQLite or adjacent
runtime metadata. Recommended fields:

- `platform_id`
- `runtime_version`
- `status`
  - `missing`
  - `installing`
  - `ready`
  - `failed`
  - `repair_required`
- `python_executable_path`
- `models_root`
- `last_error`
- `installed_at`
- `updated_at`

This state is consumed by the settings UI and by local job execution.

### Runtime Manifest

Liberty should include a versioned manifest describing required assets per
platform. Each platform entry should define:

- Python 3.9 runtime archive URL
- dependency bundle URL or install recipe
- default model URLs
- expected checksums
- extraction layout
- executable relative path

The manifest should be versioned with the app so the runtime can be upgraded
later without inventing a second protocol.

## Download and Install Flow

### Trigger Points

The install flow should be reachable from:

- system settings: primary runtime management entry
- new task screen: when local mode is selected but runtime is missing
- local job retry: when a failed job depends on a missing runtime

Startup should not automatically begin downloading without user action. Startup
should only perform detection and show a clear install-required state.

### Install Sequence

Recommended install sequence:

1. Detect current platform and architecture
2. Load bundled runtime manifest
3. Create runtime workspace directories
4. Download Python runtime archive from domestic mirror
5. Verify checksum
6. Extract runtime
7. Install Python dependencies into the managed runtime
8. Download default FunASR model assets from domestic mirror
9. Verify checksums
10. Write runtime metadata and mark status as `ready`

If any step fails, the runtime state moves to `failed` with full diagnostic log
content retained.

### Dependency Installation Strategy

The runtime installation should avoid depending on a system Python. The managed
runtime should bootstrap its own package installation.

Preferred strategy:

- ship a pinned requirements lock file or wheel manifest in the repo
- invoke the managed Python executable directly to install into its own
  environment
- prefer wheel-based downloads where possible for repeatable installation

This keeps installation logic consistent across platforms and reduces drift.

### Model Installation Strategy

The first milestone should install one default model set that covers the
current Liberty local workflow. That means the runtime installer owns the
download of:

- ASR model
- VAD model
- punctuation model
- speaker model required by the current speaker separation flow

The installer should record model installation success independently from the
Python runtime so future repair flows can target only broken models.

## Runtime Resolution in Local Jobs

### Execution Priority

Local job execution should resolve Python in this order:

1. managed Liberty runtime if status is `ready`
2. manually configured `pythonPath` if present
3. fail with a clear runtime-not-installed error

This preserves advanced recovery options without keeping manual path setup as
the primary desktop workflow.

### Runner Integration

The existing local job pipeline in Rust should be updated to:

- query managed runtime state before spawning Python
- derive the Python path from managed runtime metadata
- record in process logs whether the job used the managed runtime or manual
  fallback

The existing runner script can remain in the app bundle, but it must be able to
locate the managed model directory and any runtime-specific environment values.

## UI Design

### Settings Page

System settings should gain a dedicated `本地运行环境` section with:

- current runtime status
- installed runtime version if ready
- managed Python version if ready
- install button when missing
- repair or reinstall button when failed or outdated
- log viewer or latest error summary

This section should read as a first-class product surface, not a low-level dev
panel.

### New Task and Failure Recovery

When the runtime is missing:

- local mode should remain visible
- the app should explain that the local runtime is not yet installed
- the user should get a direct jump to install

When a local job fails because the runtime is broken:

- job failure reason should point to runtime repair
- settings should expose the last install error and a retry path

## Domestic Download Sources

The runtime manifest should use domestic mirrors for:

- Python runtime archives
- dependency wheels or dependency package bundles
- FunASR model artifacts

The mirror configuration should be centralized and versioned. Hardcoding
download URLs across multiple files is not acceptable because updates become
fragile.

The app should also support future source switching if a mirror becomes
unavailable. That means runtime install code should depend on a structured
source manifest, not fixed inline URLs.

## Error Handling

The runtime installer must treat failure visibility as a first-class concern.

Failures to handle clearly:

- unsupported platform or architecture
- download timeout or DNS failure
- checksum mismatch
- extraction failure
- dependency install failure
- model download failure
- managed runtime executable missing after install
- runtime detected but incompatible with Liberty expectations

For each failure:

- write detailed install log output
- store a concise human-readable error in runtime state
- keep partial state isolated so a retry can safely clean it up

## Data and Persistence Changes

The design requires a new runtime state persistence layer in Tauri. This can be
implemented as either:

- SQLite tables owned by `local_db.rs`
- a small managed-runtime metadata file plus lightweight DB pointers

Recommendation:

- keep durable runtime state in SQLite for query consistency with the rest of
  the local desktop system
- store verbose logs and temporary files on disk under the runtime root

## Security and Integrity

The first milestone should at minimum validate checksums for every downloaded
archive or model artifact.

The installer should never execute a downloaded script directly. It should only
run:

- the managed Python executable after checksum verification
- Liberty-owned installer commands

Secrets are not required for public domestic mirrors in this phase.

## Testing Strategy

### Rust

- runtime status detection unit tests
- manifest parsing tests
- runtime path resolution tests
- local job Python selection tests
- install state transition tests

### Frontend

- settings page status rendering tests
- missing-runtime CTA behavior
- local-mode create-job blocking behavior when runtime is unavailable

### End-to-End

- clean install on macOS Apple Silicon
- clean install on macOS Intel
- clean install on Windows x64
- repair flow after intentionally corrupted runtime files
- local task execution using managed runtime
- fallback execution using manual `pythonPath`

## Rollout Plan

### Phase 1

- add runtime manifest and managed runtime state model
- add settings UI for detection and install
- add install orchestration for macOS and Windows
- switch local jobs to managed runtime priority

### Phase 2

- add repair and partial redownload flows
- add runtime version upgrade handling
- improve progress UI and install log viewer

### Phase 3

- optional model set management
- optional mirror switching in settings

## Open Questions

- Which exact domestic mirrors should host Python runtime archives and model
  artifacts for each platform
- Whether dependency installation should rely on wheel bundles checked into a
  release bucket versus live mirror resolution
- Whether runtime state belongs fully in SQLite or partially in file metadata

## Recommendation

Proceed with the managed runtime bootstrap design and keep manual `pythonPath`
only as an advanced override. This gives Liberty a desktop-native local setup
flow while keeping installer size controlled and avoiding repeated environment
support issues.
