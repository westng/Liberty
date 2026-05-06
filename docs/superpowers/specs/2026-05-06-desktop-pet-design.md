# Liberty Desktop Pet Design

## Overview

Liberty will add a desktop pet system that combines persistent companionship
with behavior-driven growth. The pet is not just an in-app decoration. It is a
standalone desktop presence that stays visible outside the main Liberty window
and reacts to the user's real work activity in Liberty.

The product goal is to create a long-lived emotional layer around the existing
meeting workflow without turning Liberty into a game-first application. The pet
must feel active and expressive, but its incentives should still reward useful
product behavior rather than idle grinding.

The user direction for this milestone is:

- desktop companion first
- mixed growth model
- high interaction intensity
- single main pet
- growth feedback priority:
  1. appearance stage changes
  2. emotion and state changes
  3. outfit and accessory unlocks
- growth source priority:
  1. task progression
  2. result accumulation
  3. usage activity
- V1 already includes real desktop persistence

## Goals

- Add a real desktop-persistent pet instead of an in-page avatar
- Tie pet growth to meaningful Liberty usage behavior
- Support active pet interactions and proactive pet feedback
- Keep the pet system manageable through a dedicated in-app management surface
- Reuse Liberty's Tauri desktop shell, Vue frontend, Rust local services, and
  SQLite persistence patterns

## Non-Goals

- Multi-pet collection or switching in the first version
- Cloud sync, account systems, or cross-device pet persistence
- Complex game loops such as battle, crafting, breeding, or gachas
- Deep workflow automation where the pet becomes a formal task assistant
- Audio voice playback or microphone-triggered interactions in V1

## Option Summary

1. In-app companion with fake desktop persistence
   - Lowest implementation cost
   - Not recommended because it does not satisfy the real desktop-companion
     goal
2. Real desktop pet window plus in-app management
   - Recommended because it matches the target experience and keeps
     responsibilities clear
3. Minimal desktop shell with very light growth rules
   - Faster to ship, but too shallow for the intended product identity

## Selected Approach

Liberty will implement a standalone desktop pet window managed by Tauri and a
deeper in-app pet panel managed by Vue. The desktop window is responsible for
presence, lightweight interaction, and immediate feedback. The main Liberty app
is responsible for progress visualization, management actions, configuration,
history, and richer pet details.

The pet will grow from real Liberty usage. The main experience loop is:

1. user performs work in Liberty
2. Liberty emits pet-relevant events
3. the pet updates mood, activity, and growth progress
4. the desktop pet responds visually and behaviorally
5. the user can optionally interact to amplify short-term response

This keeps the pet emotionally engaging while preserving Liberty's identity as
an audio and meeting workflow tool.

## Product Principles

### Work First, Pet Second

The system should reward work completion more than passive presence. Users
should not feel pushed to grind pet stats through meaningless behavior.

### Real Desktop Presence

The pet should exist as a genuine desktop layer with its own lifecycle, window
position, and visibility controls. The experience should still make sense when
the main Liberty window is closed or minimized.

### High Interaction With Strong Guardrails

The pet may proactively speak, request interaction, celebrate outcomes, or show
needs. However, Liberty must also provide rate limits, quiet hours, focus mode,
mute behavior, and fast hide controls so the product remains usable in work
contexts.

### One Character, Deeply Implemented

Phase 1 should invest in one character's growth curve, emotional range, and
interaction quality instead of spreading effort across multiple collectible
pets.

## User Experience

### Desktop Pet Presence

The pet appears in a frameless lightweight desktop window. Recommended
behaviors:

- visible above the desktop and ordinary windows according to the chosen Tauri
  capabilities
- draggable by the user
- remembers its last position
- supports quick hide and restore
- supports click-through mode when idle if technically stable on target
  platforms
- restores on app relaunch when the user has enabled persistence

The pet should occupy a small consistent footprint and avoid resizing the window
based on text bubble length or transient state.

### In-App Pet Panel

Liberty should add a dedicated pet management area in the main application.
This is the detailed control surface for:

- level and experience
- current stage
- mood and status
- unlocked outfits and accessories
- interaction history
- reward sources
- desktop behavior settings
- do-not-disturb and notification controls

The desktop layer stays lightweight. The main application remains the primary
place for inspection and management.

### Pet Interaction Tone

The pet is intentionally high-interaction. It should proactively react to user
behavior and sometimes ask for attention. Typical behaviors may include:

- celebrating completed transcription or summary steps
- reacting when exports finish
- becoming more excited during active work streaks
- nudging the user after long gaps
- asking for optional quick interaction

To keep this sustainable, the behavior engine must enforce:

- maximum proactive prompts within a time window
- reduced interruption during long-running work
- a user-visible mute or focus switch
- suppression rules during specific task states if needed

## Functional Scope

V1 should include six subsystems.

### 1. Desktop Presence System

Responsibilities:

- create and manage the pet window
- save and restore window position
- control visibility, pinning, drag state, and quick-hide behavior
- coordinate with app lifecycle so the pet can persist independently from the
  main window when allowed

### 2. Growth System

Responsibilities:

- calculate experience gains
- manage level progression
- manage stage transitions
- expose growth summaries to frontend surfaces

This is the long-term progression layer.

### 3. Mood and State System

Responsibilities:

- determine the current short-term emotional state
- map work events and interaction events into mood changes
- control idle, excited, sleepy, attention-seeking, and celebration states

This is the short-cycle expressive layer and should remain separate from level
progression.

### 4. Interaction System

Responsibilities:

- accept direct pet interactions such as tap, pet, feed, or response actions
- apply short-term mood effects
- optionally grant small bonus growth contributions
- enforce per-day or per-window limits where needed

Interactions should enhance progression, not dominate it.

### 5. Event Feedback System

Responsibilities:

- subscribe to meaningful Liberty workflow events
- translate those events into pet reactions and growth triggers
- decouple product workflows from pet logic

This layer is the integration boundary between Liberty's core work features and
the pet feature.

### 6. Pet Management System

Responsibilities:

- expose pet details and settings in the main application
- display stage roadmap and unlock progress
- present unlocked cosmetics and current selection
- provide behavior, mute, and persistence controls

## Growth Design

### Growth Source Priority

Growth sources are ordered by product value:

1. task progression
2. result accumulation
3. usage activity

This means a completed meaningful action should grant more value than passive
presence.

### Recommended Experience Sources

Task progression examples:

- import a meeting file
- start local transcription
- complete local transcription
- generate an AI summary
- complete result refinement milestones

Result accumulation examples:

- export meeting outputs
- save summary variants
- maintain meeting members or other reusable assets that help the workflow

Usage activity examples:

- daily app open
- streak continuation
- minimum active-session threshold

### Progression Structure

V1 should use a simple, inspectable growth model:

- `level`
- `experience`
- `stage`

Suggested logic:

- level increases through cumulative experience
- stage changes at milestone levels
- stage change is the main visual payoff

The exact numbers can be tuned later, but the structure should make stage
changes feel materially more important than individual levels.

### Stage Priority

Growth feedback should prioritize:

1. appearance stage changes
2. mood and state changes
3. outfit and accessory unlocks

That means the first visible reward users care about is "my pet has grown,"
followed by "my pet feels alive right now," with cosmetics as an extension
instead of the core loop.

## State Design

The pet needs a short-lived status model distinct from progression.

Recommended state families:

- idle
- cheerful
- excited
- proud
- needy
- sleepy
- bored

Inputs that influence state:

- recent Liberty actions
- elapsed inactive time
- recent interaction count
- current focus or mute mode
- current growth milestones

State changes should be reversible and cheap. They should not require schema or
UI redesign whenever the behavior set grows.

## Interaction Design

V1 should support a small but coherent interaction set instead of many shallow
actions.

Recommended direct interactions:

- tap or click pet
- short affection interaction
- feed or gift interaction
- acknowledge proactive prompt

Design rules:

- interaction rewards should be capped or strongly diminished after repetition
- interactions should mostly improve mood and only mildly accelerate growth
- missed interactions should not punish the user heavily

## Desktop Behavior Design

### Window Model

The pet should run in a dedicated Tauri window separate from the main Liberty
window. This window should be small, frameless, and optimized for animation and
interaction rather than text-heavy layouts.

### Position and Persistence

Store:

- last screen position
- visibility state
- pinned or always-on-top preference
- click-through preference if enabled

The system should gracefully handle invalid restored positions caused by display
changes.

### Disturbance Controls

V1 should include:

- mute pet interactions
- focus mode or do-not-disturb
- quick hide
- proactive prompt frequency control

Because the user selected high interaction intensity, these controls are not
optional polish. They are part of the core feature contract.

## Architecture

### Frontend

The Vue frontend should host:

- pet management page or panel
- pet settings controls
- visual progress components
- local view models for pet state, stage, and unlock summaries

Recommended areas:

- a dedicated pet view under `src/views/`
- pet-related composables under `src/composables/`
- a service wrapper under `src/services/`

### Tauri and Rust

Rust should host:

- pet persistence models
- experience calculation entry points
- event application logic
- desktop window lifecycle and configuration orchestration

Likely touch points:

- `src-tauri/src/local_db.rs` for storage
- a new pet-focused module such as `src-tauri/src/local_pet.rs`
- `src-tauri/src/main.rs` or adjacent window registration code for pet window
  creation and commands

### Persistence Model

SQLite should store three classes of data.

Permanent data:

- pet profile
- level
- experience
- stage
- unlocked cosmetics
- lifetime counters

Periodic data:

- daily interaction flags
- daily activity summary
- streak data
- temporary mood modifiers with timestamps

Immediate runtime data:

- current mood
- current animation or activity state
- current desktop position cache if also mirrored in storage

The immediate runtime portion may be partially memory-backed, but user-visible
window state and durable pet state should survive app restarts.

## Data Model

The exact schema can evolve, but V1 should account for the following records.

### Pet Profile

- `id`
- `name`
- `level`
- `experience`
- `stage`
- `current_mood`
- `created_at`
- `updated_at`

### Pet Cosmetic Unlock

- `id`
- `pet_id`
- `cosmetic_type`
- `cosmetic_key`
- `unlocked_at`
- `equipped`

### Pet Event Ledger

- `id`
- `pet_id`
- `event_type`
- `event_source`
- `event_value`
- `event_time`
- `metadata`

This ledger is useful for debugging growth, replaying derived state, and
understanding why the pet changed.

### Pet Settings

- `pet_id`
- `desktop_enabled`
- `always_on_top`
- `click_through_enabled`
- `muted`
- `focus_mode_enabled`
- `proactive_level`
- `last_window_x`
- `last_window_y`
- `updated_at`

## Event Integration

The pet system should integrate with real Liberty workflow events rather than
polling generic UI state.

Recommended first event set:

- meeting file imported
- local transcription started
- local transcription completed
- AI summary started
- AI summary completed
- export completed
- reusable management action completed when product value is clear

The event layer should convert these into:

- experience gains
- mood changes
- optional proactive desktop reactions

This mapping should live in Rust or a shared service boundary, not be scattered
across unrelated Vue views.

## Error Handling

The pet feature should fail soft.

- If the pet window cannot open, Liberty should continue running normally
- If pet persistence data is damaged, Liberty should rebuild safe defaults
- If a workflow event cannot be applied to the pet ledger, the workflow itself
  should still complete
- If cosmetics or animation assets are missing, the pet should fall back to a
  default stage appearance

The pet must never block core meeting processing.

## Testing Strategy

### Rust

- schema creation and migration tests for pet tables
- experience calculation tests
- event-to-growth mapping tests
- state transition tests
- settings persistence tests

### Frontend

- pet service integration tests for Tauri command wrappers
- component tests for progress and settings surfaces where practical
- manual verification for layout stability in the pet panel

### Desktop Behavior

Manual validation will be important for:

- pet window visibility and drag behavior
- restore position correctness after relaunch
- always-on-top and hide behavior
- high-interaction rate limiting
- focus mode suppression

## Delivery Recommendation

Even though V1 includes real desktop persistence, implementation should still be
sequenced internally.

1. storage and command surface
2. desktop pet window shell
3. event integration and basic growth
4. mood and interaction loop
5. cosmetics and polish

This keeps the milestone coherent while reducing integration risk.

## Open Decisions For Implementation Plan

The implementation plan should make concrete decisions on:

- the exact desktop window behavior supported consistently on macOS and Windows
- whether click-through ships in V1 or remains conditional by platform
- the initial number of stages
- the initial proactive prompt frequency defaults
- whether the first pet visuals are raster sprite sheets, web animations, or a
  lightweight canvas-based renderer
