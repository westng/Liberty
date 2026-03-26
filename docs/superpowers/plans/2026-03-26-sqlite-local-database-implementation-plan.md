# Liberty SQLite Local Database Implementation Plan

## Goal

Replace Liberty's split local persistence model with a SQLite-backed desktop
data layer that stores tasks, task files, transcript segments, speaker
segments, process logs, AI model configurations, AI summary templates, and AI
summary runs. Rework the AI summary window into the approved two-column layout
while switching frontend reads and writes away from browser-local storage.

## Phase 1 Deliverables

- SQLite database initialized automatically by Tauri
- Schema and migration tracking for database versioning and legacy import state
- Automatic import of existing JSON-based local tasks into SQLite
- SQLite-backed Tauri read and write commands for:
  - jobs
  - job source files
  - transcript and speaker segments
  - AI models
  - AI templates
  - AI summary runs
- Local FunASR processing updated to write final business data into SQLite
- Frontend stores refactored to use Tauri-backed persistence instead of local
  storage for AI features
- AI summary window redesigned into left context/history plus right
  configuration/result layout

## Work Breakdown

### 1. Rust SQLite Foundation

- Add a SQLite dependency in `src-tauri/Cargo.toml`
- Create a dedicated Rust database module instead of embedding SQL in command
  handlers
- Resolve the local database path under the app local data directory
- Add schema bootstrap and migration-version tracking
- Ensure initialization is callable before any data command uses the database

### 2. Schema Design And Migration Tracking

- Create tables for:
  - `jobs`
  - `job_source_files`
  - `transcript_segments`
  - `ai_model_configs`
  - `ai_summary_templates`
  - `ai_summary_runs`
  - metadata or migrations table
- Add indexes where useful:
  - job ordering by `created_at`
  - segment lookup by `job_id`
  - summary run lookup by `job_id`
- Seed built-in templates exactly once
- Track whether legacy JSON import has already been executed

### 3. Legacy JSON Import

- Scan the existing Liberty jobs directory
- Parse:
  - `job.json`
  - `progress.json`
  - `result.json`
  - `process.log`
- Import task metadata, files, transcript segments, speaker segments, and log
  content into SQLite
- Make the importer idempotent by stable job id
- Avoid reimporting already migrated tasks on later launches

### 4. Local Job Persistence Refactor

- Refactor the existing `local_jobs.rs` flow so job creation, listing, detail,
  deletion, retry, and result loading read and write SQLite
- Keep temporary runner files during the migration milestone if they simplify
  the runner contract
- Replace JSON-file state reads in:
  - `list_jobs`
  - `get_job`
  - `get_job_result`
  - `delete_job`
  - `retry_job`
- Continue exposing the current frontend-friendly job shape from Tauri

### 5. Runner Output Ingestion

- On local processing completion:
  - ingest transcript segments into `transcript_segments` as `transcript`
  - ingest speaker segments into `transcript_segments` as `speaker`
  - update task duration and statuses in `jobs`
  - store process log text in `jobs.process_log`
- On retry:
  - clear or replace stale segment rows for the job
  - reset task statuses cleanly
- Preserve a robust failure path that records detailed error messages and log
  output in SQLite

### 6. AI Resource Commands

- Add Tauri commands for AI models:
  - list
  - save
  - delete
- Add Tauri commands for AI templates:
  - list
  - save custom
  - delete custom
- Seed built-in templates in SQLite instead of in frontend local storage
- Keep built-in templates visible but undeletable in the frontend

### 7. AI Summary Run Commands

- Add Tauri commands for:
  - list summary runs by job
  - create summary run in `running` state
  - update summary run to `completed` or `failed`
- Store:
  - prompt preview
  - raw provider response
  - normalized result JSON
  - error message
- Make workbench and summary window derive active summary state from SQLite
  rather than browser-local storage

### 8. Frontend Store Refactor

- Remove local-storage persistence for:
  - AI models
  - AI templates
  - AI summary runs
- Replace it with Tauri service bindings
- Keep frontend composables focused on:
  - loading data
  - updating form state
  - rendering current results
- Minimize business duplication between `useMeetingStore` and the AI store

### 9. AI Summary Window Layout Refactor

- Rebuild `AiSummaryView` into a two-column layout
- Left column:
  - task title and summary state
  - transcript stats
  - AI summary run history
- Right column, top:
  - model selector
  - template selector
  - speaker toggle
  - timestamp toggle
  - extra instructions
  - generate button
- Right column, bottom:
  - normalized summary preview
  - error display when the latest run failed
- Ensure the layout reads well at the target desktop sizes

### 10. Workbench And Detail Integration

- Update the Result Workbench to consume SQLite-backed summary runs
- Ensure active summary result and summary status come from the database-backed
  task model
- Keep Job Detail consistent with the new summary status semantics
- Keep export behavior working against the normalized summary result

### 11. Verification

- Run `cargo check`
- Run frontend type-check and production build
- Manually verify:
  - old JSON tasks import into SQLite
  - new tasks are created in SQLite
  - retry updates the same job correctly
  - model management persists across restarts
  - template management persists across restarts
  - AI summary runs persist across restarts
  - AI summary window layout is correct and readable
  - failed AI runs do not break transcript browsing

## Implementation Order

1. Add SQLite dependency and Rust database bootstrap module
2. Add schema creation, migration tracking, and built-in template seeding
3. Implement legacy JSON import
4. Refactor local job commands to read and write SQLite
5. Ingest runner outputs and logs into SQLite
6. Add Tauri commands for AI models, templates, and summary runs
7. Refactor frontend services and stores off local storage
8. Rebuild AI summary window layout and reload flow
9. Validate build, runtime wiring, and migration behavior

## Key File Targets

### Rust

- `src-tauri/Cargo.toml`
- new database module under `src-tauri/src/`
- `src-tauri/src/local_jobs.rs`
- `src-tauri/src/lib.rs`

### Frontend

- `src/types/meeting.ts`
- `src/composables/useMeetingStore.ts`
- `src/composables/useAiStore.ts`
- `src/services/`
- `src/views/AiSummaryView.vue`
- `src/views/WorkbenchView.vue`
- `src/views/ModelManagementView.vue`
- `src/views/TemplateManagementView.vue`
- `src/style.css`

### Docs

- `docs/superpowers/specs/2026-03-26-sqlite-local-database-design.md`

## Risks

- Rust SQLite integration may require dependency download and environment
  setup not previously needed
- The current job contract assumes file-based storage and may hide missing
  fields when moved to the database
- Legacy import can become lossy if JSON parsing does not mirror current job
  semantics closely enough
- The frontend currently depends on in-memory merged models and will need a
  careful cutover

## Mitigations

- Use a single database access layer with typed mapping helpers
- Keep legacy import focused and idempotent
- Preserve the current frontend-facing `MeetingJob` shape even if the backing
  source changes
- Replace local-storage persistence in one pass instead of maintaining dual
  writes

## Definition Of Done

- Liberty initializes a local SQLite database automatically
- Existing JSON-based tasks are imported into SQLite once
- New and retried tasks persist through SQLite-backed commands
- Models, templates, and AI summary runs persist through SQLite instead of
  browser-local storage
- AI summary results survive app restarts
- The AI summary window uses the new two-column layout
- The build passes after the migration
