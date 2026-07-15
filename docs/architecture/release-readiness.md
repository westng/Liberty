# Release Readiness

## Required Platforms

Every release candidate must account for these desktop targets:

| Platform | Platform ID | Rust Target | Validation |
| --- | --- | --- | --- |
| macOS Apple Silicon | `darwin-aarch64` | `aarch64-apple-darwin` | primary |
| macOS Intel | `darwin-x64` | `x86_64-apple-darwin` | primary |
| Windows x64 | `windows-x64` | `x86_64-pc-windows-msvc` | primary |
| Windows x86 | `windows-x86` | `i686-pc-windows-msvc` | extended |

Windows x86 remains a supported target, but it must be validated independently
because runtime dependencies and model performance differ from x64.

## Automated Checks

Run before release:

```bash
pnpm check
```

This includes:

- frontend type checking and production build
- release metadata consistency
- platform matrix consistency
- Tauri security baseline checks
- Rust formatting check
- Rust tests
- Rust clippy with warnings denied

## Manual Release Checks

- Confirm `runtime-manifest.json` has explicit entries for each supported platform ID.
- Confirm configured Python and ffmpeg asset URLs are reachable with a cheap HEAD or byte-range probe before publishing them to users.
- Confirm placeholder or unavailable runtime sources are not advertised in the client.
- Confirm macOS Intel and Apple Silicon artifacts are separate.
- Confirm Windows x86 and x64 artifacts do not share runtime assumptions.
- Confirm Windows x86 unsupported runtime-download messaging is still accurate when the platform remains listed.
- Confirm the Environment & Models settings flow can list configured download sources and surface runtime-install logs.
- Confirm diagnostics report shows the expected current platform and schema version.
- Confirm no API keys, tokens, or local source paths appear in exported diagnostics.
- Confirm desktop-pet diagnostic log export redacts sensitive paths and includes useful runtime/resource state.
