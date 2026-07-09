# Handoff

Rolling snapshot of the current state. Read this first when picking the project back up.
Updated as the final step of every task (LOOPS XXVI).

## Where things stand

- **Last task:** Phase 2 — single agent session (PTY + xterm.js). Complete.
- **Phases done:** 0 (foundation), 1 (workspaces & projects), 2 (single agent session).
- **Next up:** Phase 3 — multi-agent orchestration (agent grid, live status, Codex
  adapter, kill-all on quit). See `PLAN.md`.

## What runs

- `./dev.sh dev` — launch the app. Sidebar creates workspaces + adds project dirs. Each
  project has a “▶ Claude” button that spawns a real Claude Code agent in that directory
  in a PTY, streamed into an xterm terminal you can type into. Kill button ends it.
- `./dev.sh test` — Rust `cargo test` (11 tests, incl. PTY mock-agent) + Vitest (11).
- `./dev.sh lint` — eslint + tsc + clippy (-D warnings) + rustfmt check. Green.
- `./dev.sh verify` — everything CI runs. Green.

Agent backends are pluggable (`claude`, `codex`, `mock`). Tests drive the `mock` backend
so the spawn→stream→write→exit path is verified without real CLI auth.

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

## Unattended-run guards

- **Battery watchdog:** `~/.autodev/battery-watchdog.sh` runs detached. At BAT0 ≤5% while
  discharging it logs the time to `~/.autodev/lowbatt.log` and suspends the laptop.
- **Shutdown on completion:** after all phases are pushed and the tree is clean, the run
  powers the laptop off (`systemctl suspend`/`poweroff`).
