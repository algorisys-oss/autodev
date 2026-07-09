# Handoff

Rolling snapshot of the current state. Read this first when picking the project back up.
Updated as the final step of every task (LOOPS XXVI).

## Where things stand

- **Last task:** Phase 0 — foundation. Complete.
- **Phases done:** 0 (foundation).
- **Next up:** Phase 1 — workspaces & projects (see `PLAN.md`).

## What runs

- `./dev.sh dev` — launch the app (hot reload). Boots to the AutoDev window; the UI
  reads `app_info` and settings from the Rust core over typed commands.
- `./dev.sh test` — Rust `cargo test` (2 disk-state tests) + Vitest (3 ipc tests). Green.
- `./dev.sh lint` — eslint + tsc + clippy + rustfmt check. Green.
- `./dev.sh verify` — everything CI runs. Green.

## The one command to verify

```
./dev.sh verify
```

## Known environment note

This machine runs inside snap-packaged VSCode. A native GTK/WebKit binary launched from
that shell crashes with `undefined symbol __libc_pthread_init` because snap injects its
`core20` libs and GTK paths. `dev.sh` strips those before launching (`scrub_snap_env`),
so `./dev.sh dev` works. Running the raw binary or `npm run tauri dev` directly from a
snap shell will hit the error; go through `dev.sh`.

## Open decisions / notes

- `codex` CLI is installed (confirmed by the user), so Phase 3 can integration-test both
  Claude Code and Codex backends for real.
- Later phases with external deps (voice = whisper, browser = Playwright, loop engine =
  LLM API) will ship a pluggable backend + a stub where a model/key is absent, documented
  here when reached.
