# Liberty Shared Types

Versioned JSON Schema files under `schemas/` are the source of truth for cross-language contracts.
Generated files under `src/generated/` must not be edited by hand. Run `pnpm contracts:generate`
after changing a schema and use `pnpm contracts:check` to detect drift.
