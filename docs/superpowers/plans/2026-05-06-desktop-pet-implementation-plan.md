# Liberty Desktop Pet Implementation Plan

> Historical note: this plan reflects the first desktop-pet implementation pass.
> Current frontend structure, pet growth, LP economy, food rewards, item gates,
> and daily blind-box behavior are governed by
> [Liberty 宠物 255 级成长生态策略](../specs/2026-05-21-宠物255级成长生态策略.md).
> Treat this file as historical sequencing context, not the source of current
> numeric rules.

## Goal

Add a real desktop-persistent pet to Liberty using the existing Tauri, React,
TypeScript, Rust, and SQLite architecture. The pet should live in its own
desktop window, grow from meaningful Liberty activity, support high-frequency
interaction with user controls, and expose a dedicated in-app management
surface.

## Phase 1 Deliverables

- Pet domain types shared across frontend views and services
- SQLite persistence for pet profile, settings, event ledger, and cosmetic
  unlock records
- Rust pet module with Tauri commands for:
  - pet state loading
  - pet settings updates
  - interaction application
  - event application
  - desktop pet window control
- Desktop pet webview window created through existing window helpers
- A lightweight pet desktop UI with placeholder visuals until final assets land
- Pet management page in the main application
- Event mapping from core Liberty workflow milestones into pet growth and mood
- Basic growth loop with:
  - level
  - cumulative growth value
  - 8 companion stages
  - 255-level snapshot
  - current mood
- LP wallet and local reward ledger
- Daily free blind-box state and history
- Disturbance controls with:
  - desktop enable switch
  - mute switch
  - focus mode
  - proactive interaction level
- Position persistence for the pet window

## Work Breakdown

### 1. Domain Model Expansion

- Add frontend pet types alongside existing meeting types or in a new dedicated
  type module
- Define:
  - `PetProfile`
  - `PetSettings`
  - `PetMood`
  - `PetStage`
  - `PetCosmeticUnlock`
  - `PetEventLedgerEntry`
  - `PetInteractionAction`
  - `PetGrowthSummary`
  - `PetLevelSnapshot`
  - `PetStoreState`
  - `PetBlindBoxState`
- Keep the first pass explicit and serializable for Tauri invoke boundaries
- Avoid mixing pet transient state into `MeetingJob`

### 2. SQLite Schema And Persistence

- Extend `src-tauri/src/local_db.rs` with pet tables
- Add schema bootstrap or migration logic for:
  - `pet_profile`
  - `pet_settings`
  - `pet_event_ledger`
  - `pet_cosmetic_unlocks`
  - `pet_wallets`
  - `pet_inventory`
  - `pet_economy_ledger`
  - `pet_milestone_counters`
  - `pet_blind_box_draws`
- Seed a default single pet record when no profile exists
- Provide read and write helpers for:
  - loading the active pet
  - saving desktop settings
  - saving event ledger entries
  - applying experience and stage changes atomically
  - applying LP rewards atomically
  - recording daily blind-box draws
- Keep pet writes isolated from job persistence failures

### 3. Rust Pet Module

- Add a new module such as `src-tauri/src/local_pet.rs`
- Implement Tauri command wrappers for:
  - `get_pet_profile`
  - `get_pet_settings`
  - `save_pet_settings`
  - `list_pet_event_ledger`
  - `apply_pet_interaction`
  - `apply_pet_workflow_event`
  - `get_pet_store_state`
  - `purchase_pet_store_item`
  - `equip_pet_inventory_item`
  - `unequip_pet_inventory_slot`
  - `use_pet_inventory_item`
  - `get_pet_blind_box_state`
  - `draw_pet_blind_box`
  - `show_pet_window`
  - `hide_pet_window`
  - `update_pet_window_position`
- Register the new commands in `src-tauri/src/lib.rs`
- Keep command contracts narrow and deterministic

### 4. Growth Calculation Layer

- Add a Rust-owned 255-level growth calculator instead of scattering math in React
- Encode the source priorities from the spec:
  - task progression
  - result accumulation
  - usage activity
- Implement an explicit event-to-reward map for the first event set:
  - meeting file imported
  - local transcription started
  - local transcription completed
  - AI summary completed
  - export completed
- Support:
  - cumulative experience
  - level-up detection
  - 8-stage threshold checks
  - Lv.255 cap with continued cumulative experience
  - work reward multipliers and LP multipliers
  - food fixed growth values outside multiplier logic
  - cosmetic unlock hooks for later use

### 5. Mood And State Layer

- Model short-lived state separately from progression
- Start with a small mood enum:
  - `idle`
  - `cheerful`
  - `excited`
  - `proud`
  - `needy`
  - `sleepy`
  - `bored`
- Map both workflow events and direct interactions into mood changes
- Persist only what must survive restarts
- Keep timers and cooldown logic centralized so the desktop UI remains dumb

### 6. Desktop Pet Window Shell

- Extend `src/services/window.ts` with pet-window helpers
- Open a dedicated pet window label such as `desktop-pet`
- Use a frameless, fixed-size window optimized for a small visual footprint
- Pass route context to a new standalone route such as `/pet-desktop`
- Add restore logic:
  - create on launch when enabled
  - restore last known position
  - hide instead of destroy when temporarily disabled if that reduces churn
- Defer advanced platform-specific click-through until the shell is stable

### 7. Desktop Pet Frontend View

- Add a standalone pet window view under the current React feature structure
- Build a constrained UI with:
  - pet visual layer
  - small mood bubble area
  - interaction hit targets
  - drag affordance if needed
- Use placeholder static or simple animated assets initially
- Do not block implementation on final art delivery
- Keep the layout stable so future sprite-sheet swaps do not require structural
  rework

### 8. In-App Pet Management View

- Add a new route and navigation entry for pet management
- Create a dedicated page that shows:
  - current level and experience
  - current stage
  - current mood
  - recent event history
  - unlocked cosmetics
  - desktop behavior settings
- Reuse existing Liberty page layout patterns from management screens
- Keep V1 focused on clarity, not dense gamification

### 9. Frontend Service And State Wiring

- Add a shared service such as `apps/desktop/src/shared/services/tauri/pet.ts`
- Wrap all pet-related Tauri invokes in that service
- Add a focused React store such as `apps/desktop/src/features/pet/stores/usePetStore.ts`
- Keep pet state ownership separate from `useMeetingStore`
- Add only narrow integration points from meeting workflows into pet events

### 10. Workflow Event Integration

- Identify the existing points where Liberty already knows a workflow milestone
  succeeded
- Trigger pet events from those stable boundaries instead of low-level UI
  click handlers
- Recommended first integration points:
  - after job creation succeeds
  - after local transcription status becomes completed
  - after AI summary run completes
  - after export succeeds
- Ensure workflow completion remains the source of truth and pet updates are
  best-effort side effects

### 11. Disturbance Controls And Guardrails

- Add desktop behavior settings for:
  - desktop enablement
  - mute
  - focus mode
  - proactive level
- Enforce rate limiting for proactive prompts in one shared location
- Suppress proactive interruptions while long-running jobs are actively
  processing if that feels noisy in testing
- Provide a fast hide action from the desktop pet surface or tray-adjacent entry
  if available

### 12. Placeholder Asset Strategy

- Use temporary placeholder visuals so implementation can proceed before final
  pet images arrive
- Recommended placeholder options:
  - one base character silhouette
  - one idle expression
  - one excited expression
  - one speech bubble frame
- Keep asset contracts stable:
  - predictable file names
  - fixed frame sizes
  - simple stage-to-asset mapping
- When final art lands later, replacement should be a content swap rather than
  a UI rewrite

### 13. Verification

- Run TypeScript checks for new routes, services, and React stores
- Run frontend production build
- Run Rust compile checks for new Tauri module wiring
- Manually verify:
  - pet window opens and closes correctly
  - pet position persists across relaunch
  - pet page renders correctly in the main app
  - interaction actions change mood and update state
  - workflow completion grants expected experience
  - mute and focus mode suppress proactive behavior
  - app continues functioning if the pet window fails to open

## Implementation Order

1. Add pet domain types and define the invoke DTO surface
2. Add SQLite pet tables and bootstrap logic in `local_db.rs`
3. Implement `local_pet.rs` with core state, settings, and event commands
4. Register commands in `src-tauri/src/lib.rs`
5. Add frontend pet service and React pet store
6. Add desktop pet standalone route and window helper
7. Build the desktop pet window UI with placeholder assets
8. Add the in-app pet management page and navigation entry
9. Wire stable Liberty workflow completions into pet events
10. Add disturbance controls and proactive-behavior guardrails
11. Run build checks and manual desktop verification

## Key File Targets

### Frontend

- `apps/desktop/src/app/router/index.ts`
- `apps/desktop/src/app/App.tsx`
- `apps/desktop/src/shared/services/ui/windows.ts`
- `apps/desktop/src/shared/services/tauri/pet.ts`
- `apps/desktop/src/features/pet/stores/usePetStore.ts`
- `apps/desktop/src/features/pet/views/PetManagementView.tsx`
- `apps/desktop/src/features/pet-store/`
- `apps/desktop/src/features/pet-blind-box/`
- `apps/desktop/src/shared/types/meeting.ts`

### Tauri

- `src-tauri/src/lib.rs`
- `src-tauri/src/local_db.rs`
- `src-tauri/src/local_db/pet_leveling.rs`
- `src-tauri/src/local_pet.rs`
- `src-tauri/src/infrastructure/repositories/pet_store.rs`
- `src-tauri/src/infrastructure/repositories/pet_blind_box.rs`

### Assets

- `src/assets/` for placeholder pet visuals and future final assets

### Docs

- `docs/superpowers/specs/2026-05-06-desktop-pet-design.md`

## Risks

- Desktop window behavior can differ across macOS and Windows, especially for
  always-on-top and click-through behavior
- Pet event logic can become noisy if integrated at too many UI-level points
- High interaction intensity can quickly become disruptive without strong
  throttling
- Final visual assets may require different framing if the placeholder contract
  is vague

## Mitigations

- Ship a stable minimal pet window before enabling advanced desktop behaviors
- Centralize pet event emission behind workflow success boundaries
- Make mute, focus mode, and proactive limits part of the first implementation
- Lock the placeholder asset contract early so final art swaps remain cheap
