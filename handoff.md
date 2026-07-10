# Handoff

Rolling snapshot of the current state. Read this first when picking the project back up.
Updated as the final step of every task (LOOPS XXVI).

## Where things stand

- **Last task:** Hardening pass — evaluator diff wiring, a settings UI, and richer agent
  status. Complete. Earlier in the session: loop auto-advance + hands-off auto-run, and a
  dark-mode text-input contrast fix. README now documents building a standalone executable.
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
- **Evaluator diff:** the loop records the project HEAD as `base_commit` when it enters
  Generating and feeds `git diff <base>` (the round's work-tree changes) into the evaluator
  prompt. Non-repo project dirs → empty diff.
- **Settings UI:** ⚙ in the header opens a modal to edit theme, default effort, and the
  transcribe/screenshot/browser command templates (previously hand-edited in settings.json).
- **Agent status:** dots distinguish `running` / `idle` / `waiting` (blocked on a prompt) /
  `exited` / `error` (non-zero exit). `waiting` is now silence-derived: any output ⇒ `running`,
  and only when an agent goes quiet does `tick()` classify its tail as `waiting` vs `idle`.
  Detection strips ANSI and recognises Claude/Codex approval menus, y/n, and "press enter".
- **Signing:** release bundles ship unsigned (certs are secrets). The README "Code signing &
  notarization" section has the exact macOS/Windows hooks to enable it in CI.
- **Next up:** nothing outstanding from the original gap list. Remaining honest caveats: the
  `waiting` heuristic is still pattern-based (won't catch every exotic prompt), and a live loop
  run depends on the agent models following the tightened prompts. See `implement.md`.

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
- `./dev.sh test` — Rust `cargo test` (42 tests) + Vitest (40).
- `./dev.sh build` — release build + platform bundle (standalone binary + AppImage/deb/rpm on
  Linux). See the README "Building a standalone executable" section.
- Release automation: push a `v*` tag → `.github/workflows/release.yml` builds Linux/macOS/
  Windows via `tauri-action` and uploads installers to a draft GitHub release. README has a
  Download table and a "Publishing a release" walkthrough.
- `./dev.sh lint` — eslint + tsc + clippy (-D warnings) + rustfmt check. Green.
- `./dev.sh verify` — everything CI runs. Green.

Workspaces: create one (name) then add existing folders via **+dir** (native folder picker),
or use **Open folder as workspace…** to do both in one pick (workspace named after the folder).
State is in `~/.autodev/workspaces.json` (metadata only — project files are never copied).

Releases: `./dev.sh release X.Y.Z` bumps both manifests, tags `vX.Y.Z`, and pushes; the tag
drives `.github/workflows/release.yml` to build and upload installers to a draft GitHub release.

Agent backends are pluggable (`claude`, `codex`, `antigravity`, `mock`). Antigravity runs
Google's `agy` CLI (`-i` initial prompt, `-m` model, `--add-dir`, `--dangerously-skip-permissions`);
its flags follow Google's published guide and aren't verified against a local install here, and
`agy` has no plan-mode/image flag so those UI toggles are no-ops for it. Tests drive the `mock` backend
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
