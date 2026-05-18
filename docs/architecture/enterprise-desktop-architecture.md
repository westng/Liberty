# Liberty Enterprise Desktop Architecture

## Scope

Liberty is a local-first desktop application for meeting media processing. The enterprise target is a maintainable, secure, testable desktop product that supports:

- macOS Apple Silicon: `aarch64-apple-darwin`, platform id `darwin-aarch64`
- macOS Intel: `x86_64-apple-darwin`, platform id `darwin-x64`
- Windows x64: `x86_64-pc-windows-msvc`, platform id `windows-x64`
- Windows x86: `i686-pc-windows-msvc`, platform id `windows-x86`

Windows x86 is a separately validated platform because Python runtime availability, memory pressure, and model performance differ from Windows x64.

## Target Layers

### Frontend

- `app`: application shell, router, bootstrapping, global providers.
- `features`: user-facing feature modules. Views should coordinate UI state, not own long-running workflows.
- `shared`: reusable components, typed clients, common services, i18n, styles, and types.
- `features/*/application`: frontend use cases that orchestrate stores, Tauri clients, and remote clients.

### Tauri Backend

- `commands`: Tauri command adapters. They validate transport inputs and call application services.
- `application`: use cases such as creating jobs, retrying jobs, installing runtime bundles, and generating summaries.
- `domain`: entities, value objects, state machines, policies, and typed errors.
- `infrastructure`: SQLite repositories, file system access, Python runner integration, HTTP AI clients, OS services, and platform-specific implementations.

Command modules must not directly own SQL, child-process orchestration, or file-system policy. Infrastructure modules must not depend on Tauri window state unless the integration is explicitly OS-bound.

## Shared Code Extraction

Common helpers live under `infrastructure` when they touch runtime concerns such
as time, IDs, persistence, credentials, or platform-specific services. Feature
modules should call these helpers instead of redefining timestamp generation,
schema migration checks, or repository writes.

The second implementation pass starts this extraction with:

- shared timestamp and ID helpers
- reusable SQLite `ADD COLUMN IF MISSING` helper
- `job_events` repository
- Runner command construction isolated from job orchestration
- frontend diagnostics use case extracted out of the settings view

The third implementation pass continues the boundary split:

- AI summary-run persistence moved into `infrastructure/repositories/ai_summary_runs.rs`
- desktop-pet persistence moved into `infrastructure/repositories/pet.rs`
- Python runner process concerns moved into `infrastructure/runner_process.rs`
- frontend polling and job-snapshot merge rules moved into `features/meeting/application`
- `useMeetingStore` now coordinates use cases instead of owning polling mechanics

The fourth refinement pass reduces remaining large orchestration files:

- AI command adapters stay in `local_ai.rs`, while HTTP request policy, prompt
  construction, and response normalization live in `local_ai/*`
- managed-runtime commands stay in `local_runtime.rs`, while manifest loading,
  archive extraction, resource path resolution, logging, and process streaming
  live in `local_runtime/*`
- settings page state and runtime-panel calculations live in
  `features/settings/application`, leaving `SettingsView.vue` focused on layout

## Data And Migration

- SQLite schema changes are versioned migrations, not ad hoc `ALTER TABLE` statements.
- `app_meta.schema_version` is the source of truth for the local database version.
- Repository modules own SQL and expose typed operations to application services.
- Migrations must be idempotent at the migration runner level and covered by upgrade tests.

## Job System

The enterprise job runner owns all long-running local work:

- queue state: `pending`, `running`, `completed`, `failed`, `cancelled`
- controls: create, retry, cancel, resume
- concurrency: controlled by user settings and platform limits
- persistence: job snapshots plus structured `job_events`
- process protocol: Python runner emits JSON lines for progress, result, and error events

Text logs are a user-facing projection of structured events, not the only source of operational truth.

## Security Baseline

- Tauri CSP is enabled for production builds.
- Capabilities are split by window role:
  - `main`: orchestration, dialogs, exports, and child-window creation
  - `ai-summary` / `meeting-notes`: read-only result windows
  - `model-editor` / `template-editor` / `member-editor`: edit windows
- File-system permissions use explicit scopes.
- API keys and remote tokens live in the system credential store:
  - macOS: Keychain
  - Windows: Credential Manager
- Logs and diagnostics redact tokens, API keys, and unnecessary local path details.

The implementation enables CSP, explicit file-system scopes, and per-window
capability files. File-system writes and child-window creation stay limited to
the main window; result and editor windows only receive the native window
permissions they actually use.

AI model records now store an `api_key_ref` and keep API keys in the system
credential store. Model listing hydrates secrets through the credential store,
save/update writes non-empty keys to the OS store, and deletion removes the
referenced secret. This keeps the public model configuration contract stable
without persisting plaintext API keys in SQLite.

## Quality Gates

Every pull request and release build must run:

- TypeScript type checking
- frontend production build
- Rust formatting check
- Rust tests
- Rust clippy with warnings denied
- release version consistency checks

Release workflows run after quality workflows pass.

## Release Matrix

Each supported platform gets:

- independent runtime bundle
- runtime manifest and checksum validation
- platform-specific smoke validation
- separate release artifact naming
- signing and notarization strategy where applicable

macOS artifacts must distinguish Apple Silicon and Intel builds. Windows x86 and x64 must never share runtime bundle assumptions.
