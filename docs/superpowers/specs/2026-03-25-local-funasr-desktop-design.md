# Liberty Local FunASR Desktop Design

## Overview

Liberty will move from a mock or remote-service-first workflow to a local
desktop processing model for development. The desktop app will keep its current
Vue and Tauri UI, but real media processing will run through a locally managed
Python process that executes FunASR on the user's machine. The first goal is to
make the developer's own macOS and Windows machines process a single local file
end to end with transcription, timestamps, and speaker segmentation.

## Goals

- Process a single local audio or video file from the desktop app
- Launch a local Python process from Tauri instead of calling a remote HTTP API
- Use FunASR to generate transcript text, timestamps, and speaker segments
- Persist task state and results in a stable local task directory
- Reuse the existing Liberty task list, detail, and workbench UI with minimal
  frontend model changes
- Support both macOS and Windows during development

## Non-Goals

- Packaging Python or models for end-user distribution in phase 1
- Auto-installing Python, FunASR, or model files in phase 1
- Real-time recording or real-time transcription
- Multi-file batch merging in one task
- Local meeting-note generation in the first real-processing milestone
- Task cancellation in the first milestone

## Option Summary

1. Tauri-managed local Python process
   - Tauri creates task directories, launches Python, tracks progress, and
     reads result files
   - Python receives file paths and writes standardized output files
   - Recommended because it matches the requirement to keep processing inside
     the desktop app without introducing a local HTTP layer
2. Tauri launches a local HTTP service and the frontend calls localhost
   - Clear network boundary, but adds an unnecessary service lifecycle
   - Not recommended for the first milestone
3. Frontend directly executes local processing
   - Weak process control and poor cross-platform behavior
   - Not recommended

## Selected Approach

Use Tauri as the local orchestrator and Python plus FunASR as the media
processor. Vue remains a UI and state consumer. Tauri becomes responsible for
task persistence, process lifecycle, status mapping, and result loading.

## System Architecture

### Frontend

The Vue frontend keeps the current views:

- New Job
- Jobs
- Job Detail
- Result Workbench
- Settings

The frontend no longer treats processing as mock-only or remote-only. It calls
Tauri commands for task creation, task listing, task detail, task result, and
retry. The frontend should remain unaware of Python internals and should depend
only on stable task data returned by Tauri.

### Tauri Orchestrator

The Tauri layer owns:

- Creating task ids and local task directories
- Validating configured Python executable path and processing script path
- Launching and monitoring the Python process
- Mapping Python progress to Liberty task stages
- Reading task metadata, progress, logs, and result files
- Returning a frontend-friendly job model

Tauri is the only boundary the frontend talks to for real processing.

### Python Processor

The Python layer owns:

- Accepting file path and processing arguments
- Running FunASR transcription
- Producing timestamps
- Producing speaker segmentation
- Writing normalized result files
- Writing progress updates and logs

The Python process should not need any knowledge of the Liberty frontend.

## Task Flow

1. User selects a single local media file and submits a task.
2. Frontend sends the task request to Tauri.
3. Tauri creates a local task directory and writes the initial task metadata.
4. Tauri launches the configured Python executable with the configured runner
   script and task arguments.
5. Python updates progress files while processing.
6. Tauri polls or reads task status from local files and updates the frontend.
7. Python writes final transcript and speaker outputs plus an aggregated result.
8. Tauri marks the task as completed or failed and exposes the result to the
   existing workbench UI.

## Task Directory Contract

Each task should have its own directory under a Liberty-managed workspace. The
directory structure should be stable and readable on both macOS and Windows.

Recommended files:

- `job.json`
  - Stable task metadata and requested inputs
- `progress.json`
  - Current stage, percent, status message, and last update time
- `transcript.json`
  - Transcript segments without speaker attribution if needed separately
- `speaker_segments.json`
  - Transcript segments with speaker labels
- `result.json`
  - Aggregated frontend-facing result model
- `process.log`
  - Raw execution log for debugging and failure diagnosis

The frontend should consume Tauri responses derived primarily from `result.json`
instead of assembling page data from many files directly.

## Data Contract

The current `MeetingJob` frontend model should remain the target shape returned
by Tauri as much as possible.

Required output fields:

- `id`
- `title`
- `sourceFiles`
- `createdAt`
- `lang`
- `hotwords`
- `enableSpeaker`
- `uploadStatus`
- `asrStatus`
- `overallStatus`
- `failureReason`
- `transcriptSegments`
- `speakerSegments`
- `exportFormats`

Phase 1 behavior changes:

- `summaryStatus` can stay present for compatibility but should remain queued or
  omitted from real processing until local summary generation exists
- `summary` should be returned as a stable empty object with empty strings and
  empty arrays so current pages do not break while local summary generation is
  still out of scope

## Tauri Command Boundary

The first milestone only needs a narrow command surface:

- `create_job`
  - Accept title, file path, language, hotwords, speaker flag, and template
  - Create task directory and launch Python processing
- `list_jobs`
  - Return all locally known jobs
- `get_job`
  - Return one job with current status and failure summary
- `get_job_result`
  - Return the aggregated result for the workbench
- `retry_job`
  - Re-run the existing task inputs using the same configured Python runner

Task cancellation is explicitly out of scope for this milestone.

## Python Runner Contract

The desktop app should not invoke FunASR internals directly from Rust. It should
call a dedicated Python runner script with a stable CLI contract.

Recommended arguments:

- `--job-dir`
- `--input`
- `--lang`
- `--speaker`
- `--hotwords`

Expected runner behavior:

- Exit with status code `0` on success
- Exit non-zero on failure
- Always write `process.log`
- Write `progress.json` before and during processing
- Write `result.json` only after the final result is complete enough for UI use

## Processing State Model

Liberty should keep a simple stable state machine:

1. `uploaded`
2. `queued`
3. `transcribing`
4. `speaker_processing`
5. `completed`
6. `failed`

The previous `summarizing` stage should not be part of the real local pipeline
in this milestone. If the existing UI still expects it, Tauri should map local
processing state into the existing frontend model without inventing a fake local
summary phase.

## Settings

Development mode needs two new settings:

- Python executable path
- FunASR runner script path

These must be user-editable in the existing settings page instead of being hard
coded. This keeps local development workable across macOS and Windows and avoids
embedding machine-specific paths into source files.

## Error Handling

### Python Launch Failures

- Missing executable path or bad script path should fail fast
- Tauri should set the task to `failed` with a readable failure reason
- The UI should surface the failure without hiding the raw log

### Dependency Or Model Failures

- Missing Python dependency or model assets should produce explicit failure
  messages
- Do not collapse these cases into a generic "processing failed" message

### Result Integrity Failures

- If `result.json` is missing or invalid after the process exits, the task
  should fail
- Tauri should still expose recent log output for diagnosis

### Retry

- Retry should reuse the original file path and task parameters
- Retry should preserve prior logs or rotate them instead of silently deleting
  all evidence from the previous attempt

## Cross-Platform Notes

- Use path handling that tolerates spaces, Unicode, and different separators
- Do not assume shell quoting rules are identical between macOS and Windows
- Prefer direct process spawning with explicit argument arrays over shell
  command strings
- Keep task directories and user-configured paths visible and debuggable during
  development

## Phase 1 Scope

### Included

- Single-file local task creation
- Local Python process execution from Tauri
- FunASR transcription
- Timestamp extraction
- Speaker segmentation
- Local task persistence
- Existing Liberty pages wired to real local results
- Retry of failed tasks
- Development support on macOS and Windows

### Excluded

- Multi-file processing in one job
- Local summary generation
- Auto-download of runtime or models
- Distribution-ready packaging of Python dependencies
- Task cancellation
- Real-time media capture

## Testing Strategy

- Unit tests for frontend task-state mapping and result shaping
- Rust-side tests for task directory creation and result loading where practical
- Manual development tests on macOS and Windows with:
  - valid audio input
  - valid video input
  - Python path misconfiguration
  - runner script path misconfiguration
  - missing model or dependency failures
  - file paths containing spaces or non-ASCII characters

Success for phase 1 means a single local file can move from creation to a real
completed result on the developer's machine without any mock processing.

## Risks And Mitigations

- Python environment drift across machines
  - Mitigation: keep executable and script paths configurable in settings
- Result contract instability between Python and Tauri
  - Mitigation: stabilize around `result.json` as the frontend-facing boundary
- Windows path and quoting failures
  - Mitigation: use direct process APIs and test with real Windows paths early
- Existing UI assumptions around summary generation
  - Mitigation: keep frontend compatibility fields temporarily while excluding
    local summary generation from the phase 1 pipeline

## Next Step

The implementation plan should focus on:

- Adding Tauri commands for local jobs
- Defining the on-disk task workspace and result loader
- Extending settings for Python and runner paths
- Replacing the current mock-or-remote create flow with Tauri job creation
- Mapping local result files into the existing `MeetingJob` frontend model
