# Liberty Meeting Members Management Design

文档类型：设计
状态：历史快照
创建日期：2026-05-01
最后核实：2026-08-12（仅核实分类与迁移路径，正文技术事实未重新核实）
适用范围：Liberty 会议人员管理方案
权威边界：本文保留历史设计背景，不代表当前实现；当前行为以代码、测试和正式文档为准
依据：迁移前同名历史设计（原 Superpowers 过程文档）

## Overview

Liberty needs a dedicated management page for personnel records used by meeting
workflows. This page is only responsible for maintaining the roster itself. It
does not yet participate in new meeting creation, participant selection, or any
other downstream workflow.

The feature adds a new sidebar menu entry and a standalone management screen
backed by Liberty's local SQLite database. Each record stores four business
fields:

- `name`
- `department`
- `sortOrder`
- `isRecorder`

The system must guarantee that at most one person is marked as the meeting
recorder at any time.

## Goals

- Add a dedicated sidebar entry for personnel management
- Provide a simple local CRUD screen for personnel records
- Persist all personnel data in the existing local SQLite database
- Guarantee a globally unique default meeting recorder
- Reuse Liberty's current Vue + Tauri + SQLite architecture and UI patterns

## Non-Goals

- Using this roster in meeting creation or meeting detail flows
- Search, filtering, pagination, or bulk operations
- Import or export for personnel records
- Multi-user sync or cloud persistence
- Role systems or access control

## Option Summary

1. Add a dedicated management page and sidebar entry
   - Recommended because the feature has a clear boundary and can evolve
     independently
2. Embed personnel management into the settings page
   - Lower initial change surface, but mixes unrelated concerns and makes later
     expansion awkward

## Selected Approach

Liberty will add a new sidebar menu item named `人员管理` and route it to a new
standalone management page. The page will show the current list and support
create, edit, and delete operations. Persistence will be implemented as new
Tauri commands over the existing local SQLite layer in `local_db.rs`.

The recorder flag will be enforced in the backend transaction path. When one
record is saved with `isRecorder = true`, all other records must be cleared in
the same database transaction before the target record is inserted or updated.

## User Experience

### Navigation

- Add `人员管理` to the left navigation menu
- Add a dedicated route such as `/members`
- The page behaves like existing resource pages such as model management and
  template management

### Page Scope

The page is limited to roster maintenance:

- view list
- add member
- edit member
- delete member

No cross-page linking or workflow automation is included in this milestone.

### List Layout

Each row should show:

- name
- department
- sort order
- recorder status
- actions

Recommended actions:

- edit
- delete

The page should include a primary `新增人员` action near the top.

### Editing Pattern

Use one shared form for both create and edit. A modal or inline editor is
acceptable; the preferred choice is whichever best matches the codebase's
existing lightweight management patterns.

Field rules:

- `name` is required
- `department` is optional and defaults to an empty string
- `sortOrder` is an integer and defaults to `0`
- `isRecorder` is boolean

When a user sets one person as recorder, the UI should save normally without
asking the user to manually clear the previous recorder.

## Data Model

Add a new table: `meeting_members`

Recommended columns:

- `id` TEXT PRIMARY KEY
- `name` TEXT NOT NULL
- `department` TEXT NOT NULL DEFAULT ''
- `sort_order` INTEGER NOT NULL DEFAULT 0
- `is_recorder` INTEGER NOT NULL DEFAULT 0
- `created_at` TEXT NOT NULL
- `updated_at` TEXT NOT NULL

### Business Semantics

- `name` supports duplicate values because real users may share the same name
- `department` is informational only in this phase
- `sort_order` controls display order
- `is_recorder` represents the globally unique default meeting recorder

### Ordering

The list should sort by:

1. `sort_order` ascending
2. `updated_at` descending or `name` ascending as a stable tiebreaker

The exact secondary sort can follow the implementation that best matches the
current management pages, as long as the ordering stays stable.

## Backend Design

### Rust Types

Add a new serializable type in the local database layer:

- `MeetingMember`

Suggested fields:

- `id: String`
- `name: String`
- `department: String`
- `sort_order: i64` or `i32`
- `is_recorder: bool`
- `created_at: String`
- `updated_at: String`

### Local Database Functions

Add new database functions in `src-tauri/src/local_db.rs`:

- `list_meeting_members`
- `save_meeting_member`
- `delete_meeting_member`

Responsibilities:

- `list_meeting_members` reads the ordered list
- `save_meeting_member` inserts or updates one member
- `delete_meeting_member` removes one member by id

### Transaction Rule for Recorder Uniqueness

`save_meeting_member` must run inside a transaction.

If the incoming record has `is_recorder = true`:

- clear `is_recorder` on all other rows
- upsert the incoming row with `is_recorder = true`

If the incoming record has `is_recorder = false`:

- only upsert the target row
- do not automatically assign another recorder

This guarantees uniqueness even if frontend state becomes stale.

### Schema Migration

Database initialization must create the `meeting_members` table if it does not
already exist. This should be added alongside the current table creation logic
in the local database bootstrap path.

## Tauri Command Surface

Add a thin command wrapper module similar to the existing local resource
modules.

Suggested commands:

- `list_meeting_members`
- `save_meeting_member`
- `delete_meeting_member`

These commands should be registered in the Tauri invoke handler so the Vue
frontend can use them through `invoke`.

## Frontend Design

### Types

Add a new shared frontend type:

- `MeetingMember`

Suggested fields mirror the backend DTO:

- `id`
- `name`
- `department`
- `sortOrder`
- `isRecorder`
- `createdAt`
- `updatedAt`

### Service Layer

Add a new local service module following current patterns:

- `listMembers()`
- `saveMember(member)`
- `deleteMember(id)`

This service should wrap Tauri `invoke` calls and stay consistent with existing
local settings and AI resource services.

### View Layer

Add a new view, for example:

- `src/views/MemberManagementView.vue`

The screen should:

- load the local list on mount
- display ordered rows
- support create and edit from one form
- support delete with a lightweight confirmation
- immediately refresh local state after mutations

### Navigation and Routing

Update:

- the router to register the new route
- the sidebar navigation list to expose the new page
- localization messages for menu text and page copy

## Validation and Error Handling

### Validation

Frontend validation should cover:

- non-empty `name`
- integer `sortOrder`

Backend validation should reject:

- empty or whitespace-only `name`

Backend validation remains the source of truth.

### Error Handling

If a save or delete fails:

- keep the current page state visible
- show the returned error message in the management UI

No optimistic mutation is required.

## Testing Strategy

### Manual Verification

Verify:

- the sidebar shows the new menu item
- the page opens correctly
- a member can be created
- a member can be edited
- a member can be deleted
- sort order affects list ordering
- setting one recorder clears the previous recorder
- deleting the recorder leaves the system with no recorder and no corruption

### Focused Regression Risk

The main regression risk is local database initialization and command
registration. Validation should confirm that existing jobs, models, templates,
and settings behavior remain unchanged after the schema update.

## Open Questions Resolved

- Recorder uniqueness: global uniqueness is required
- Page scope: this page is management-only in this milestone
- Name uniqueness: not required
