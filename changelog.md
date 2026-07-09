# Changelog

Newest first. Functional changes only (LOOPS XXIV).

## [2026-07-10]

### Phase 2 — Single agent session
- Rust `agent` module: launch a coding-agent CLI in a real PTY (`portable-pty`), stream
  output, accept input, resize, kill. `AgentManager` tracks sessions with a `kill_all`.
- Backends behind one builder: `claude` (`--permission-mode plan`,
  `--dangerously-skip-permissions`, `--model`), `codex`, and `mock` (arbitrary command,
  for tests/CI without real auth). Flags verified against the installed CLIs.
- Commands: `agent_spawn`/`agent_write`/`agent_resize`/`agent_kill`/`agent_list`/
  `agent_kill_all`. Output/exit stream to the frontend as `agent://output` (base64 to
  preserve escape bytes) and `agent://exit` events. `AppError::Pty` added.
- Frontend: `TerminalPane` (xterm.js + fit addon) wired to the events and commands; App
  gained a per-project “▶ Claude” launcher and a kill control. `base64ToBytes` helper.
- Tests: +4 Rust (real-PTY mock-agent spawn/stream/input, manager kill-all) and +3
  frontend (base64 byte fidelity). Total 11 Rust + 11 frontend. App boot re-verified.

### Phase 1 — Workspaces & projects
- Rust `workspace` module: `Workspace`/`Project`/`WorkspaceStore` persisted to
  `~/.autodev/workspaces.json`; create/delete workspace, add/remove project (basename
  naming, absolute-path canonicalization, duplicate + missing-dir rejection).
- `@`-mention resolver: fuzzy project match (case/space/hyphen-insensitive) plus a
  capped, sorted file listing that skips `node_modules`, `.git`, `target`, etc.
- Commands: `list_workspaces`, `create_workspace`, `delete_workspace`, `add_project`,
  `remove_project`, `resolve_mention`. Added `NotFound`/`Conflict` error variants.
- Frontend: reactive `workspace-store`, `WorkspaceSidebar` (create workspace, native
  folder picker via `tauri-plugin-dialog`, per-project remove), two-pane App shell.
- Tests: +5 Rust (workspace CRUD, unique ids, mention resolution) and +5 frontend
  (store orchestration with a fake ipc). Total 7 Rust + 8 frontend.

### Phase 0 — Foundation
- Scaffolded the desktop app: Tauri v2 (Rust core) + SolidJS + TypeScript + Vite.
- Renamed the crate/app to `autodev` (`com.algorisys.autodev`), licensed AGPL-3.0-only.
- Rust core modules: `error` (command-boundary error type), `state` (disk-backed
  `AppSettings` under `~/.autodev/`), `commands` (`app_info`, `get_settings`,
  `set_settings`). Files: `src-tauri/src/*.rs`.
- Typed command contract mirrored in the frontend at `src/lib/ipc.ts`; settings structs
  use camelCase on the wire (`serde(rename_all)`).
- App shell (`src/App.tsx`) reads `app_info` + settings on mount and round-trips a theme
  change back to the core, proving the bridge both directions.
- Test harness: Vitest (`src/lib/ipc.test.ts`, 3 tests) + `cargo test` (2 disk-state
  tests). Lint: eslint + tsc + clippy + rustfmt.
- `dev.sh`: single developer entry point (setup/dev/build/test/lint/verify) with a snap
  environment scrub so the app launches from snap-packaged VSCode.
- CI workflow at `.github/workflows/ci.yml`.
- Docs: `README.md`, `handoff.md`, `implement.md`.
