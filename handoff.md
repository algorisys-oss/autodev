# Handoff

Rolling snapshot of the current state. Read this first when picking the project back up.
Updated as the final step of every task (LOOPS XXVI).

## Where things stand

- **Last task:** Phase 3 — multi-agent orchestration. Complete.
- **Phases done:** 0–3 (foundation, workspaces, single agent, multi-agent orchestration).
- **Next up:** Phase 4 — prompt composer (@-mention, difficulty→agents heuristic, mode
  toggles: plan/bypass/ultrathink/effort). See `PLAN.md`.

## What runs

- `./dev.sh dev` — launch the app. Sidebar manages workspaces + project dirs. Each project
  has “▶ Claude” and “▶ Codex” launchers; spawn many at once across projects. An agent
  grid shows every session with a live status dot (running/idle/exited); click a card to
  focus its terminal. “Kill all” stops everything; closing the window kills all agents so
  none are orphaned. Per-agent output is also logged to `~/.autodev/logs/<id>.log`.
- `./dev.sh test` — Rust `cargo test` (11 tests, incl. PTY mock-agent) + Vitest (17).
- `./dev.sh lint` — eslint + tsc + clippy (-D warnings) + rustfmt check. Green.
- `./dev.sh verify` — everything CI runs. Green.

Agent backends are pluggable (`claude`, `codex`, `mock`). Tests drive the `mock` backend
so the spawn→stream→write→exit path is verified without real CLI auth. The frontend agent
store buffers each agent's output, so switching focus replays full scrollback (this also
fixed the Phase 2 startup-output race).

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
