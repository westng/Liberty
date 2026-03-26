# Liberty SQLite Local Database Design

## Overview

Liberty currently stores local desktop task state primarily through per-job JSON
files and stores AI summary resources and summary runs in frontend-local
storage. This split was acceptable while the AI summary layer was still an
early prototype, but it is no longer a stable foundation once the product needs
to retain tasks, transcripts, speaker segments, logs, models, templates, and AI
summary outputs in a single coherent local system.

Liberty will move to SQLite as the single local business data source for the
desktop application. The database will own tasks, task files, transcript
segments, speaker segments, AI model configurations, AI summary templates, AI
summary runs, and stored process logs. The filesystem will remain useful for
input media paths and temporary runner output, but it will no longer be the
primary source of truth.

The AI summary window will also be reworked from the current evenly split
multi-column layout into a task-focused two-column workspace:

- left column for task context and run history
- right column for the current summary configuration and result preview

## Goals

- Make SQLite the single local source of truth for Liberty desktop business
  data
- Eliminate the current split between Tauri JSON persistence and frontend local
  storage for AI features
- Store tasks, source files, transcript segments, speaker segments, logs,
  models, templates, and AI summary runs in one local database
- Support automatic import of existing JSON-based tasks into SQLite
- Keep local FunASR processing working while shifting persistence to the
  database
- Redesign the AI summary window into a denser, clearer, task-centric layout

## Non-Goals

- Cloud sync or multi-device database replication
- User-managed external database files in phase 1
- Advanced SQL analytics or reporting views in phase 1
- Full removal of temporary runner output files during the migration milestone
- Encrypting local database contents in this phase

## Option Summary

1. Keep JSON files for tasks and only move AI data into SQLite
   - Would still leave Liberty with two primary local data sources
   - Not recommended because the split would keep growing more fragile
2. Keep SQLite for AI data and JSON files for tasks, then sync between them
   - Slightly cleaner than the current prototype, but still a dual-source model
   - Not recommended
3. Use SQLite as the only business data source and treat filesystem output as
   temporary input or compatibility data
   - Recommended because it creates a single coherent model for all local data

## Selected Approach

SQLite becomes the desktop app's primary persistence layer. Tauri owns all
database initialization, migration, reads, and writes. The frontend consumes
Tauri commands instead of directly reading or writing AI data from local
storage.

The current JSON task directory layout remains useful during the transition:

- existing tasks can be imported from those files
- the Python runner can continue writing its current outputs temporarily
- Rust will translate runner output into SQLite records after each run

## Storage Architecture

### Database Ownership

The SQLite database should live under the app's local data directory and be
created automatically during app startup or first command access.

Rust is responsible for:

- opening the database connection
- creating tables if missing
- applying schema migrations
- importing legacy JSON-based tasks
- exposing typed Tauri commands for frontend consumption

The Vue frontend should not manipulate raw SQLite directly and should not keep
its own shadow copies of models, templates, or summary runs in local storage.

### Filesystem Role After Migration

Filesystem storage remains relevant for:

- original media input paths
- temporary runner output files during processing
- compatibility import from existing task directories

Filesystem storage is no longer the primary business record for:

- task list state
- transcript segments
- speaker segments
- AI summary runs
- models
- templates
- stored process logs

## Data Model

### Jobs

The `jobs` table stores one row per meeting task and becomes the source for task
list and task detail views.

Recommended columns:

- `id`
- `title`
- `created_at`
- `duration_minutes`
- `lang`
- `enable_speaker`
- `summary_template`
- `upload_status`
- `asr_status`
- `summary_status`
- `overall_status`
- `failure_reason`
- `process_log`
- `python_path`
- `runner_script_path`
- `last_exported_at`

### Job Source Files

The `job_source_files` table stores task input files.

Recommended columns:

- `id`
- `job_id`
- `name`
- `path`
- `size_label`
- `kind`

### Transcript Segments

One shared `transcript_segments` table should store both regular transcript
segments and speaker-attributed segments.

Recommended columns:

- `id`
- `job_id`
- `segment_type`
  - `transcript`
  - `speaker`
- `start_ms`
- `end_ms`
- `speaker`
- `text`
- `segment_order`

This avoids duplicated table shape while keeping the two segment streams
queryable.

### AI Model Configurations

The `ai_model_configs` table stores reusable OpenAI-compatible model
definitions.

Recommended columns:

- `id`
- `name`
- `base_url`
- `api_key`
- `model`
- `enabled`
- `is_default`
- `created_at`
- `updated_at`

### AI Summary Templates

The `ai_summary_templates` table stores built-in and user-created prompt
templates.

Recommended columns:

- `id`
- `name`
- `description`
- `prompt`
- `include_speaker_by_default`
- `include_timestamp_by_default`
- `builtin`
- `created_at`
- `updated_at`

Built-in templates are seeded once into SQLite and then treated like regular
readable database records. They remain undeletable in the UI.

### AI Summary Runs

Every manual AI generation creates one row in `ai_summary_runs`.

Recommended columns:

- `id`
- `job_id`
- `model_config_id`
- `template_id`
- `include_speaker`
- `include_timestamp`
- `extra_instructions`
- `status`
- `error_message`
- `prompt_preview`
- `raw_response`
- `result_json`
- `created_at`
- `updated_at`

`result_json` should remain a JSON string in phase 1 instead of being split into
additional relational tables. The normalized summary structure is stable enough
for rendering but may still evolve as template variety increases.

## Migration Model

### Legacy Import

Existing JSON-based tasks should be imported automatically into SQLite without
requiring user action.

Import sources:

- `job.json`
- `progress.json`
- `result.json`
- `process.log`

Import behavior:

- scan the existing job directory root on startup or database initialization
- create corresponding rows when a job has not already been imported
- preserve logs, statuses, transcript segments, and speaker segments
- avoid duplicate import on later launches

### Migration Tracking

Use an internal metadata or migrations table to track:

- current schema version
- whether legacy job import has completed

This avoids repeated import work and gives Liberty a stable path for future
schema upgrades.

## Processing Flow With SQLite

### Job Creation

When a task is created:

1. Rust inserts the job row
2. Rust inserts the related source-file rows
3. Rust marks the task queued
4. Rust launches the Python runner

### Runner Progress

During processing, Rust updates:

- `jobs.asr_status`
- `jobs.overall_status`
- `jobs.failure_reason`
- `jobs.process_log`

The current progress file may still be written temporarily, but the UI should
not rely on it once SQLite-backed reads exist.

### Runner Completion

When processing completes:

1. Rust reads the runner output
2. Rust clears or replaces prior segment rows for the job
3. Rust inserts transcript segments
4. Rust inserts speaker segments
5. Rust updates task status and duration
6. Rust stores any log output in `jobs.process_log`

### AI Summary Generation

The frontend still assembles and sends the OpenAI-compatible request in phase
1, but persistence moves to Tauri-backed SQLite commands:

1. create `ai_summary_runs` row in `running` state
2. call provider
3. update the run to `completed` or `failed`
4. workbench and summary window reload from SQLite-backed data

## Frontend Data Boundary

After the migration:

- jobs, transcripts, summary runs, models, and templates should load from Tauri
  commands
- the frontend should stop using browser-local storage for AI resources or AI
  summary history
- frontend stores become view-state owners, not persistence owners

## AI Summary Window Layout

The AI summary window should move to a two-column layout.

### Left Column

The left column should contain:

- current task title
- task status badges
- transcript segment count
- recent AI summary run history

This column is informational and should remain narrower than the main work
area.

### Right Column

The right column should be split vertically into two sections.

Upper section:

- model selector
- template selector
- speaker toggle
- timestamp toggle
- additional instructions
- generate button

Lower section:

- active summary result preview
- readable error block when the latest run failed

This structure groups the active work and the result together while keeping
history from competing for attention.

## Command Surface

The Tauri command layer should expand to include SQLite-backed access for:

- job list
- job detail
- job result
- delete job
- retry job
- list AI models
- save AI model
- delete AI model
- list AI templates
- save AI template
- delete AI template
- list AI summary runs by job
- create or update AI summary run

The frontend should not need to know whether data originally came from legacy
JSON import or direct SQLite writes.

## Risks

- Migrating business data from JSON files into SQLite can create duplicate or
  inconsistent records if import logic is not idempotent
- Process logging can grow large if stored as a single job field without
  bounded handling
- Retrying jobs must clear stale segments and stale AI summary runs in a way
  that preserves user expectations
- SQLite integration adds a Rust dependency and schema management complexity

## Mitigations

- Make legacy import idempotent and keyed by stable job ids
- Keep migration state explicit in a metadata table
- Replace per-job segments transactionally on re-run
- Preserve historical AI runs unless the product explicitly wants retry to reset
  them
- Use one well-defined Rust data module instead of scattering SQL across command
  handlers

## Testing Strategy

### Database Validation

- schema initializes on first run
- built-in templates seed exactly once
- legacy tasks import exactly once
- repeated app launches do not duplicate records

### Job Flow Validation

- create task
- process task
- retry failed task
- delete task
- read transcript and speaker rows correctly

### AI Flow Validation

- save model configs
- save custom templates
- generate AI summary run
- persist completed result
- persist failed result
- reopen app and confirm run history survives

### UI Validation

- task list still loads correctly from the database
- workbench reads database-backed summary state
- AI summary window layout matches the new two-column design
- left history and right result panes stay readable on desktop sizes
