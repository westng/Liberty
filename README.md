<p align="center">
  <img src="https://avatars.githubusercontent.com/u/277389313?s=200&v=4" width="128" height="128" alt="Liberty">
</p>

<h1 align="center">Liberty</h1>

<p align="center">
  A desktop workspace for meeting media processing.
</p>

<p align="center">
  Local transcription · Speaker diarization · AI summarization · Result organization
</p>

<p align="center">
  <a href="apps/desktop/src-tauri/tauri.conf.json"><img src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white" alt="Tauri 2"></a>
  <a href="apps/desktop/package.json"><img src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=111111" alt="React 19"></a>
  <a href="apps/desktop/package.json"><img src="https://img.shields.io/badge/TypeScript-5-3178C6?logo=typescript&logoColor=white" alt="TypeScript 5"></a>
  <a href="apps/desktop/src-tauri/Cargo.toml"><img src="https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white" alt="Rust stable"></a>
  <a href="apps/desktop/src-tauri/resources/runtime-manifest.json"><img src="https://img.shields.io/badge/Python-3.10-3776AB?logo=python&logoColor=white" alt="Python 3.10"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-green.svg" alt="License"></a>
</p>

[English](./README.md) | [简体中文](./README.zh-CN.md)

Liberty is a local-first desktop application for meeting media processing. The current frontend uses React, the desktop shell uses Tauri 2, native services are implemented in Rust, and local transcription runs through a managed Python 3.10.17 runtime with a FunASR runner. It can process meetings fully through local SQLite and the managed runtime, while still keeping an optional remote backend mode.

The current workflow starts on the dashboard, continues through job creation or the job queue, and opens completed work in a dedicated result workbench for review, cleanup, and export.

## Current Capabilities

- Review job metrics, processing trends, result completeness, recent jobs, resources, and companion state on the dashboard.
- Create meeting jobs through the desktop file picker with local audio and video files.
- In local mode, process one local file with the managed Python 3.10.17 runtime, FunASR runner, and ffmpeg.
- Filter and search the job queue, then open details or a dedicated result workbench, retry jobs, and delete jobs according to their state.
- Track ASR and diarization separately. Only verified results enter the speaker projection; unavailable or failed diarization preserves the transcript without inventing speaker labels.
- Manage OpenAI-compatible models, summary templates, and meeting members as cards with dedicated editor windows.
- Use AI summary windows, repeated summary runs, and active summary selection.
- Review transcripts, filter by speaker, rename speakers, open meeting notes, and work from a dedicated result workspace.
- Export transcript TXT, notes Markdown, bundled Markdown, and formal DOCX meeting minutes.
- Manage meeting members with Excel import/export, department, sort order, and recorder metadata.
- Navigate Settings by appearance, theme, runtime, processing defaults, remote compatibility, and diagnostics, with separate controls for Python, ffmpeg, and model resources.
- Switch Chinese/English UI, automatic/light/dark theme, transparent/tinted glass style, and accent color.
- View diagnostics for platform matrix, database schema version, runtime status, security baseline, and exported desktop-pet diagnostic logs.
- Use the desktop companion, 255-level growth, LP wallet, Pet Store, inventory, daily free blind box, daily check-in, make-up check-in, gift boxes, redeem center, item detail window, and native desktop pet rendering.

## Runtime Modes

### Local Mode

When `backendUrl` is empty, the app uses local SQLite and local Tauri commands. If the user has not manually configured a Python path, the app automatically installs or repairs the managed runtime based on runtime status.

Local mode includes:

- Python, ffmpeg, download sources, and model endpoints described by `runtime-manifest.json`.
- The local transcription runner under `python/funasr-runner/`.
- Job creation, execution, retry, and log synchronization in `apps/desktop/src-tauri/src/local_jobs.rs`.
- Runtime installation, download-source selection, validation, warmup, and logs in `apps/desktop/src-tauri/src/local_runtime/`.
- SQLite storage for jobs, transcripts, AI summaries, members, settings, and pet data.

Current local job execution processes one file with a local path. In local mode, selecting multiple files keeps the last selected file.

The local concurrency setting is an upper bound, not a fixed worker count. The scheduler reserves 2 GiB for the operating system and desktop app, budgets 4 GiB per ASR runner, and divides logical CPU threads across the effective concurrency; an 8 GiB device runs one local job by default.

### Remote Mode

When `backendUrl` is set, the frontend uses `shared/services/remote/meetingApi.ts` to call the remote meeting API. Remote mode keeps job creation, listing, detail, and retry entry points, but the managed local runtime is not required before use.

### AI Summary Pipeline

AI summarization is not automatic after transcription. From the workbench, the user opens the AI summary window, chooses the model, template, speaker/timestamp options, generates a summary, and saves a summary run.

AI requests are sent by the Rust `local_ai` module to an OpenAI-compatible endpoint. Model API keys are stored through the system credential store: macOS Keychain or Windows Credential Manager.

### Pet Pipeline

The pet pipeline is a local companion system outside the core meeting flow. Current pet rules are centralized in the pet system notes: real work is the main growth source, LP is a local reward point, food grants fixed growth, and daily blind boxes, daily check-ins, make-up tickets, gift boxes, and redeem keys are local benefit or operations entry points. App startup tries to sync desktop pet state, but pet loading failure must not block the main window.

See [docs/pet-system.md](./docs/pet-system.md) for the current implementation notes.

## Technical Architecture

| Layer | Current implementation |
| --- | --- |
| Desktop shell | Tauri 2 |
| Frontend | React 19 + TypeScript + Vite |
| Routing | In-repo lightweight RouterContext |
| Native services | Rust, Tauri commands, SQLite, system credentials, DOCX/XLSX processing |
| Local transcription | Python 3.10.17 + FunASR runner + ffmpeg |
| Local storage | SQLite through bundled `rusqlite` |
| AI interface | OpenAI-compatible Chat Completions |
| Desktop pet rendering | macOS AppKit private API + Windows GDI/Win32 |

## Project Structure

```text
.
├─ apps/
│  └─ desktop/
│     ├─ src/
│     │  ├─ app/                 React shell, navigation, lightweight routing
│     │  ├─ assets/              Frontend images and store assets
│     │  ├─ features/            Jobs, AI, settings, members, pet, store pages
│     │  └─ shared/              i18n, components, services, types, global styles
│     └─ src-tauri/
│        ├─ capabilities/        Tauri window permission boundaries
│        ├─ resources/           Runtime manifest, DOCX template, pet resources
│        ├─ src/                 Rust commands, database, runtime, exports, pet
│        └─ tauri.conf.json      Tauri config and bundled resources
├─ python/
│  └─ funasr-runner/             Local transcription runner and Python deps
├─ scripts/                      Startup, runtime preparation, release checks
├─ docs/
│  ├─ architecture/              Architecture and release readiness docs
│  ├─ images/                    README screenshots
│  ├─ ai/                        AI process docs and migrated historical records
│  └─ pet-system.md              Current pet system notes
├─ Cargo.toml                    Rust workspace
├─ package.json                  pnpm workspace scripts
└─ pnpm-workspace.yaml
```

## Main Screens

- `Dashboard`: review job metrics, processing trends, result completeness, recent jobs, resources, and companion state.
- `New Job`: choose local media, title, language, speaker diarization, and hotwords.
- `Job Queue`: filter and search jobs; completed jobs open the result workbench directly, while active or failed jobs open details; retry and delete actions remain available by state.
- `Job Detail`: inspect input, status, progress, logs, and failure reason, then open the result workbench for a completed job.
- `Result Workbench`: use a dedicated window scoped to one job to review transcripts, filter or rename speakers, open AI summary and notes windows, and export results. The legacy `/results` route redirects to the completed-job queue.
- `Models` / `Templates` / `Members`: manage resources as cards and edit them in dedicated windows; member management includes Excel import/export.
- `Settings`: navigate categorized appearance, locale, runtime-component, ASR, remote-backend, and diagnostics controls.
- `Pet Center`: view pet level, cumulative growth, stage, events, desktop behavior, and interactions.
- `Pet Store`: view LP, catalog, inventory, equipment, food/tool usage, and item details.
- `Daily Blind Box`: open up to 10 free local boxes per day; rewards come from the Pet Store, excluding pets.
- `Daily Check-In`: view streaks, reward calendar, history, claim rewards, or use a make-up ticket.
- `Redeem Center`: enter locally verified redeem keys for LP, growth value, or pet items, and view device-local redemption history.

## Local Data

SQLite currently stores:

- App settings and managed runtime state.
- Jobs, input files, transcript segments, job events, and process-log snapshots.
- AI models, summary templates, summary runs, and active summary selection.
- Meeting members, departments, sort order, and recorder flag.
- Pet profile, desktop behavior settings, growth events, stage cosmetics, and level snapshots.
- LP wallet, inventory, economy ledger, milestone counters, daily blind box history, check-in records, daily free-claim state, and redemption history.

The schema is created in `apps/desktop/src-tauri/src/local_db/schema.rs`; migration versioning is maintained in `infrastructure/migrations.rs`.

## Development Commands

Install dependencies:

```bash
pnpm install --frozen-lockfile
```

Start the frontend:

```bash
pnpm desktop:dev:web
```

Start the Tauri desktop app:

```bash
pnpm desktop:tauri dev
```

Build the frontend:

```bash
pnpm desktop:build:web
```

Build the desktop app:

```bash
pnpm desktop:tauri build
```

Run the full check:

```bash
pnpm check
```

`pnpm check` runs contract-drift checks, frontend type checking and unit tests, Python Ruff/pytest, Rust fmt/tests, the frontend production build, Clippy, and version, platform, security-baseline, runtime-lock, dependency-governance, and ASR blocking-fixture checks.

## Supported Platforms

| Platform | Rust target | Validation level | Runtime backend |
| --- | --- | --- | --- |
| macOS Apple Silicon | `aarch64-apple-darwin` | Primary | FunASR |
| macOS Intel | `x86_64-apple-darwin` | Primary | FunASR |
| Windows x64 | `x86_64-pc-windows-msvc` | Primary | FunASR |
| Windows x86 | `i686-pc-windows-msvc` | Extended | sherpa-onnx |

## Documentation

- [Pet system notes](./docs/pet-system.md)
- [Enterprise desktop architecture](./docs/architecture/enterprise-desktop-architecture.md)
- [Release readiness](./docs/architecture/release-readiness.md)

## Notes

- The frontend uses React/TSX.
- Local mode uses the client-downloaded managed runtime by default, with manual Python override available in Settings.
- Local execution depends on ffmpeg, the Python runner, and model resources; incomplete runtime state is marked `repair_required`.
- Runtime download sources are controlled by `runtime-manifest.json` and the Environment & Models selection in Settings; sources must be real reachable URLs, not placeholder assets.
- Formal DOCX meeting minutes use `apps/desktop/src-tauri/resources/templates/meeting-minutes.docx`.
- The pet system is a local reward and companion system. It does not include real-money payments, top-ups, trading, or leaderboards; the daily blind box is a free benefit that does not consume LP, sell attempts, or use paid probability drops.

## License

This project is licensed under the MIT License. See [LICENSE](./LICENSE) for details.
