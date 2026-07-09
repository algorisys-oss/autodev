# Handoff

Rolling snapshot of the current state. Read this first when picking the project back up.
Updated as the final step of every task (LOOPS XXVI).

## Where things stand

- **Last task:** Phase 4 — prompt composer. Complete.
- **Phases done:** 0–4 (foundation, workspaces, single agent, orchestration, composer).
- **Next up:** Phase 5 — git worktree isolation + merge-back. See `PLAN.md`.

## What runs

- `./dev.sh dev` — launch the app. Sidebar manages workspaces + project dirs. A prompt
  composer drives launches: type a task, `@mention` projects to attach them as context
  (`--add-dir`), set a difficulty 1–10 (auto-suggests agent count + plan/ultrathink),
  toggle plan/bypass/ultrathink, pick backend + working dir, and fan out to N agents.
  Prompt history persists (`~/.autodev/prompts.json`). The agent grid shows every session
  with a live status dot; click to focus its terminal. “Kill all” and window-close kill
  every agent (no orphans). Per-agent output logs to `~/.autodev/logs/<id>.log`.
- `./dev.sh test` — Rust `cargo test` (13 tests) + Vitest (25).
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
