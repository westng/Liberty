# Liberty Online AI Summary Implementation Plan

文档类型：计划
状态：历史快照
创建日期：2026-03-26
最后核实：2026-08-12（仅核实分类与迁移路径，正文完成度和技术事实未重新核实）
适用范围：Liberty 在线 AI 总结实施计划
权威边界：本文保留历史实施背景，不是当前任务清单；当前行为以代码、测试和正式文档为准
依据：迁移前同名历史计划（原 Superpowers 过程文档）

## Goal

Add a manual online AI summarization layer on top of Liberty's existing local
desktop transcription flow. Users should manage reusable model configurations
and reusable prompt templates, then launch a dedicated AI-summary window from
the Result Workbench to generate structured meeting output through a standard
OpenAI-compatible API.

## Phase 1 Deliverables

- New menu entries for Model Management and Template Management
- Local persistence for saved model configurations and saved prompt templates
- Built-in prompt templates seeded in the app
- Result Workbench `AI 总结` entry that opens a dedicated Tauri window
- AI summary window with:
  - model selection
  - template selection
  - speaker toggle
  - timestamp toggle
  - additional instructions input
  - generate action
- OpenAI-compatible request service using `baseUrl`, `apiKey`, and `model`
- Structured summary run records stored per meeting job
- Workbench display of the active AI summary result
- Failure handling that preserves transcript usability when AI summarization
  fails

## Work Breakdown

### 1. Domain Model Expansion

- Extend the TypeScript meeting domain with:
  - `AiModelConfig`
  - `AiSummaryTemplate`
  - `AiSummaryRun`
  - normalized `AiSummaryResult`
- Update `MeetingJob` to support:
  - `summaryRuns`
  - `activeSummaryRunId`
  - `summaryStatus` with `idle`, `running`, `completed`, `failed`
- Keep existing transcript and speaker types unchanged
- Define clear rules for empty or missing AI-summary state on existing jobs

### 2. Local Persistence Layer

- Add storage helpers for:
  - model configurations
  - template configurations
  - per-job summary-run records
- Keep built-in templates seeded in code and merged with user-created templates
- Persist user-defined templates and saved models across restarts
- Avoid coupling AI resource persistence directly to the local ASR storage files
  unless the job-storage shape clearly benefits from it

### 3. Model Management Page

- Add a new route and page for Model Management
- Support CRUD for saved model entries
- Validate required fields:
  - display name
  - base URL
  - API key
  - model name
- Support default-model selection
- Support enabled or disabled state for saved entries
- Keep phase 1 scope narrow and avoid advanced parameter controls

### 4. Template Management Page

- Add a new route and page for Template Management
- Seed built-in templates such as:
  - standard meeting summary
  - decisions and action items
  - project weekly review
  - interview notes
- Support custom template creation and editing
- Support duplicating built-in templates into editable custom templates
- Prevent direct deletion of built-in templates
- Expose template description and default speaker or timestamp toggles in the
  management UI

### 5. Navigation And App Shell

- Add navigation entries for:
  - Model Management
  - Template Management
- Keep the current app shell and route capsule pattern consistent
- Ensure new pages fit the existing desktop layout and spacing language
- Keep Settings focused on app-level and local-runtime preferences instead of
  AI resource management

### 6. Result Workbench Integration

- Add an `AI 总结` action in the Result Workbench
- Make the action available only when transcript data exists
- Display current summary state in the workbench:
  - no summary yet
  - generating
  - latest summary available
  - latest summary failed
- Render the active structured AI summary result using the normalized schema
- Decide whether phase 1 shows only the latest run or a selectable history list
  in the workbench
  - recommended: show latest run first, keep underlying multi-run data model

### 7. Dedicated AI Summary Window

- Add a dedicated Tauri window for a single meeting's AI summary flow
- Pass the current `jobId` into the window context
- Load:
  - current meeting title
  - available models
  - available templates
  - existing summary runs for the job if needed
- Initialize the form from:
  - default model
  - selected template defaults
- Implement the core form:
  - model selector
  - template selector
  - include speaker
  - include timestamp
  - additional instructions
- Show submission status, success state, and failure details in the window

### 8. Prompt Assembly Service

- Build a dedicated AI summary service instead of putting prompt assembly in the
  general meeting store
- Assemble requests from:
  - saved model config
  - selected template prompt
  - run-time toggles
  - user additional instructions
  - meeting transcript context
- Format transcript context according to toggles:
  - speaker and timestamp
  - speaker only
  - timestamp only
  - plain text
- Use a stable two-message structure:
  - `system` for template prompt
  - `user` for meeting content and run-time instructions

### 9. OpenAI-Compatible API Integration

- Implement a typed request layer for standard OpenAI-style chat requests
- Inject:
  - authorization header from API key
  - configured base URL
  - selected model
- Keep provider-specific assumptions minimal
- Centralize HTTP error handling for:
  - invalid credentials
  - endpoint failures
  - model mismatch
  - malformed responses

### 10. Response Parsing And Run Persistence

- Require templates to request JSON output matching the normalized summary shape
- Store the raw provider response for every run
- Parse and validate the structured summary result
- On success:
  - mark the run completed
  - update `activeSummaryRunId`
  - expose the result in the workbench
- On failure:
  - mark the run failed
  - save error details and raw response when available
  - keep transcript data and previous successful summaries intact

### 11. State Management Refactor

- Introduce focused AI modules instead of expanding `useMeetingStore` into a
  single large owner of all behavior
- Recommended modules:
  - model store or service
  - template store or service
  - AI summary request service
  - AI summary window state module
- Keep `useMeetingStore` responsible for meeting jobs and ASR lifecycle
- Add only the narrow integration points needed to read and update summary-run
  data

### 12. Verification

- Type-check all new TypeScript domain and page code
- Build the frontend bundle
- Run Tauri Rust checks if window wiring requires Tauri changes
- Manually verify:
  - creating, editing, and deleting custom models
  - creating, editing, duplicating, and deleting custom templates
  - opening AI summary window from a completed meeting
  - generating a summary successfully
  - generating multiple summaries for one job with different templates
  - invalid API key behavior
  - invalid base URL behavior
  - malformed JSON response handling
  - transcript remains usable after summary failure

## Implementation Order

1. Extend domain types for models, templates, runs, and normalized AI summary
   results
2. Add local persistence helpers and seed built-in templates
3. Add Model Management route and page
4. Add Template Management route and page
5. Extend app navigation and shell wiring
6. Add workbench `AI 总结` entry and current summary-state rendering
7. Add dedicated Tauri AI summary window and route context
8. Implement prompt assembly service and OpenAI-compatible request client
9. Implement response parsing, run persistence, and active-summary selection
10. Run build checks and manual end-to-end validation

## Key File Targets

### Frontend

- `src/types/meeting.ts`
- `src/router/index.ts`
- `src/App.vue`
- `src/composables/useMeetingStore.ts`
- `src/services/`
- `src/views/WorkbenchView.vue`
- new management views for models and templates
- new AI summary window view or route module

### Tauri

- `src-tauri/src/` if a dedicated window command or window helper is required
- `src-tauri/tauri.conf.json` only if multi-window setup needs configuration

### Docs And Seed Data

- `docs/ai/designs/2026-03-26-online-ai-summary-design.md`
- built-in template seed definitions in the frontend service or data layer

## Risks

- Summary data can sprawl if AI run history is mixed loosely into existing job
  state
- OpenAI-compatible providers vary in response shape details and error payloads
- Prompt assembly may become inconsistent if template and runtime options are
  scattered across components
- A new Tauri window adds coordination complexity if job context passing is not
  explicit

## Mitigations

- Normalize all AI summary output into one strict result schema
- Isolate provider integration behind a dedicated AI summary service
- Keep model management and template management out of Settings
- Use explicit window context derived from `jobId` rather than ad hoc shared UI
  state
- Preserve raw responses and prompt previews for debugging instead of hiding AI
  failures behind generic error messages

## Definition Of Done

- Users can manage saved OpenAI-compatible model configurations
- Users can manage built-in and custom summary templates
- The Result Workbench can open a dedicated AI summary window for a meeting
- Users can choose a model and template and manually generate a structured
  summary
- Generated summaries are saved as run records tied to the meeting job
- The latest active summary is visible in the workbench
- AI failures do not break transcript browsing or mark the whole meeting task as
  failed
- The frontend build passes after the integration changes
