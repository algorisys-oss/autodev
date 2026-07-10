# Implementation Tracking

Audit trail from decision to code (LOOPS XXV). Newest first.

## Hardening — Loop auto-advance — COMPLETE

**Decided:** Close the biggest Phase 9 deferral — the loop transcribing the planner's and
evaluator's output by hand. Parse it instead. Parsing lives in the Rust core (pure,
unit-tested); the frontend only passes IDs. Chose to parse the agent's **on-disk output log**
(`~/.autodev/logs/<agent>.log`, already written on every spawn) keyed by agent id, so the
frontend never handles the raw output text — it keeps the hard boundary intact and sidesteps
the plan-mode constraint (a planner in plan mode can't write a structured file, but it still
prints to the terminal, which is logged). Kept the manual textarea/checkboxes as an editable
fallback rather than removing them: parsing terminal scrollback is best-effort, so a wrong
parse must be correctable, not fatal.

**Built:**
- `loop_engine.rs` (+7 tests): `strip_ansi` (CSI/OSC + carriage-return redraws), `parse_contract`
  (list items after a `CONTRACT` header, else all list items), `parse_verdicts` (per-criterion
  `N. PASS/FAIL`, unreported ⇒ FAIL). Tightened planner/evaluator prompts to emit that shape.
- `commands.rs`: `loop_apply_planner` / `loop_apply_evaluator` — read the role agent's log,
  parse, advance; error (phase unchanged) on missing log / no criteria so the UI falls back.
- `loop-panel.tsx`: tracks the running role agent; a `createEffect` fires on its exit and calls
  the matching apply command (planner → contract+generating, generator → evaluating, evaluator →
  graded). `ipc.ts` bindings added. First `.tsx` component tests (+3); `vitest.config` gains
  `resolve.conditions: ["development","browser"]` so Solid renders under jsdom.

**Then — hands-off auto-run:** added an opt-in **Auto-run** toggle (off by default). When on,
`autoAdvance` chains `runRoleFor(next)` after a successful advance and `create` kicks off the
planner, so a run is create → planner → generator → evaluator → retry/pass/fail with no clicks.
Refactored `runRole` into `runRoleFor(loop)` (with a `roleRunning` guard against double-spawn).
Bounded by `max_iterations`; a parse failure doesn't advance, so the chain stops and the manual
fallback shows. Kept it opt-in because auto-launching a chain of agents shouldn't be silent
(security note). +1 component test.

**Status:** complete. `./dev.sh verify` green (39 Rust + 32 frontend).

**Deliberate deferrals:**
- Evaluator diff still empty: embedding the real per-iteration `git diff` needs a base commit
  recorded when generation starts. The evaluator agent inspects the repo directly meanwhile.
- Parsing is best-effort against terminal scrollback; the manual controls are the safety net.

## Phase 9 — Autonomous loop engine — COMPLETE

**Decided:** Realize the LOOPS Tier 6 architecture concretely: three roles with separate
system prompts, a contract of testable criteria, disk-backed state, and a phase machine
that retries on failure until `max_iterations` then fails. The pure pieces (role prompts,
`grade_and_advance`, disk roundtrip) are fully unit-tested. Each role runs as a real agent
in the loop's project dir via the existing agent infra. Human-in-the-loop for now: the
user runs a role, then records its contract / grades its verdicts.

**Built:**
- `src-tauri/src/loop_engine.rs` (+5 tests) — roles, `LoopState`, `Criterion`, prompts,
  `grade_and_advance`, disk save/load/append_log; `state::loops_dir`.
- 8 `loop_*` commands; `src/components/loop-panel.tsx` + a Workspace/Loops header tab.

**Status:** complete. `./dev.sh verify` green (32 Rust + 28 frontend). Full-app boot
re-verified with all commands registered.

**Deliberate deferrals (the honest gap vs a fully hands-off loop):**
- Not yet auto-advancing: the loop doesn't parse the planner's output into the contract or
  the evaluator's output into verdicts automatically — the user transcribes those. Closing
  this needs reliable agent-completion detection + structured-output parsing (the real next
  bottleneck, LOOPS XXXV). The state machine, prompts, and disk state are all in place for it.
- Evaluator runs with an empty diff argument; wiring `git diff` of the loop branch into the
  evaluator prompt is a small follow-up.

## Phase 8 — Browser handoff — COMPLETE

**Decided:** The valuable, reproducible core is *generating* a good handoff prompt (pure,
tested). Actual browser control is left pluggable via `browserCommand` (e.g. a Playwright
runner reading the handoff from `{file}`) rather than bundling Playwright — mirrors the
video's Comet flow where an agent writes a handoff and a browser AI executes it. Without a
`browserCommand`, the user copies the handoff into any browser AI.

**Built:**
- `src-tauri/src/handoff.rs` (+3 tests) — `build_handoff`, `run_browser`;
  `generate_handoff` + `run_browser_handoff` commands; `browserCommand` setting.
- `src/components/browser-handoff.tsx` modal; 🌐 button in the composer.

**Status:** complete. `./dev.sh verify` green (27 Rust + 28 frontend).

**Deliberate deferrals:**
- No bundled browser automation; `browserCommand` is the seam. A first-class Playwright
  integration can land later.

## Phase 7 — Screenshot + annotate — COMPLETE

**Decided:** Screen capture is a pluggable shell command (`screenshotCommand`), same
pattern as voice — no heavy screen-capture crate, works with grim/scrot/screencapture/etc.
Annotation is a canvas over the captured PNG; drawing is batched into a single rAF
(LOOPS XXXIX). Attachment reuses the agent launch: Codex takes `-i <file>`, Claude has no
image flag so the path is appended to the prompt for the agent to open.

**Built:**
- `src-tauri/src/capture.rs` (+3 tests) — `run_capture`, `save_png`; `capture_screen` +
  `save_shot` commands; `screenshotCommand` setting; `AgentOptions.images` wired into
  `command_line` (+1 Rust test for codex `-i` / claude prompt-append).
- `src/lib/annotate.ts` (+1 test), `src/components/annotator.tsx`, composer 📷 button +
  attachment chips.

**Status:** complete. `./dev.sh verify` green (24 Rust + 28 frontend).

**Deliberate deferrals:**
- Full-screen capture only (no region picker in-app); crop by using a region-capture
  `screenshotCommand` or cropping in the annotator later.
- Canvas drawing (annotator) is verified manually — jsdom has no real canvas; only the
  pure `arrowHead` geometry is unit-tested.

## Phase 6 — Voice-to-text — COMPLETE

**Decided:** Transcription is a pluggable shell command (`transcribeCommand` in settings),
not a bundled model — keeps the app light and lets the user pick whisper.cpp, an API
wrapper, or anything with a CLI. `{file}` is substituted (shell-quoted) and the template
runs via `sh -c`, so pipelines like `whisper-cli … && cat …txt` work. Mic capture uses the
webview's MediaRecorder; bytes go to the core, which writes a temp file and runs the
command.

**Built:**
- `src-tauri/src/transcribe.rs` (+4 tests) + `transcribe_audio` command; `AppSettings.
  transcribeCommand`.
- `src/lib/recorder.ts` (+2 tests for `extFromMime`), mic button in the composer.

**Status:** complete. `./dev.sh verify` green (20 Rust + 27 frontend).

**Deliberate deferrals:**
- No settings UI to set `transcribeCommand` yet — edit `~/.autodev/settings.json`. A
  settings panel can come later.
- MediaRecorder/getUserMedia aren't unit-tested (no jsdom support); only the pure
  `extFromMime` is. The mic path is exercised manually in the app.

## Phase 5 — Git worktree isolation + merge-back — COMPLETE

**Decided:** Shell out to `git` (no libgit2 dep, LOOPS VIII). Merge refuses a dirty target
working tree so it never clobbers uncommitted local work (LOOPS XXIII/XXXVI: keep
destructive git safe). Each fanned-out agent gets its own worktree + branch, which is what
makes parallel agents on one repo safe by construction. Worktrees live under
`~/.autodev/worktrees/<branch-slug>`.

**Built:**
- `src-tauri/src/git.rs` (+3 tests: real create→commit→diff→merge→remove; dirty-refusal).
- 6 `git_*` commands; composer “Isolate” toggle; `AgentView.worktree`; merge/remove UI.

**Status:** complete. `./dev.sh verify` green (16 Rust + 25 frontend).

**Deliberate deferrals:**
- Merge only brings in *committed* work on the branch. Uncommitted changes in a worktree
  are not auto-committed; a "commit worktree" action can come later. The agent itself
  (esp. in bypass mode) usually commits.
- No conflict-resolution UI: a conflicting merge surfaces git's error text; resolve in the
  repo directly for now.

## Phase 4 — Prompt composer — COMPLETE

**Decided:** `@`-mentions resolve to project `--add-dir` context (matches how the video
adds context). Difficulty drives suggestions via a pure `suggestForDifficulty` table
(1→1 agent/no plan, 10→6 agents/plan/ultrathink); moving the slider re-applies count +
plan + ultrathink, all still user-overridable. Ultrathink appends the "ultrathink" hint
to the Claude prompt. Arg-building was pulled into a pure `command_line` so the exact
argv (including `--add-dir`) is unit-tested rather than asserted as "is_ok".

**Built:**
- `src/lib/difficulty.ts`, `src/lib/mentions.ts` (+ `composer.test.ts`, 8 tests).
- `src/components/prompt-composer.tsx`; App now launches via the composer (removed the
  per-project quick-launch buttons — single launch path).
- Rust: `command_line` (pure) + `add_dirs`; `state` prompt history + 2 commands.

**Status:** complete. `./dev.sh verify` green (13 Rust + 25 frontend).

**Deliberate deferrals:**
- "Effort (high/extra-high)" from the video is not wired — neither CLI takes it as a
  launch flag; it is a session/settings concern. `AppSettings.default_effort` exists for
  when that lands.
- `@`-mention has no live autocomplete dropdown yet; it resolves on submit and shows
  resolved/unresolved chips. Autocomplete can come later.

## Phase 3 — Multi-agent orchestration — COMPLETE

**Decided:** A single global listener pair in the frontend `agent-store` feeds all agents
and buffers their output, rather than each terminal subscribing to Tauri events itself.
That fixes the Phase 2 race (a terminal mounting after spawn replays the buffer) and lets
focus-switching keep scrollback with only the focused terminal mounted (keyed `Show`).
Only running/idle/exited status for now; "waiting for input" is not reliably detectable
from a raw PTY, so it is folded into idle.

**Built:**
- `src/lib/agent-store.ts` (+6 tests) — buffer/replay, status/idle ticker, spawn/kill/
  killAll/close/focus/attach/detach, injectable ipc + subscribe + clock for tests.
- `src/components/agent-grid.tsx`, reworked `terminal-pane.tsx`, reworked `App.tsx`.
- Rust: `on_window_event` CloseRequested → `AgentManager::kill_all` (no orphans);
  `state::logs_dir` + best-effort per-agent disk logging in `agent_spawn`.

**Status:** complete. `./dev.sh verify` green (11 Rust + 17 frontend). App boot re-verified
with the window-close handler and disk logging.

**Deliberate deferrals:**
- Orphan-on-quit is handled for a normal window close; a SIGKILL of the app can still
  orphan children (unavoidable). PTY children also get SIGHUP when the master drops.
- Disk logs are raw bytes (include escape sequences); a stripped/plain variant can come
  later if the logs need to be read directly.

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
