# Changelog

Newest first. Functional changes only (LOOPS XXIV).

## [2026-07-10]

### Demo — real screen recording of a 3-agent build, then the app running
- Added `demo/autodev-multi-agent-demo.mp4`: a real, end-to-end screen recording of the desktop
  app — set 3 agents + Isolate (worktree), launch, and three real `claude` agents each build a
  to-do app (`index.html`/`style.css`/`app.js`) in parallel in their own worktrees (live terminals
  + status dots) — **then the built app is opened and used** (add/delete items) to prove it works.
  Captured headlessly on an Xvfb virtual display (no real-desktop interference), driven with
  xdotool, recorded with ffmpeg; the build and run segments are stitched with ffmpeg concat.
- Added `docs/recording-a-demo.md` documenting the whole process — Xvfb + GDK_BACKEND=x11 +
  software-rendering env, `tauri build --no-bundle` vs `cargo build`, xdotool driving, Claude
  Code's per-worktree onboarding prompts, and the GTK+WebKit viewer used to render the built app
  (Chrome/Firefox black out on a GLX-less Xvfb). Demo/top-level READMEs link both.

### Demo — recorded multi-agent walkthrough
- Added `demo/`: `multi-agent-demo.sh` reproduces AutoDev's fan-out + worktree-isolation flow
  (3 agents build a calc library in parallel, each on its own `git worktree`/branch, then merge
  back) using only git + bash — no GUI or CLI auth, runs in a temp dir and cleans up. Recorded
  to `multi-agent-demo.txt` (plain text) and a replayable `script`(1) capture (`.rec`/`.timing`);
  `demo/README.md` maps each step to the app's composer/grid/merge. Linked from the README.

### Open folder as workspace + one-command releases
- **"Open folder as workspace…"** button in the sidebar: pick any existing folder via the native
  picker and it creates a workspace named after the folder's basename with that folder added as
  its first project — in one step. New `createFromFolder` store method (+2 tests). The existing
  create-workspace-then-`+dir` flow is unchanged.
- **`./dev.sh release X.Y.Z`**: bumps the version in `package.json` + `tauri.conf.json`, commits,
  tags `vX.Y.Z`, and pushes — the tag drives `release.yml` to build and attach installers to a
  draft GitHub release. README's "Publishing a release" now uses this one command.

### Google Antigravity backend
- Added `Antigravity` as a third agent backend (alongside Claude and Codex), invoked as `agy`.
  Its `AgentAdapter` arm maps AutoDev's options to `agy` flags per Google's published CLI guide:
  interactive sessions pass the initial prompt via `-i`/`--prompt-interactive`, `-m <model>` for
  model, `--add-dir` for `@`-mention context, `--dangerously-skip-permissions` for bypass, and
  screenshot paths appended to the prompt (no image flag). No documented plan/read-only flag, so
  plan mode is intentionally not mapped for this backend. +2 unit tests. Selectable in the
  composer's Backend dropdown; needs `agy` on `PATH`. (Adding a backend = add an adapter arm +
  one dropdown option — nothing else changed, per the architecture's adapter seam.)

### Release automation — GitHub Releases + download docs
- Added `.github/workflows/release.yml`: on a `v*` tag (or manual dispatch), builds the app on a
  Linux/macOS(universal)/Windows matrix with `tauri-action` and uploads each platform's installers
  to a **draft** GitHub release. Wired for the `APPLE_*` signing secrets (unsigned without them).
- README: a **Download** table (grab the AppImage/dmg/exe from Releases) and a **Publishing a
  release** section (version-bump → tag → push → publish the draft). Documented exactly **where
  app state lives** (`~/.autodev/`), clarifying that adding a workspace/project only records
  metadata (name + absolute path) — project files are never copied or moved.

### Close the gaps — robust waiting detection + signing docs
- **Agent `waiting` detection reworked.** It is now a *silence-derived* state decided in
  `tick()` (like `idle`): any fresh output flips an agent back to `running`, and only once it
  goes quiet is the tail classified — a trailing prompt ⇒ `waiting`, otherwise `idle`. This
  fixes the old bug where prompt text lingering in the buffer kept an agent stuck on `waiting`.
  Detection now strips ANSI (new exported `stripAnsi`, mirrors the Rust one), scans the last few
  lines, and recognises Claude/Codex multi-line approval menus (`❯ 1.` selection cursor,
  "(use arrow keys)", "No, and tell Claude…") in addition to y/n and "press enter" prompts.
  +2 tests (waiting/idle classification, stripAnsi).
- **README: code signing & notarization.** Turned the vague "not configured" note into an
  actionable, CI-ready recipe — the exact macOS (`APPLE_*` env, notarization) and Windows
  (`certificateThumbprint` / Azure `signCommand`) hooks to fill in. Still ships unsigned (certs
  are secrets); the same `./dev.sh build` produces signed artifacts once credentials are present.

### Evaluator diff wiring — base-commit tracking
- `git`: `head_commit` + `diff_since` (+1 test). `LoopState` gains `base_commit` (serde-default
  so old state loads). Entering Generating (set-contract, planner auto-apply, retry) captures the
  project's HEAD; `loop_current_prompt` computes the round's work-tree diff against that base and
  embeds it in the evaluator prompt. Non-repo dirs → empty diff (graceful).

### Settings UI for the pluggable commands
- New `SettingsPanel` modal (⚙ in the header): edit theme, default effort, and the transcribe /
  screenshot / browser command templates that were previously hand-edited in
  `~/.autodev/settings.json`. Blank command fields persist as null ("not configured"). +2 tests.

### Richer agent status detection
- `AgentStatus` adds `error` (non-zero exit code, distinct from a clean/killed `exited`) and
  `waiting` (output tail matches a confirmation-prompt pattern; pure, exported `detectWaiting`).
  Status dots/labels and the terminal Kill/close controls updated via an `isTerminal` helper. +4
  tests. Prompt patterns are end-anchored and conservative to avoid false positives.

### README — building a standalone executable
- Documented `./dev.sh build`, the output binary + AppImage/deb/rpm/dmg/msi bundle paths,
  per-OS build requirement, versioning, signing, and the runtime CLI dependency. Refreshed the
  stale "Phase 0" Usage section.

### Loop hands-off mode — opt-in auto-run
- `LoopPanel` gains an **Auto-run** checkbox (off by default). When on, the loop spawns each
  next role itself after every advance — create → planner → generator → evaluator → retry/pass/
  fail — with no clicks. Bounded by `max_iterations`; a parse failure stops the chain (the phase
  doesn't advance, so nothing re-spawns) and shows the manual fallback. Auto-launching a chain
  of agents is never the silent default (security note). One component test.

### Loop auto-advance — parse role output to fill contract & verdicts
- Rust `loop_engine`: pure parsers `strip_ansi` (CSI/OSC escapes, carriage-return redraws),
  `parse_contract` (numbered/bulleted criteria after a `CONTRACT` header, else all list items),
  and `parse_verdicts` (per-criterion `N. PASS/FAIL`; an unreported criterion defaults to FAIL,
  matching the adversarial evaluator). Planner and evaluator prompts tightened to emit that
  exact parseable shape. Seven new unit tests.
- Commands `loop_apply_planner` / `loop_apply_evaluator` read the finished role agent's output
  log (`~/.autodev/logs/<agent>.log`), parse it, and advance the phase (planner → contract +
  generating; evaluator → grade → pass/retry/fail). They error (leaving the phase put) when the
  log is missing or no criteria parse, so the manual controls stay as a fallback.
- Frontend `LoopPanel`: tracks the running role agent; when it exits an effect auto-applies —
  planner fills the contract, generator advances to evaluating, evaluator grades. Manual
  textarea/checkboxes remain as an editable fallback shown on a parse failure. Three component
  tests (first `.tsx` component tests; `vitest.config` now resolves Solid's browser build).
- Deferred: embedding the real per-iteration git diff in the evaluator prompt (needs a recorded
  base commit); the evaluator agent inspects the repo directly for now.

### Fix — text input contrast in dark mode
- The loop-spec and criteria textareas (`.loop-new textarea`, `.loop-step textarea`) had no
  dark-mode override, so in dark mode they rendered a white background with near-white
  inherited text. Added them to the dark textarea group (`#2a2a2a` bg / white text), matching
  the composer.
- No `::placeholder` styling existed anywhere, so placeholders fell back to the faint browser
  default. Added a global `::placeholder` rule (`#767676` light, `#9a9a9a` dark; both ≈4.6:1,
  WCAG AA) so every placeholder in the app is legible.

### Phase 9 — Autonomous loop engine
- Rust `loop_engine`: `Role` (planner/generator/evaluator) with a distinct system prompt
  each (role separation, LOOPS XXVIII); `LoopState` (phase, iteration, contract, features,
  progress); `Criterion` (testable done-assertion, LOOPS XXIX). `grade_and_advance` = all
  met → passed, else retry to `max_iterations` then failed (LOOPS XXXI). Disk state under
  `~/.autodev/loops/<id>/` — state.json, contract.md, feature-list.json, progress.md, log.md
  (LOOPS XXX). Eight `loop_*` commands.
- Frontend `LoopPanel` (Loops header tab): create a loop from a spec, run each role as a
  real agent in the project dir, set the contract, grade criteria, watch the phase advance.
- Tests: +5 Rust (role prompts, phase transitions, retry-until-max, disk roundtrip).
  Total 32 Rust + 28 frontend. Full-app boot re-verified.

### Phase 8 — Browser handoff
- Rust `handoff` module: `build_handoff(task, url, context)` produces a structured
  browser-AI prompt (goal / starting point / context / steps / report-back), with
  fallbacks for empty fields. `run_browser` runs a configured `browserCommand` on the
  handoff file. `generate_handoff` + `run_browser_handoff` commands; `browserCommand`
  setting; `AppError::Browser`.
- Frontend `BrowserHandoff` modal (🌐 in the composer): task/url/context form → generate,
  copy to clipboard, or run the browserCommand and show its output.
- Tests: +3 Rust (handoff sections, empty fallbacks, browser file pass-through).
  Total 27 Rust + 28 frontend.

### Phase 7 — Screenshot + annotate
- Pluggable capture: `AppSettings.screenshotCommand` (`{file}` → PNG path, run via `sh -c`,
  e.g. grim/scrot/screencapture). Rust `capture` module runs it and returns base64;
  `save_shot` persists an annotated PNG under `~/.autodev/shots/`.
- Frontend `Annotator`: canvas over the screenshot with arrow/box/pen tools, colors, and
  undo; pointer drawing batched via requestAnimationFrame (LOOPS XXXIX). Composer 📷 button
  captures → annotates → attaches; attachments ride the next launch.
- `AgentOptions.images`: Codex gets `-i <file>` per image; Claude (no image flag) gets the
  path appended to the prompt.
- Tests: +3 Rust (capture read-back, missing-file error, save decode) and +1 frontend
  (arrowhead geometry). Total 24 Rust + 28 frontend.

### Phase 6 — Voice-to-text
- Pluggable transcription: `AppSettings.transcribeCommand` is a shell template with a
  `{file}` placeholder, run via `sh -c` (any pipeline works, e.g. whisper.cpp). Rust
  `transcribe` module renders + runs it; `transcribe_audio` command writes the recording
  to a temp file, transcribes, cleans up, and errors clearly if unconfigured.
- Frontend: `recorder` (MediaRecorder wrapper) + a mic button in the composer that records,
  transcribes, and appends the text to the prompt.
- Tests: +4 Rust (command render/quoting, stdout capture, file pass-through, failure) and
  +2 frontend (mime→ext). Total 20 Rust + 27 frontend.

### Phase 5 — Git worktree isolation + merge-back
- Rust `git` module (shells out to `git`): `is_repo`, `current_branch`, `status`
  (branch + dirty), `create_worktree`, `diff`, `merge` (no-ff, refuses a dirty target),
  `remove_worktree`. Six `git_*` commands; worktrees created under `~/.autodev/worktrees/`.
- Composer “Isolate (worktree)” toggle: each fanned-out agent gets its own worktree +
  branch (`autodev/<project>-<ts>-<i>`), so parallel agents on one repo don't collide.
  Agent bar shows the branch with Merge / Remove actions.
- Tests: +3 Rust (real create→commit→diff→merge→remove flow; merge refuses dirty target).
  Total 16 Rust + 25 frontend.

### Phase 4 — Prompt composer
- `PromptComposer`: textarea with `@`-mention resolution against workspace projects
  (resolved/unresolved chips), a difficulty 1–10 slider that suggests agent count and
  plan/ultrathink, toggles for plan/bypass/ultrathink, backend + working-dir selectors,
  and a fan-out launch to N agents. Mentioned projects pass as `--add-dir` context.
- Rust: `AgentOptions.add_dirs` → Claude `--add-dir`; arg-building refactored into a pure,
  unit-tested `command_line`. Prompt history persisted to `~/.autodev/prompts.json` with
  dedupe + cap; `get_prompt_history`/`add_prompt_history` commands.
- Tests: +2 Rust (claude/codex argv incl. add-dir; prompt-history dedupe/order) and +8
  frontend (difficulty heuristic, mention parse/resolve). Total 13 Rust + 25 frontend.

### Phase 3 — Multi-agent orchestration
- Frontend `agent-store`: one global pair of `agent://output`/`agent://exit` listeners
  feeds every agent; per-agent output is buffered (1 MB cap) and replayed when a terminal
  attaches, so focus-switching keeps full scrollback and the Phase 2 startup race is gone.
  Status per agent: running / idle (1.5 s silence) / exited(code).
- `AgentGrid` cards with live status dots; click to focus, close exited ones, “Kill all”.
- Per-project “▶ Claude” and “▶ Codex” launchers; run many agents across projects at once.
- Rust: kill every agent on window close (`on_window_event`) so no PTY child is orphaned;
  per-agent raw output logged to `~/.autodev/logs/<id>.log`.
- `TerminalPane` now attaches to the store (replay + live) instead of listening directly.
- Tests: +6 frontend (agent store: spawn/focus, buffer replay, idle, exit, close,
  kill-all). Total 11 Rust + 17 frontend. App boot re-verified.

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
