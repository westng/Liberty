# Liberty Meeting Audio Desktop Design

文档类型：设计
状态：历史快照
创建日期：2026-03-25
最后核实：2026-08-12（仅核实分类与迁移路径，正文技术事实未重新核实）
适用范围：Liberty 会议音视频桌面处理方案
权威边界：本文保留历史设计背景，不代表当前实现；当前行为以代码、测试和正式文档为准
依据：迁移前同名历史设计（原 Superpowers 过程文档）

## Overview

Liberty is a desktop application for processing meeting audio and video files.
The desktop client is built with Tauri, Vue 3, and TypeScript. Audio processing
is handled by a remote Python service that uses FunASR with SeACo-Paraformer as
the ASR base. The first release targets file upload workflows and produces both
verbatim transcripts and structured meeting notes.

## Goals

- Support local meeting audio or video file upload from the desktop client
- Use a remote service based on FunASR and SeACo-Paraformer for transcription
- Present transcript and structured meeting notes side by side
- Support timestamps, optional speaker segments, editing, retry, and export

## Non-Goals

- Real-time recording or real-time transcription
- Direct integrations with meeting platforms
- Fully offline desktop inference
- Team collaboration and permission systems
- Plugin marketplace or workflow builder in the first release

## Recommended Architecture

### Option Summary

1. Thin desktop client with remote processing
   - Tauri handles file selection, task management, result editing, and export
   - Remote services handle ASR, speaker processing, summary generation, and
     result persistence
   - Recommended because it minimizes first-release risk and matches the
     current SeACo-Paraformer usage model
2. Hybrid client with upload orchestration
   - Desktop client additionally handles chunking, resumable uploads, and more
     aggressive local caching
   - Better for large files but increases desktop complexity
3. Plugin-style workflow orchestration
   - Every processing stage becomes a configurable node in the desktop client
   - Most flexible but excessive for the first release

### Selected Approach

Use the thin desktop client approach first, while keeping interfaces stable
enough to evolve into the hybrid model later.

## System Architecture

### Desktop Client

The desktop app contains four functional areas:

- File intake
- Job list and status tracking
- Result workbench
- Settings

Tauri is only responsible for local capabilities such as file picking, export
downloads, and local cache directory management. UI and application state live
in the Vue 3 and TypeScript frontend.

### Remote Services

The remote Python backend is split into three processing lanes:

- ASR lane
  - Uses FunASR with SeACo-Paraformer for transcription
  - Produces transcript text, punctuation, hotword-enhanced recognition, and
    timestamps
- Meeting understanding lane
  - Generates structured meeting notes from transcript content
  - Produces summary, topics, decisions, and action items
- Job orchestration lane
  - Tracks job state, retries, persistence, and export generation

### Data Flow

1. User uploads one or more local meeting files from the desktop client
2. Desktop client creates a remote job
3. Remote service processes the file asynchronously
4. Desktop client polls or subscribes for job status
5. Completed jobs open in a result workbench
6. User edits transcript or meeting notes and exports the chosen output

## Frontend Information Architecture

### Main Views

- New Job
  - Drag and drop or browse for meeting files
  - Configure title, hotwords, language, and output preferences
- Jobs
  - Display queued, processing, completed, and failed jobs
  - Support retry, delete, and open result
- Job Detail
  - Show upload progress, processing stage, and error summary
- Result Workbench
  - Display transcript and meeting notes in parallel
  - Support transcript search, timestamp navigation, speaker grouping, editing,
    regeneration, and export
- Settings
  - Configure backend base URL, credentials, default hotwords, export
    preferences, and concurrency

### Result Workbench Model

The transcript and meeting notes are equally important and should be shown in
parallel instead of treating notes as a secondary artifact.

- Transcript pane
  - Segment-based data model
  - Each segment contains `startMs`, `endMs`, `speaker`, and `text`
  - Supports full-text search and timestamp-based navigation
- Notes pane
  - Structured sections instead of one freeform editor
  - Contains summary, topics, decisions, and action items
- Regeneration rule
  - Transcript edits can trigger note regeneration
  - Regeneration should never silently overwrite user-edited notes

## Job Data Model

Use a single `meeting_job` resource as the primary unit to avoid splitting the
workflow into loosely coordinated tasks.

### Core Fields

- `id`
- `title`
- `sourceFile`
- `duration`
- `createdAt`
- `hotwords`
- `lang`
- `enableSpeaker`
- `summaryTemplate`
- `uploadStatus`
- `asrStatus`
- `summaryStatus`
- `overallStatus`
- `transcriptSegments`
- `speakerSegments`
- `summary`
- `topics`
- `decisions`
- `actionItems`
- `exportFormats`
- `lastExportedAt`

## Backend API Boundary

Keep the first version narrow and task-oriented.

- `POST /api/jobs`
  - Create a job and upload a file or return upload instructions
- `GET /api/jobs`
  - Return the job list
- `GET /api/jobs/:id`
  - Return job detail and processing state
- `GET /api/jobs/:id/result`
  - Return transcript, speaker output, and meeting notes
- `POST /api/jobs/:id/retry`
  - Retry a failed job
- `POST /api/jobs/:id/regenerate-summary`
  - Regenerate notes from the latest transcript
- `GET /api/jobs/:id/export`
  - Export result in `txt`, `md`, `srt`, or `docx`

## Processing State Machine

The desktop client should only depend on stable task states instead of model
internals.

1. `uploaded`
2. `queued`
3. `transcribing`
4. `speaker_processing`
5. `summarizing`
6. `completed`
7. `failed`

## Error Handling

- Upload failures
  - Preserve a local draft and allow re-upload
- Transcription failures
  - Surface whether the error is due to file format, duration limits, service
    issues, or model failures
- Meeting-note generation failures
  - Keep transcript results usable and allow independent retry
- Export failures
  - Do not affect the main task result
- Service unavailability
  - Show backend offline state and keep cached task data visible

## Phase 1 Scope

### Included

- Upload one or more local files
- Run transcription through FunASR with SeACo-Paraformer
- Return timestamps and optional speaker segments
- Generate structured meeting notes
- Display transcript and meeting notes side by side
- Support search, light editing, retry, and export

### Excluded

- Real-time capture
- Meeting platform integrations
- Offline local model inference
- Collaboration features
- Complex access control
- Workflow plugins

## Testing Strategy

- Unit tests for frontend job state mapping, result shaping, and export actions
- Component tests for upload flow, job status rendering, and result workbench
- API contract tests between the desktop client and the Python backend
- End-to-end tests for upload, processing status progression, result viewing,
  and export

## Risks And Mitigations

- Long-running backend jobs may produce stale frontend state
  - Mitigation: use polling with explicit state transitions and retry-safe fetch
- Meeting-note generation quality may vary with transcript quality
  - Mitigation: keep transcript editable and support manual note regeneration
- Large media files may stress upload reliability
  - Mitigation: preserve space for resumable upload evolution in later phases

## Next Step

The next planning phase should turn this design into an implementation plan for:

- Tauri plus Vue 3 application scaffolding
- Frontend state model and routes
- Backend API contract and mock service
- Result workbench UI
- Export flow and local file handling
