# Implementation Tracking

Audit trail from decision to code (LOOPS XXV). Newest first.

## Phase 2 — Single agent session — COMPLETE

**Decided:** The load-bearing PTY core is a Tauri-independent function
(`spawn_session`) that takes `on_output`/`on_exit` callbacks, so tests drive it directly
with real PTYs and the Tauri command layer just supplies event-emitting callbacks. Added
a `mock` backend (runs any command) to test spawn/stream/write/exit deterministically in
CI without Claude/Codex auth — de-risking the riskiest phase on the mechanism itself
(LOOPS XXXVI). PTY bytes cross to the frontend base64-encoded so terminal escape
sequences survive intact.

**Built:**
- `src-tauri/src/agent.rs` — `AgentBackend`, `AgentOptions`, `build_command`,
  `spawn_session`, `AgentSession` (write/resize/kill), `AgentManager` (+kill_all). 4 tests.
- `commands.rs` — 6 agent commands + `agent://output`/`agent://exit` events.
- Frontend: `src/components/terminal-pane.tsx` (xterm), `src/lib/bytes.ts` (+test),
  agent ipc wrappers, App launcher/kill UI.

**Status:** complete. `./dev.sh verify` green (11 Rust + 11 frontend). Real app boot
confirmed: `Running target/debug/autodev`, no crash, terminal integrated.

**Deliberate deferrals / known gaps (address in Phase 3):**
- Small race: `agent://output` listeners attach just after spawn, so the first few
  startup bytes could be missed. Fix with a per-agent output buffer/replay in the
  Phase 3 session store.
- Agents are not yet killed on window close / app quit — Phase 3 wires `kill_all` to the
  exit hook (acceptance test there: no orphaned processes).
- Status is running/exited only; idle/waiting detection comes with the Phase 3 grid.

## Phase 1 — Workspaces & projects — COMPLETE

**Decided:** All workspace/project logic and persistence live in the Rust core (unit
tested with temp dirs); the frontend is a thin reactive store + sidebar. Directory
picking uses `tauri-plugin-dialog` (added Rust plugin + `dialog:allow-open` capability +
npm `@tauri-apps/plugin-dialog`). Project name = directory basename; paths canonicalized
to absolute. `@`-mention matching is fuzzy (normalize away case/space/hyphen).

**Built:**
- `src-tauri/src/workspace.rs` — model, disk store, CRUD, mention resolver, 5 tests.
- `commands.rs` — 6 workspace commands wrapping the store against the real data dir.
- `error.rs` — added `NotFound`, `Conflict`.
- `src/lib/workspace-store.ts` (+ test), `src/components/workspace-sidebar.tsx`,
  rewritten `src/App.tsx`/`App.css` into a two-pane layout.

**Status:** complete. `./dev.sh verify` green (7 Rust + 8 frontend tests).

**Deliberate deferrals:**
- Mention file-listing does not yet parse `.gitignore` (uses a fixed ignore list). Fine
  until it proves too coarse.
- `resolve_mention` is wired as a command but not yet surfaced in the UI; that lands with
  the Phase 4 prompt composer.

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
