# Implementation Tracking

Audit trail from decision to code (LOOPS XXV). Newest first.

## Phase 0 — Foundation — COMPLETE

**Decided:** Desktop app on Tauri (Rust core) + SolidJS + TypeScript (user picked Rust
Tauri; SolidJS recommended for fine-grained reactivity across many live terminals).
Full-ecosystem scope over 10 phases (see `PLAN.md`). Naming: lowercase-hyphenated for
frontend files/folders, scripts, configs; snake_case for Rust module files (hyphens are
invalid in Rust module names — LOOPS XXXVI, use the target's idioms).

**Built:**
- `src-tauri/` Rust core, crate `autodev` / lib `autodev_lib`.
  - `error.rs` — `AppError` (thiserror) serializing to a string across the boundary.
  - `state.rs` — `AppSettings` persisted at `~/.autodev/settings.json`; dir-parameterized
    load/save so tests use temp dirs with no global env races.
  - `commands.rs` — `app_info`, `get_settings`, `set_settings`.
  - `lib.rs` — registers the three commands.
- Frontend: `src/lib/ipc.ts` (typed wrappers, the single shared contract), `src/App.tsx`
  (shell exercising the round-trip), `src/App.css`.
- Tooling: `vitest.config.ts`, `eslint.config.js`, `dev.sh`, `.github/workflows/ci.yml`.

**Status:** complete. `./dev.sh verify` green. App boots (snap env scrub required, handled
by `dev.sh`).

**Deliberate deferrals:**
- ESLint Solid plugin omitted (fragile config paths across versions); tsc + tseslint cover
  the frontend for now. Revisit if Solid-specific lint rules become worthwhile.
- No SQLite yet; flat JSON is enough (LOOPS III). Introduce only when it hurts.
