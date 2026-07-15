# Independent Runtime Components Design

## Overview

Liberty currently presents Python, FFmpeg, and FunASR models as separate rows,
but the implementation still treats them as one serialized runtime install.
The source selector for Python and FFmpeg also opens a native file picker when
the user selects the system environment. That exposes executable paths to
ordinary users and increases setup complexity.

The runtime subsystem will be redesigned around three independently observable
components:

- Python
- FFmpeg
- FunASR models

Python and FFmpeg support two product-level sources: `Liberty managed` and
`system environment`. Selecting the system environment triggers automatic
detection and validation; users never browse for an executable. Model download
is a separate task that never depends on FFmpeg and may be requested while
Python is still being installed.

## Goals

- Make the source dropdown a pure source choice, without opening a file picker
- Automatically detect and validate installed system Python and FFmpeg
- Keep Python, FFmpeg, and model progress, errors, and retry actions independent
- Allow the model task to be requested without waiting for FFmpeg
- Start model download immediately when a usable Python is available
- Queue model download behind Python only when Python is not ready
- Derive overall local-runtime readiness from component state
- Preserve explicit user choice without silent fallback to another source

## Non-Goals

- Manual executable path entry or a hidden advanced file picker
- Download cancellation or pause/resume in this phase
- Installing packages into an arbitrary system Python
- Supporting multiple active Python or FFmpeg versions
- Replacing the existing FunASR and ModelScope model acquisition mechanism

## Selected Approach

Liberty will use an independent component state machine with a small dependency
coordinator. Each component owns its lifecycle state, progress, error, and log;
Python and FFmpeg additionally own a source and resolved path. The aggregate
runtime state is a projection used by job execution and shell status; it is not
the authority for component UI.

A full generic task graph is unnecessary. The only cross-component dependency
is that the current FunASR model warmup command requires a validated Python.
FFmpeg is not a dependency of model acquisition.

## Settings Model

Add explicit source settings:

- `python_runtime_source`: `managed` or `system`
- `ffmpeg_runtime_source`: `managed` or `system`

The source fields are authoritative. Empty paths must no longer be overloaded
to mean `managed`.

Existing `python_path` and `ffmpeg_path` columns become backend-owned resolution
caches during migration. They are not editable or visible in the UI. When the
source is `system`, detection refreshes the resolved path. When the source is
`managed`, runtime resolution ignores any cached system path.

Runtime source and resolved-path fields are removed from the generic editable
settings payload. They are written only by dedicated runtime commands. The
generic `save_settings` command must merge editable preferences into the stored
record without accepting runtime source or path values from the frontend.

Migration rules:

- existing non-empty Python path -> Python source `system`
- existing empty Python path -> Python source `managed`
- existing non-empty FFmpeg path -> FFmpeg source `system`
- existing empty FFmpeg path -> FFmpeg source `managed`

The migration backfill runs atomically in the schema transaction. On the first
system detection after migration, Liberty tries the stored nonstandard path
first, validates it, and then scans standard candidates if it fails. A failed
cache is cleared only after the scan completes.

## Component State Model

Expose a structured component state for each component:

- `component`: `python`, `ffmpeg`, or `model`
- `source`: `managed` or `system`, present only for Python and FFmpeg
- `availability`: `unavailable`, `ready`, or `unsupported`
- `active_artifact`
  - `generation_id`
  - `artifact_version`
  - `resolved_path`, backend-only and used for diagnostics/job snapshots
- `operation`
  - `kind`: `idle`, `detecting`, `waiting_for_python`, `downloading`,
    `installing`, `validating`, or `failed`
  - `generation`: monotonically increasing integer
  - `phase`: stable machine-readable phase name
  - `progress`: integer from 0 to 100 when measurable, otherwise `null`
  - `last_error`
- `updated_at`

Availability and operation are deliberately separate. A redownload can run or
fail while the previous active generation remains ready for jobs. When no
active artifact exists, the same operation states render an unavailable row.
`not detected` is represented as unavailable availability plus a failed
operation with typed error code `system_not_detected`.

The persisted state must be component-scoped. A Python transition must not
overwrite FFmpeg or model progress and errors. Logs must also be component
scoped instead of sharing one parse-dependent install log.

The public DTO is:

```text
RuntimeStatus {
  platformId,
  runtimeVersion,
  python: RuntimeComponentState,
  ffmpeg: RuntimeComponentState,
  models: RuntimeComponentState,
  shellReady,
  shellStatus,
}
```

`models.source` is absent and models are always Liberty-managed. Rust owns the
serialized enum strings and TypeScript mirrors the same closed union types.
All state changes run through one Rust transition reducer used by commands,
workers, and startup reconciliation.

### Normative Transitions

| Trigger | Availability before operation | Operation transitions and availability result |
| --- | --- | --- |
| Select system source | any | clear active selection for new jobs; `detecting` -> activate detected path and `ready`, or `failed(system_not_detected)` and `unavailable` |
| Select managed source | any | `idle`; activate a valid current managed generation and `ready`, otherwise `unavailable` |
| Install/redownload managed component | unavailable or ready | `downloading` -> `installing` -> `validating` -> activate new generation and `ready`, or `failed` while retaining any previous ready generation |
| Request/redownload models with Python ready | unavailable or ready | `installing(acquiring_models)` -> `validating` -> activate new generation and `ready`, or `failed` while retaining any previous ready generation |
| Request/redownload models without Python | unavailable or ready | `waiting_for_python`; automatically continues when Python becomes ready while any previous model generation remains available |
| Retry system detection | unavailable | `detecting` -> activate detected path and `ready`, or `failed(system_not_detected)` |
| Unsupported platform/component | any | availability `unsupported`, operation `idle` |
| Manifest/model version changes | ready | old active generation remains available to existing job snapshots; new jobs see unavailable until the selected current generation is installed |

Starting a new operation resets progress, phase, and error for that component
and increments `operation.generation`. A worker may update or commit state only
when its captured generation and source still match the current record.

On application startup, reconciliation follows these rules:

- system `detecting` restarts automatic detection
- Python/FFmpeg `downloading`, `installing`, or `validating` becomes operation
  `failed` with an interrupted-operation error; a valid active generation keeps
  availability ready
- model `waiting_for_python` remains waiting and resumes when Python is ready
- model `installing` or `validating` becomes operation `failed`; partial
  staging content is removed before retry
- ready availability is retained only when artifact version, completion marker, and
  component validation still pass
- invalid or corrupt managed artifacts are unavailable with a failed operation,
  never ready

## Source Selection and Detection

### Python

Selecting `system environment` performs an asynchronous scan of:

- executables available through `PATH`
- standard macOS Homebrew, MacPorts, and system locations
- standard Windows Python launcher and installation locations

Candidates are tried in deterministic order. Liberty validates the complete
Python capability required by the local runner, not only `--version`. The first
candidate that passes validation becomes the resolved system Python.

Each candidate has a 90-second validation timeout. Timed-out child processes
and descendants are terminated before scanning continues. A full scan has a
bounded candidate count and cannot remain in `detecting` indefinitely.

If no candidate passes, the row displays `System Python not detected` with a
`Detect again` action. Liberty does not silently switch to managed Python.

### FFmpeg

FFmpeg follows the same flow. Detection checks `PATH` and standard platform
locations, executes `ffmpeg -version`, and selects the first valid candidate.

Each FFmpeg candidate has a 15-second timeout and the same child-process cleanup
rule.

This work adds a cross-platform timed-process primitive in `process_utils`.
Unix commands run in a dedicated process group that receives termination and,
after a grace period, a kill signal. Windows commands run in a Job Object that
is terminated on timeout. Detection, validation, and installer subprocesses use
this primitive; merely hiding a Windows console window is not process cleanup.

If detection fails, the row displays `System FFmpeg not detected` with a
`Detect again` action. Liberty does not silently switch to managed FFmpeg.

### Managed Source

Selecting `Liberty managed` does not start a download automatically. The row
immediately reflects whether the managed artifact already exists and provides
`Download`, `Redownload`, or `Repair` as appropriate.

## Independent Task Flow

### Python Task

Managed Python owns download, checksum verification when supplied by the
manifest, extraction, dependency installation, and validation. Its busy state
disables only Python source and Python actions.

When Python becomes ready, the coordinator checks whether the model operation
is `waiting_for_python`. If so, model acquisition starts automatically.

### FFmpeg Task

Managed FFmpeg owns download, checksum verification when supplied by the
manifest, extraction, and validation. It may run concurrently with Python and
the model task. Its busy state disables only FFmpeg source and FFmpeg actions.

### Model Task

The model download button is enabled whenever a download source is selected,
regardless of FFmpeg state and regardless of whether Python is currently being
installed.

The existing FunASR model acquisition uses Python `AutoModel` and ModelScope,
so the executable step has a real Python dependency. The coordinator checks
only the selected Python component's availability, never its concurrent
operation kind:

- Python availability is `ready` -> start model acquisition immediately using
  a lease on that active generation, even when a Python redownload is running
- Python availability is `unavailable` -> set the model operation to
  `waiting_for_python`; start automatically when availability becomes `ready`
- Python availability is `unsupported` -> fail the model operation with a typed
  unsupported-dependency error

The model task never waits for FFmpeg.

### Model Storage and Completion

The runtime manifest gains an explicit `modelSetVersion` and model profile. The
initial profile is the current Paraformer set:

- `paraformer-zh`
- `fsmn-vad`
- `ct-punc`
- `cam++`

The acquisition script uses explicit ModelScope model IDs from the runtime
manifest and downloads snapshots into a new immutable generation directory.
It returns a structured JSON mapping from each required role to its resolved
local directory. It does not use log text as a completion protocol.

Validation passes those explicit local directories to `AutoModel`, sets
`MODELSCOPE_OFFLINE=1`, `HF_HUB_OFFLINE=1`, and disables updates. Validation is
not allowed to pass model IDs or aliases that could trigger a network lookup.
If the installed SDK cannot honor offline mode, validation fails rather than
repairing the cache from the network.

After local-path validation exits successfully, Liberty verifies that every
required directory contains non-empty model files and writes a
`ready.json` marker containing model-set version, profile, explicit model IDs,
resolved relative directories, completion timestamp, and operation generation.
Only then may the generation become active.

Model readiness requires a matching marker and a successful offline warmup
validation using the marker's explicit local paths. Directory existence alone
is never treated as readiness. A model-set version change requires a new
generation.

`AutoModel` does not expose reliable byte progress. During model acquisition,
`progress` is `null` and `phase` drives an indeterminate progress bar with
labels such as `waiting_python`, `acquiring_models`, and `validating_models`.
No percentage is inferred from text logs.

## Concurrency and Leases

Each component has an independent operation guard. A Python, FFmpeg, and model
operation may run concurrently when their dependency rules allow it. Repeated
requests for the same busy component are idempotent.

Workers capture both `operation.generation` and source where applicable. Before activating files
or persisting success/failure, they compare both values with current state. A
stale worker cleans its staging directory and exits without changing current
state.

Model acquisition obtains a read lease on the resolved Python identity for the
duration of the acquisition and validation process. That lease keeps the exact
Python generation or system path alive, but it does not block building or
activating a newer Python generation.

Local jobs hold read leases on Python, FFmpeg, and the active model version for
their full child-process lifetime. New generations can be installed and
activated while a job uses an older immutable generation. Garbage collection
deletes inactive generations only after their lease count reaches zero. FFmpeg
operations never acquire or wait on a model lease.

Model acquisition therefore waits only for a usable Python. It never waits for
FFmpeg or an active transcription job. Existing jobs keep their model snapshot
while a newly completed generation becomes active for subsequent jobs.

## Managed Artifact Layout and Activation

Managed artifacts never overwrite a fixed live directory. Each successful or
in-progress install has an immutable generation directory:

```text
runtime/components/python/generations/<generation-id>/
runtime/components/ffmpeg/generations/<generation-id>/
runtime/components/models/generations/<generation-id>/
```

Workers create a fresh generation directory, write files, validate them, write
`ready.json`, and fsync the marker before activation. Activation is a SQLite
transaction that updates the component's `active_generation_id` and ready
metadata. It does not rename over or delete the previous directory and is
portable when the previous generation is open on Windows.

Startup reconciliation scans only referenced active generations and unfinished
operation generations. A generation without a valid marker is never activated.
An unreferenced incomplete generation is removed. A committed active pointer
with a missing or invalid marker is rolled back to unavailable and reported as
corrupt. Python and FFmpeg markers record manifest identity, executable relative
path, and validation timestamp; model markers record the model metadata defined
above.

## Backend Commands

Replace the single install-oriented frontend contract with component commands:

- `get_runtime_status`
- `set_runtime_component_source(component, source)`
- `detect_runtime_component(component)`
- `install_runtime_component(component)`
- `get_runtime_component_log(component)`

`set_runtime_component_source` persists the explicit source and returns the
updated component state. For `system`, it launches detection. For `managed`, it
inspects the managed artifact without downloading it.

Valid command combinations are closed and enforced by Rust:

- set source: Python or FFmpeg only
- detect: Python or FFmpeg only, and only for selected `system` source
- install: Python or FFmpeg only for selected `managed` source; models always
- component log: Python, FFmpeg, or models

Invalid combinations return a typed command error without mutating state.

The backend guards each component independently. Repeated calls for the same
busy component are idempotent, while calls for another component may run in
parallel subject only to the Python leases described above.

## Aggregate Readiness and Job Resolution

Shell-level local runtime readiness is true only when:

- selected Python source resolves to a validated Python
- selected FFmpeg source resolves to a validated FFmpeg
- FunASR models are ready

FFmpeg is globally required by the current local transcription workflow. If a
future job type does not require it, that job must use a separate job-specific
capability check rather than weakening shell readiness.

Job execution resolves each component strictly from its selected source. It
must never auto-detect a system fallback when the user selected managed, or use
a managed fallback when the user selected system.

The existing aggregate `ManagedRuntimeState` may remain temporarily as a
compatibility projection, but component state is the source of truth. The
projection must be derived after every component transition.

## Frontend Interaction

Each Python and FFmpeg row contains:

- source dropdown
- component progress and status
- component-specific action button

Behavior:

- choose `System environment` -> show `Detecting`, then `Ready` or `Not detected`
- no native file picker is opened
- detected paths are not displayed in the normal settings UI
- choose `Liberty managed` -> inspect managed status; do not start downloading
- source controls remain usable while other components are busy
- action buttons disable only when their own component is busy

The model row has no source dropdown. Its download action is independent of
FFmpeg and can wait for Python while Python is unavailable.

The bottom-level `Download environment and models` action should be removed or
reframed as an explicit `Download missing components` convenience action. It
must dispatch independent component tasks rather than recreate a serialized
installer.

## Error Handling

- Detection failures stay local to the selected component
- Download and validation failures retain the successful state of other
  components
- Switching source invalidates only the affected component resolution
- A stale system path triggers re-detection on startup or status refresh
- A model operation waiting for Python survives Python failure and resumes after
  Python repair
- Application restart converts interrupted component work into a retryable
  failed or waiting state without corrupting completed components
- Candidate timeouts always terminate spawned process trees
- Stale operation generations never activate files or overwrite current errors
- Runtime mutations create and activate immutable generations instead of
  replacing files in use; inactive generations are collected after leases end

## Version and Integrity Rules

Every ready managed component stores an `artifact_version` derived from the
bundled runtime manifest version and that component's asset identity. Models use
the explicit `modelSetVersion`.

When a manifest checksum is present, Liberty must verify it before extraction.
The current manifest contains rolling FFmpeg URLs and may omit checksums, so
this redesign does not claim cryptographic integrity where the manifest cannot
provide it. Pinning release URLs and publishing non-empty checksums is a
separate release-hardening task. Version reconciliation still uses the bundled
manifest identity and never silently treats a changed manifest as current.

## Testing Strategy

### Rust

- settings migration from path-derived source to explicit source
- generic settings writes cannot modify backend-owned source or resolved paths
- deterministic system candidate ordering
- independent Python and FFmpeg detection success/failure
- per-component busy guards allow different components concurrently
- table-driven reducer coverage for every normative transition
- stale operation generations cannot persist progress, failure, or activation
- model request waits without Python and resumes after Python readiness
- model redownload without Python retains an existing active model generation
- model acquisition starts from the active Python generation while a Python
  redownload is concurrently installing a replacement generation
- FFmpeg activity never blocks model acquisition
- active transcription leases never block acquisition or activation and keep old
  generations alive until release
- generation activation is transactional and recovers from missing markers,
  partial files, and pointer/marker mismatch
- partial model caches without a matching marker are unavailable
- offline model validation uses only explicit local paths and fails when a
  required local file is missing
- timeout tests verify termination of the candidate process and descendants
- startup reconciliation covers every transient operation kind
- aggregate readiness derives from all selected component states
- job resolution honors selected source without fallback

### Frontend

- source dropdown never invokes the native file picker
- system selection renders detecting, ready, and not-detected states
- one busy component does not disable other component actions
- model action remains enabled while FFmpeg downloads
- model operation `waiting_for_python` renders `Waiting for Python`
- ready availability remains visible while a redownload runs or fails
- responsive settings rows do not overflow

### Integration

- managed Python and FFmpeg install concurrently
- system Python plus managed FFmpeg and models
- managed Python plus system FFmpeg and models
- model request before managed Python finishes
- restart during each transient operation preserves other completed components
- activate new managed generations while an existing job uses old generations

## Acceptance Criteria

- Users never need to know or select executable paths
- Selecting a system source automatically detects and validates that component
- Python, FFmpeg, and model rows show independent status and progress
- Model download can be requested while Python or FFmpeg is busy
- Model acquisition begins immediately with a valid Python and otherwise queues
  only on Python
- Local jobs start only when the selected sources and models are all ready
