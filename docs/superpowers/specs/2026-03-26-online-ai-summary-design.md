# Liberty Online AI Summary Design

## Overview

Liberty already supports local desktop transcription through Tauri and FunASR.
The next step is to add an online AI summarization layer that consumes the
transcription result and produces structured meeting outputs on demand. This
capability should remain optional, user-controlled, and clearly separated from
the local ASR pipeline.

The AI summary flow will not run automatically. Users will trigger it manually
from the Result Workbench. Liberty will provide two new management areas for
shared AI resources:

- Model Management
- Template Management

When a user wants to generate a summary, Liberty will open a dedicated
AI-summary window for the current meeting. That window will let the user choose
which model and which built-in or custom prompt template to use, and whether to
include speaker labels and timestamps in the prompt context.

## Goals

- Add online AI summarization without changing the local FunASR processing model
- Keep summary generation fully manual and initiated by the user
- Support standard OpenAI-compatible model endpoints
- Provide reusable model configurations and reusable prompt templates
- Allow built-in prompts plus one-off user instructions for a single run
- Open summarization in a dedicated window instead of a small inline dialog
- Persist AI summary runs and associated results per meeting task
- Keep transcription usable even when AI summarization fails

## Non-Goals

- Automatic summary generation after transcription finishes
- Multi-step agent workflows or chained reasoning pipelines in the first phase
- Streaming token output in the first phase
- Team-shared cloud templates or synced model credentials
- Template marketplace or remote template download
- Complex model parameter tuning beyond the core endpoint fields in phase 1

## Option Summary

1. Append one fixed AI summary call to the existing task flow
   - Smallest implementation, but too rigid for repeated runs and templates
   - Not recommended because it will force a redesign when customization grows
2. Add a dedicated AI summary layer with managed models, managed templates, and
   per-run configuration
   - Keeps ASR and AI responsibilities separate
   - Supports repeatable manual runs without disturbing the base task result
   - Recommended
3. Build a full workflow engine with multiple AI stages from the start
   - Most flexible in theory, but much heavier than the current product needs
   - Not recommended for the first online-AI milestone

## Selected Approach

Use a dedicated AI summary layer that sits on top of the completed local
transcription result. The layer will expose:

- model resource management
- template resource management
- a per-meeting AI summary window
- persistent summary-run history tied to a meeting job

The ASR pipeline remains the source of transcript, speaker, and timestamp data.
The AI layer transforms that existing result into structured summary output
through a standard OpenAI-compatible API call.

## Product Architecture

### Existing Areas

The existing Liberty areas remain:

- New Job
- Jobs
- Result Workbench
- Settings

### New Areas

Two new menu items will be added:

- Model Management
- Template Management

### Summary Entry Point

The Result Workbench will gain an `AI 总结` action. Clicking it opens a new
Tauri window scoped to the current meeting task. This window becomes the place
where users select the model, template, and per-run options before generating a
summary.

## UX Model

### Model Management

Model Management stores reusable online model configurations. Phase 1 should
support multiple saved entries even if the user only uses one in practice.

Each model configuration contains:

- `id`
- `name`
- `baseUrl`
- `apiKey`
- `model`
- `enabled`
- `isDefault`
- `createdAt`
- `updatedAt`

Purpose:

- `name` is human-friendly and appears in selection UIs
- `baseUrl`, `apiKey`, and `model` are used for standard OpenAI requests
- `enabled` hides deprecated entries without deleting them
- `isDefault` gives the AI summary window a stable initial selection

Phase 1 does not need advanced parameters such as `temperature` or
`max_completion_tokens`.

### Template Management

Template Management stores prompt templates for different meeting-output
patterns. The product should ship with built-in templates and also allow user
created templates.

Each template contains:

- `id`
- `name`
- `description`
- `prompt`
- `includeSpeakerByDefault`
- `includeTimestampByDefault`
- `builtin`
- `createdAt`
- `updatedAt`

Rules:

- Built-in templates cannot be deleted directly
- Users can duplicate built-in templates into editable custom templates
- The template owns the stable prompt body
- One-off user instructions are provided at run time and are not written back
  into the template

### AI Summary Window

The AI summary entry opens a new dedicated window titled with the current
meeting context, for example `AI 总结 - 项目周会`.

The window contains:

- current meeting title and status context
- model selector
- template selector
- `include speaker` toggle
- `include timestamp` toggle
- free-form additional instructions field
- `生成总结` action
- result or error area

This is intentionally a full window instead of a compact dialog because the
flow needs room to grow into previewing prompt details, retrying runs, and
showing structured results cleanly.

## Data Model

### Meeting Job Changes

The current `MeetingJob.summary` field is too narrow if Liberty wants to support
repeated manual runs. Summary output should become a first-class run history.

The meeting job should be extended with:

- `summaryRuns: AiSummaryRun[]`
- `activeSummaryRunId?: string`
- `summaryStatus: "idle" | "running" | "completed" | "failed"`

The existing transcript and speaker data remain the source context for summary
generation.

### AI Summary Run

Each user-triggered generation creates a new summary run record.

Recommended shape:

- `id`
- `jobId`
- `modelConfigId`
- `templateId`
- `includeSpeaker`
- `includeTimestamp`
- `extraInstructions`
- `status`
- `errorMessage`
- `promptPreview`
- `rawResponse`
- `result`
- `createdAt`
- `updatedAt`

Recommended `status` values:

- `running`
- `completed`
- `failed`

Purpose:

- `promptPreview` helps diagnose prompt assembly problems
- `rawResponse` preserves the provider output for debugging
- `result` stores normalized structured summary data for the UI

### Structured Summary Result

Phase 1 should require the AI layer to normalize output into one stable result
shape even if different templates emphasize different sections.

Recommended result shape:

- `title`
- `overview`
- `topics`
- `decisions`
- `actionItems`
- `risks`
- `followUps`

Recommended details:

- `topics` is a string array
- `decisions` is a string array
- `actionItems` is an object array containing fields such as `task`, `owner`,
  and `dueDate`
- `risks` and `followUps` can be optional in the UI if the template does not
  produce them

## Request Assembly

### Input Sources

Each AI summary request is assembled from four sources:

1. Model configuration
2. Template configuration
3. Per-run user options
4. Current meeting transcript context

### Context Rules

The request context should include:

- meeting title
- meeting language
- optional hotwords
- user additional instructions
- transcript segments

Transcript formatting depends on the two toggles:

- speaker on, timestamp on:
  - `[00:01:12 - 00:01:35] Speaker A: ...`
- speaker on, timestamp off:
  - `Speaker A: ...`
- speaker off, timestamp on:
  - `[00:01:12 - 00:01:35] ...`
- speaker off, timestamp off:
  - plain text only

These toggles do not change the stored transcript. They only control the prompt
payload for the current run.

### Message Structure

Use a standard two-message layout:

- `system`
  - the selected template prompt
- `user`
  - current meeting data, formatting options, and extra instructions

This is more maintainable than flattening all prompt logic into one string.

## OpenAI-Compatible Integration

Liberty should integrate with standard OpenAI-style chat endpoints through the
saved model configuration.

Phase 1 request requirements:

- use `baseUrl`
- use `apiKey`
- use `model`
- send a JSON-oriented prompt contract

The API layer should be encapsulated behind a dedicated AI summary service so
the rest of the frontend does not care whether the provider is OpenAI itself or
any compatible endpoint.

## Result Parsing

The selected template should instruct the model to return JSON matching the
normalized summary structure.

The client should handle provider output in two layers:

1. store the raw provider response
2. parse and validate the normalized summary result

If parsing fails:

- mark the summary run as failed
- save the raw response
- expose a readable parsing error to the user

Do not silently downgrade to free-form text in phase 1 because the workbench
needs structured output.

## State and Error Model

### Separation from ASR

AI summarization must not control whether a meeting task itself is usable.

Rules:

- if ASR succeeds, the task remains usable
- if AI summarization fails, only the summary run fails
- the transcript, speakers, and timestamps remain available
- users can retry AI summarization without rerunning ASR

### Failure Surfaces

Typical summary failures include:

- bad API key
- unreachable endpoint
- model name mismatch
- invalid provider response
- JSON parsing failure
- empty transcript context

These failures should be surfaced in the AI summary window and reflected in the
summary-run record. They should not overwrite or erase the transcription data.

## Storage Strategy

Phase 1 can persist model configurations, template configurations, and summary
run metadata locally in the desktop app's existing client-side storage model.

Recommended storage categories:

- app-level model configurations
- app-level template configurations
- per-job summary-run records

Built-in templates should be seeded in code. User-defined templates and saved
models should remain editable and locally persistent across restarts.

## Testing Strategy

### Unit-Level Focus

- prompt assembly with different toggle combinations
- normalized result parsing
- run status transitions
- model and template validation

### UI-Level Focus

- workbench launches AI summary window with the correct job context
- model management CRUD behaves correctly
- template management CRUD behaves correctly
- summary run success path updates the active result
- summary run failure path preserves task usability

### Manual Validation

- generate summaries for a completed meeting using multiple templates
- switch between multiple saved model configurations
- verify repeated runs do not overwrite transcript data
- verify AI failures do not mark the whole meeting task as failed

## Implementation Boundaries

To keep the codebase maintainable, this feature should be split into distinct
modules instead of extending `useMeetingStore` into a catch-all.

Recommended modules:

- AI model service or store
- AI template service or store
- AI summary request service
- AI summary window state module

The current local-meeting processing service should remain focused on job
creation, polling, retry, deletion, and result loading for ASR.

## Delivery Order

Recommended implementation order:

1. Add data types and local persistence for models, templates, and summary runs
2. Add Model Management and Template Management pages
3. Add the `AI 总结` entry in Result Workbench and open the dedicated window
4. Implement OpenAI-compatible request assembly and response parsing
5. Save completed runs and display the active result in the workbench

This order isolates risk and makes failures easier to diagnose during
development.
