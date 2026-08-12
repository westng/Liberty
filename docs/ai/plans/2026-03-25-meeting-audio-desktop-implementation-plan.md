# Liberty Meeting Audio Desktop Implementation Plan

文档类型：计划
状态：历史快照
创建日期：2026-03-25
最后核实：2026-08-12（仅核实分类与迁移路径，正文完成度和技术事实未重新核实）
适用范围：Liberty 会议音视频桌面处理实施计划
权威边界：本文保留历史实施背景，不是当前任务清单；当前行为以代码、测试和正式文档为准
依据：迁移前同名历史计划（原 Superpowers 过程文档）

## Goal

Build the first phase of the Liberty desktop client with Tauri, Vue 3, and
TypeScript. The client will upload meeting files, track remote processing jobs,
and present transcript and structured meeting notes side by side.

## Phase 1 Deliverables

- Tauri desktop shell with Vue 3 and TypeScript frontend
- Frontend routing for new job, jobs, job detail, result workbench, and settings
- Shared TypeScript job domain model
- API client layer for the remote meeting-processing service
- Mock-first UX with local fallback data for development
- Export bridge through Tauri shell APIs

## Work Breakdown

### 1. Project Bootstrap

- Initialize package metadata and frontend build tooling with Vite
- Configure TypeScript, Vue SFC support, and lint-ready project structure
- Add Tauri Rust shell with minimal commands required for file-system export

### 2. Frontend Shell

- Create app layout with sidebar navigation and top status area
- Add Vue Router and the five first-phase routes
- Establish a small design token system for spacing, typography, color, and
  surface styles

### 3. Domain Model And State

- Define `meeting_job` types and processing states
- Create composables for job list loading, job detail loading, and optimistic UI
- Add a mock store that can stand in for the backend during early development

### 4. API Integration Layer

- Implement a typed HTTP client for the remote Python service
- Define endpoints for job creation, listing, detail, result, retry,
  regeneration, and export
- Centralize request errors and backend unavailability handling

### 5. Core Views

- New Job: file picker, drag-and-drop, hotword input, submission
- Jobs: searchable list, status badges, retry actions
- Job Detail: pipeline stage view and failure summaries
- Result Workbench: transcript pane and structured meeting-notes pane
- Settings: backend URL and default processing options

### 6. Tauri Integration

- Add desktop-safe file selection flow
- Add export target selection and output write support
- Keep Tauri commands narrow so most logic stays in TypeScript

### 7. Verification

- Run TypeScript build checks
- Run frontend production build
- Run Tauri Rust checks if dependencies resolve locally

## Implementation Order

1. Bootstrap project files and package structure
2. Build app shell and route scaffolding
3. Add domain types and mock-backed state
4. Implement core views with mock data
5. Connect typed API client
6. Add Tauri export bridge
7. Run validation and fix integration issues

## Risks

- Native Tauri build prerequisites may be missing on the machine
- Network access may be required to install npm crates and Rust crates
- The backend contract may evolve once the Python service is implemented

## Mitigations

- Keep the frontend usable with mock data before the backend exists
- Keep API access behind a thin repository layer
- Keep Tauri commands minimal and optional during the first pass
