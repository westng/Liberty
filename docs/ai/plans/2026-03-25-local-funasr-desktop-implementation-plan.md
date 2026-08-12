# Liberty Local FunASR Desktop Implementation Plan

文档类型：计划
状态：历史快照
创建日期：2026-03-25
最后核实：2026-08-12（仅核实分类与迁移路径，正文完成度和技术事实未重新核实）
适用范围：Liberty 本地 FunASR 桌面处理实施计划
权威边界：本文保留历史实施背景，不是当前任务清单；当前行为以代码、测试和正式文档为准
依据：迁移前同名历史计划（原 Superpowers 过程文档）

## Goal

Replace the current mock-or-remote processing flow with a local desktop
processing pipeline that launches a developer-configured Python runner from
Tauri, uses FunASR for transcription and speaker segmentation, and feeds the
existing Liberty UI with real local task data.

## Phase 1 Deliverables

- Tauri command layer for local job creation, listing, detail loading, result
  loading, and retry
- Local task workspace on disk with stable per-job directory structure
- Settings support for Python executable path and FunASR runner script path
- Frontend state flow switched from remote-first mock handling to Tauri local
  job orchestration
- Real local processing for one audio or video file with:
  - transcript text
  - timestamps
  - speaker segments
- Compatibility mapping from local result files into the existing `MeetingJob`
  frontend model
- Failure reporting based on Python launch errors, missing dependencies, and
  invalid result files

## Work Breakdown

### 1. Settings And Configuration

- Extend `SettingsState` with:
  - `pythonPath`
  - `runnerScriptPath`
- Update the settings view to expose both fields clearly for development use
- Persist the new settings alongside the existing backend and template fields
- Decide how local mode is enabled:
  - recommended: use local mode when both Python and runner paths are present
- Keep the current remote backend settings intact for now, but make the local
  execution path the primary development route

### 2. Tauri Local Job Domain

- Add Rust-side job models for:
  - job metadata
  - progress snapshot
  - result payload
- Define a local workspace root for Liberty task directories
- Implement per-job directory creation and file naming rules
- Add helpers to read and write:
  - `job.json`
  - `progress.json`
  - `result.json`
  - `process.log`
- Keep file IO centralized so command handlers stay thin

### 3. Python Runner Invocation

- Add a Rust command that spawns the configured Python executable with explicit
  argument arrays
- Pass the runner contract arguments:
  - `--job-dir`
  - `--input`
  - `--lang`
  - `--speaker`
  - `--hotwords`
- Avoid shell command strings to reduce quoting bugs on Windows
- Capture process exit status and append stdout and stderr into `process.log`
- Mark the job failed if Python cannot start or exits non-zero without a valid
  result

### 4. Tauri Command Surface

- Implement Tauri commands for:
  - `create_job`
  - `list_jobs`
  - `get_job`
  - `get_job_result`
  - `retry_job`
- Return payloads shaped close to the current TypeScript `MeetingJob`
- Keep command signatures stable and frontend-friendly
- Add clear error messages for:
  - missing Python path
  - missing runner script path
  - nonexistent input file
  - invalid result contract

### 5. Frontend Service Layer Refactor

- Introduce a local-meeting service layer for Tauri invocations
- Refactor `useMeetingStore` to support:
  - local Tauri mode
  - existing mock fallback during transition if needed
- Stop treating remote HTTP as the default processing path for development
- Replace the current mock-only `simulatePipeline` behavior when local mode is
  available
- Keep the frontend route structure and view composition intact

### 6. Frontend Model Compatibility

- Extend the TypeScript settings model with Python and runner fields
- Define compatibility behavior for missing local summary data:
  - `summaryStatus` stays non-blocking
  - `summary` returns an empty stable structure
- Keep transcript and speaker rendering logic compatible with the new local
  result contract
- Ensure exports continue to prefer `speakerSegments` when available

### 7. New Job Flow Changes

- Keep the current desktop file picker UI
- Restrict phase 1 task submission to a single file for real local processing
- Prevent accidental multi-file task creation in local mode until that workflow
  exists
- On submit:
  - validate title
  - validate file path presence
  - validate Python and runner configuration
  - call Tauri `create_job`
- Route to job detail after local job creation as today

### 8. Jobs, Detail, And Workbench Wiring

- Load jobs from Tauri instead of relying only on in-memory mock seeds
- Update the detail page to reflect local stages and failure output cleanly
- Update the workbench loader to read the local aggregated result
- Keep current UI simplifications where possible to avoid unnecessary redesign
- Ensure retry uses the original local input path and parameters

### 9. Python Runner Reference Contract

- Add a development runner script in the repo or document its expected external
  location
- Define the exact JSON shapes required for:
  - `progress.json`
  - `result.json`
- Require the runner to emit:
  - transcript segments
  - speaker segments
  - failure reason when applicable
- Decide whether video inputs are passed directly to the runner or whether the
  runner itself handles audio extraction

### 10. Verification

- Type-check the frontend
- Build the frontend bundle
- Run Tauri Rust checks
- Perform manual local processing tests on macOS with:
  - valid audio file
  - valid video file
  - bad Python path
  - bad runner path
  - missing dependency or model
- Perform at least one Windows path test with spaces in the file path

## Implementation Order

1. Extend settings types and UI with Python and runner configuration
2. Add Rust local workspace helpers and job file contracts
3. Add Rust Tauri commands for local jobs
4. Implement Python process spawning and log handling
5. Add frontend local service bindings to Tauri commands
6. Refactor `useMeetingStore` to use local job orchestration
7. Update New Job flow for single-file local processing
8. Wire Jobs, Detail, and Workbench to local result loading
9. Validate failure handling and retry behavior
10. Run end-to-end local processing tests

## Key File Targets

### Frontend

- `src/types/meeting.ts`
- `src/composables/useMeetingStore.ts`
- `src/views/NewJobView.vue`
- `src/views/SettingsView.vue`
- `src/views/JobDetailView.vue`
- `src/views/WorkbenchView.vue`
- `src/services/`

### Tauri

- `src-tauri/src/` command and filesystem modules
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json` only if command exposure or capabilities need it

### Docs And Dev Assets

- `docs/ai/designs/2026-03-25-local-funasr-desktop-design.md`
- local runner contract documentation or helper script checked into the repo

## Risks

- The current frontend model still assumes summary data exists
- Tauri process management may differ between macOS and Windows
- Python runner output may drift without a strict contract
- Existing mock data may mask integration gaps if left enabled too broadly

## Mitigations

- Keep a stable empty summary shape until local summary generation is designed
- Use direct process APIs and explicit argument arrays
- Centralize result parsing around one Rust loader for `result.json`
- Limit local real-processing scope to single-file tasks until the core path is
  reliable

## Definition Of Done

- A developer can configure local Python and runner paths in Liberty
- A developer can select one local media file and create a task
- Tauri launches the Python runner and persists task state locally
- The task reaches `completed` or `failed` with inspectable logs
- The workbench shows real transcript and speaker data from local files
- Retry works for a failed local task
- The build passes after the integration changes
