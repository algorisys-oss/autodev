# Handoff

Rolling snapshot of the current state. Read this first when picking the project back up.
Updated as the final step of every task (LOOPS XXVI).

## Where things stand

- **Last task:** Hardening — loop auto-advance + hands-off auto-run. Complete. Also fixed
  dark-mode text-input contrast (loop textareas + a global `::placeholder` rule).
- **Phases done:** 0–9. **All planned phases are built.** The full ecosystem from `PLAN.md`
  is implemented: workspaces, multi-agent orchestration, composer, worktrees, voice,
  screenshot, browser handoff, and the Planner/Generator/Evaluator loop.
- **Loop auto-advance:** the loop now parses each role agent's terminal output to advance
  itself. On planner exit the contract is filled and the loop moves to generating; on
  generator exit it moves to evaluating; on evaluator exit the `N. PASS/FAIL` verdicts are
  parsed and graded (pass/retry/fail). Parsing lives in Rust (`loop_engine::strip_ansi`,
  `parse_contract`, `parse_verdicts`) and runs in `loop_apply_planner`/`loop_apply_evaluator`
  over the agent's `~/.autodev/logs/<id>.log`. The manual textarea/checkboxes remain as an
  editable fallback and appear whenever a parse fails. Tick **Auto-run** in the composer for a
  fully hands-off run: the loop then spawns each next role itself (create → planner → generator
  → evaluator → retry/pass/fail), bounded by `max_iterations`; a parse failure stops the chain.
  Auto-run is off by default.
- **Next up:** embed the real per-iteration git diff in the evaluator prompt (needs a recorded
  base commit); a settings UI for the pluggable commands; richer status detection. See
  `implement.md`.

Voice and screenshot both use pluggable shell commands in `~/.autodev/settings.json`
(each a template with a `{file}` placeholder):
- `transcribeCommand` — e.g. `whisper-cli -f {file} -otxt -of {file} && cat {file}.txt`
- `screenshotCommand` — e.g. `grim {file}` (Wayland), `scrot {file}` (X11),
  `screencapture {file}` (macOS)
- `browserCommand` — optional; a script (e.g. Playwright) that reads the handoff from
  `{file}`. Without it, generate + copy the handoff into a browser AI manually.
Without them, the mic / screenshot / run buttons return a clear "not configured" error.

## What runs

- `./dev.sh dev` — launch the app. Sidebar manages workspaces + project dirs. A prompt
  composer drives launches: type a task, `@mention` projects to attach them as context
  (`--add-dir`), set a difficulty 1–10 (auto-suggests agent count + plan/ultrathink),
  toggle plan/bypass/ultrathink, pick backend + working dir, and fan out to N agents.
  Prompt history persists (`~/.autodev/prompts.json`). The agent grid shows every session
  with a live status dot; click to focus its terminal. “Kill all” and window-close kill
  every agent (no orphans). Per-agent output logs to `~/.autodev/logs/<id>.log`.
- Isolate: tick “Isolate (worktree)” in the composer to run each agent in its own
  `git worktree` (own branch), so parallel agents never collide. The focused agent's bar
  shows the branch with Merge / Remove actions (merge refuses a dirty target).
- Screenshot: 📷 in the composer captures the screen, opens an annotator (arrow/box/pen
  + colors + undo), and attaches the annotated PNG to the next launch (Codex gets it via
  `-i`; Claude gets the path referenced in the prompt).
- Browser handoff: 🌐 in the composer opens a modal to describe a web task and generate a
  structured handoff prompt; copy it into a browser AI (Comet-style) or run a configured
  `browserCommand` (e.g. Playwright) on it.
- Loops (header tab): create an autonomous loop from a spec; run Planner → Generator →
  Evaluator (each as a real agent in the project dir); set the contract, grade criteria,
  and the loop advances (pass / retry / fail). State lives on disk under
  `~/.autodev/loops/<id>/` (state.json, contract.md, feature-list.json, progress.md, log.md).
- `./dev.sh test` — Rust `cargo test` (39 tests) + Vitest (32).
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
