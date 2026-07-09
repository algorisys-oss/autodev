# Changelog

Newest first. Functional changes only (LOOPS XXIV).

## [2026-07-10]

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
