# Handoff

Rolling snapshot of the current state. Read this first when picking the project back up.
Updated as the final step of every task (LOOPS XXVI).

## Where things stand

- **Last task:** Rich view increment 3B(1) — **pre-launch tool permissions** (branch
  `feat/rich-view`, **not yet merged/shipped**). The composer gained a **Tool permissions**
  section (shown for backends that declare the capability — Claude) to auto-allow and/or block
  tools before launch, wired to `--allowedTools`/`--disallowedTools`. Blocked tools are removed
  from the session (proven live: "There's no Bash tool available in this session"); the posture is
  stored on the agent and re-applied on Rich follow-up turns. Declarative: `BackendSpec`
  `allowed_tools_flag`/`disallowed_tools_flag`; `AgentOptions.allowed_tools`/`disallowed_tools`;
  `backend_list` reports `tool_permissions`. **Flags are emitted as one `--flag=a,b` arg** —
  live testing caught that these flags are *variadic* and a space-separated value swallows the
  positional prompt (the classic "Input must be provided… when using --print" error). This is the
  **B1** scope the user chose: coarse per-tool control, not per-action approve/deny buttons.
  **B2** (true per-action buttons) is deferred — it needs an MCP permission server / the Agent SDK
  (a Node sidecar = stack change), since 2.1.207 has no `--permission-prompt-tool` and `-p`
  stream-json emits no inline permission events. **Verify:** 124 Rust + 119 frontend tests,
  clippy/rustfmt/eslint/tsc + vite build clean; the exact allow/deny arg vector blocks a tool while
  keeping the prompt (real `claude` 2.1.207). GUI unverified here.
- **Prior task:** Rich view increment 3A — **interactive multi-turn follow-ups** (branch
  `feat/rich-view`, **not yet merged/shipped**). After a Rich turn finishes, a composer in the
  card view sends a follow-up that runs as a fresh one-shot turn `--resume`ing the same backend
  session; its cards append to the **same** conversation card. Stays in the PTY model — each turn
  is one-shot (no pipe transport). Design: `StructuredMode.resume_flags` (with `{id}`) expresses
  each backend's resume form (Claude `--resume <id>`, Codex `exec resume <id>`);
  `AgentOptions.resume_session_id`; session id captured from the normalized `SessionInit` event
  (now carries `session_id`; Codex's `thread.started` maps to it too). Frontend: the store keeps a
  `resumeMap` (follow-up process id → conversation agent id) so a follow-up's output/events/exit
  route onto the original card; `store.followUp(id, text)`; a `userMessage` event renders the
  user's turn; `rich-pane` gained the follow-up composer. **Verify:** end-to-end at the CLI level
  the exact resume arg order recalls prior context (`claude` 2.1.207); 122 Rust + 118 frontend
  tests, clippy/rustfmt/eslint/tsc + vite build clean. **GUI unverified here** — try: launch a
  Claude Rich session (Bypass on), let it finish, type a follow-up, Send → new cards append.
  **Next: increment 3B — approval buttons** (per-action approve/deny), which needs an MCP
  permission-prompt server (no `--permission-prompt-tool` flag in 2.1.207; `-p` stream-json has no
  inline permission events) — bigger + security-sensitive, scope separately.
- **Side task (DONE):** the status footer's git branch is now **live** — it re-polls git status
  on an interval (`StatusFooter` `pollMs`, default 3s) instead of refetching only when project
  paths change, so a checkout / new worktree updates the branch + ● dirty marker as it happens.
- **Prior task:** Rich view increment 2 — **Codex driver (multi-backend seam proven)** (branch
  `feat/rich-view`, **not yet merged/shipped**). A second `StructuredDriver`
  (`CodexJsonlDriver`, `codex exec --json`) maps Codex's `item.started`/`item.completed`/
  `turn.completed` JSONL (agent_message → AssistantText; command_execution → ToolCall+ToolResult;
  turn.completed → Done) onto the **same** `AgentEvent` model — so the Rich view renders Codex
  with **zero frontend changes** (the composer offers the toggle because `backend_list` now
  reports Codex as `structured`; `rich-pane`'s tool summarizer already reads the `command` key).
  Both drivers share the byte-buffered `drain_lines` splitter. Codex spec declares
  `structured: codex exec --json --skip-git-repo-check`. Driver + spec tested against real
  `codex-cli` 0.144.0 output (6 new tests; 118 Rust + 116 frontend green, lint clean). GUI render
  for Codex unverified here (same read-only/one-shot shape as Claude). **Next:** the interactive
  path (approvals/follow-ups over the bidirectional stream-json contract), or reasoning-item
  mapping for Codex.
- **Prior task:** Rich view — structured card-based agent sessions, increment 1 (branch
  `feat/rich-view`, **not yet merged/shipped**). An opt-in alternative to the raw xterm pane:
  a session renders as native cards (assistant text, thinking, tool calls, tool results, a done
  chip with cost/duration) driven by a normalized `agent_event::AgentEvent` stream. **Scope of
  increment 1: read-only + one-shot, Claude only** — because `claude --output-format stream-json`
  is one-shot (`-p`); interactive approvals/follow-ups are a later increment (needs the
  bidirectional `--input-format stream-json` contract, confirmed present on `claude` 2.1.207).
  - **Architecture (decided with user): Rust-direct + normalized event model, NOT the Anthropic
    SDK** — so Codex/others plug in behind the same UI later. New Rust `agent_event.rs`
    (`AgentEvent` enum + `StructuredDriver` trait + `ClaudeStreamJsonDriver`, 11 tests over real
    `claude` 2.1.207 output); `BackendSpec` gained a declarative `structured` capability (Claude
    only); `AgentOptions.rich`; `build_args` routes rich→stream-json flags (3 tests); `commands.rs`
    runs the driver and emits normalized events on a new **`agent://event`** channel (raw NDJSON
    still emitted + logged for the Raw-stream toggle). Frontend: `ipc.ts` mirrors `AgentEvent` +
    `BackendInfo.structured`; `agent-store` collects `events[]` per rich agent (2 tests); new
    `rich-pane.tsx` renders the cards; composer **Rich view** toggle (backend-gated); `App.tsx`
    swaps RichPane⇄TerminalPane with a per-session **Raw stream** toggle.
  - **Verify:** `./dev.sh verify`-equivalent all green — 112 Rust + 113 frontend tests, eslint +
    tsc + clippy + rustfmt clean, `vite build` clean. **GUI still unproven here:** the live
    card-rendering path (composer toggle → spawn → `agent://event` → cards) needs an eyeball. To
    try it: `./dev.sh dev`, pick **Claude**, check **Rich view**, optionally **Bypass
    permissions** (so tools run and produce tool cards), type a prompt, **Launch** — the focused
    session shows cards; **Raw stream** flips to the NDJSON.
  - **Next increment:** adapter capability flag polish + a second backend driver (Codex), then
    the interactive path (approvals/follow-ups over `--input-format stream-json`).
- **Prior task:** Agent-terminal visibility fixes (v0.10.1, on `dev`). Two CSS-only fixes in
  `src/App.css`. (1) The focused terminal could be squeezed too short to show a full-screen agent
  menu: `.main-scroll`'s flex-grow shrank `.agent-session` (which had `min-height: 0`) to a few
  rows, so the highlighted selection in Claude Code's MCP-onboarding prompt rendered off-screen and
  arrow keys moved an invisible cursor. `.agent-session` now has `min-height: 300px` (~17 rows).
  (2) xterm's scrollback had no visible scrollbar — WebKitGTK renders the viewport's
  `overflow-y: scroll` as a transient overlay that never shows at rest; added an explicit
  `.terminal-pane .xterm-viewport::-webkit-scrollbar` style for a persistent themed bar. **Verify:**
  `./dev.sh verify` green (98 Rust + frontend); lint + build clean. **GUI-confirmed by user** in
  the running dev instance (menu selection now visible; scrolling works). **Scope:** CSS only; no
  Rust, no TS logic, no new deps.
- **Prior task:** Cross-agent structured annotation (P9, on `dev`). The screenshot/annotate flow
  now captures **notes** (one per line) with the drawing; a capture is an `Annotation { image,
  notes }` that **fans out to every agent in a launch** — notes as prompt text (so they reach any
  backend incl. Pi), image where supported. Pure `annotate.ts` (`annotationBlock`) +
  `agent-prompts.ts` (`composeAgentPrompt`) build each prompt; composer reuses the image fan-out.
  **Verify:** `./dev.sh verify` green (97 Rust + 17 frontend). GUI-only: the annotator notes UI
  round-trip (logic is unit-tested). **Scope:** orchestrator-layer half done; the live-DOM element
  picker (pi-annotate-style selectors/box-model) needs a browser+native-host bridge — documented
  follow-on feeding the same artifact. **P7 (RPC embed) deferred** — Pi + its extensions already
  run in the Pi cell; full RPC is large/Pi-only/overlaps P9. **Remaining roadmap:** P9 live-DOM
  capture (needs browser), optional P6/P7/P8.
- **Prior task:** Pi as a backend (M1 spike, on `dev`). Installed Pi (v0.80.6), verified its CLI
  flags, and shipped a working backend spec: `examples/backends/pi.json` (+ installed to
  `~/.autodev/backends/pi.json`). AutoDev launches **Pi** as an interactive cell via P1 — model→
  `--model`, bypass→`--approve`, one-shot→`-p`, positional prompt; a Rust test (`include_str!`)
  locks the mapping. Running a model needs Pi's own `/login`. **Known gaps** (in
  `examples/backends/README.md`): no `--add-dir` forwarding, no plan/image mapping, and Pi's
  extensions run *inside* the cell — the RPC embedding (P7) that would surface pi-annotate in
  AutoDev's UI is still future work. **Verify:** `./dev.sh verify` green. GUI-only: a live Pi PTY
  session (flag acceptance was confirmed via `pi --approve --model … -p …`, which reached the
  auth prompt).
- **Prior task:** Executable extensions (P5 of `PI-PARITY-PLAN.md`, on `dev`). Drop a
  self-contained JS module in `~/.autodev/extensions/`; its default export gets an `autodev` API
  to register P3 hooks and composer `/commands`. **Trust model (chosen with user): trusted +
  surfaced** — no sandbox (user's own files), but Settings lists loaded extensions with ✓/✗ +
  errors and Help documents it with a trust warning. Rust `extensions.rs` reads name+source;
  frontend runs each via a blob-URL ES module import; commands merge into the composer. **Verify:**
  `./dev.sh verify` green (96 Rust + 17 frontend files). GUI-only: the blob module-loader path
  (logic around it is unit-tested via an injected evaluator). To try: create
  `~/.autodev/extensions/x.js` with `export default (autodev)=>{ autodev.registerCommand('hi','Hello') }`,
  restart, type `/hi`. **Remaining roadmap:** P9 (cross-agent annotation — browser bridge), M1
  (Pi spike — hands-on browser); P6/P7/P8 optional. **Queued next:** a maximize/expand control for
  the New-task textarea (user request).
- **Prior task:** Theme toggle + dark contrast + screenshot default (on `dev`, from review
  feedback). Header ☀/🌙 toggles light/dark (persisted); theming is now attribute-driven
  (`:root[data-theme]`, converted from media-query-only) with a pre-paint script in `index.html`
  and a `theme.ts` module — "system" still follows the OS live. Full dark-mode contrast pass
  (Help panel, status chips, phase badges, modal labels, captions). Screenshots now fall back to
  a detected platform tool (grim/scrot/spectacle/… , macOS `screencapture`) when unconfigured.
  **Verify:** `./dev.sh verify` green (94 Rust + 16 frontend files). NOTE: theme/contrast are
  covered by unit tests + tsc/eslint but not a live GUI pass here — worth an eyeball in the
  running app.
- **Prior task:** P3 close-out — loop auto-advance on the hook bus (on `dev`). The autonomous
  loop's auto-advance now reacts to the public `exit` hook (`loop-panel.tsx`) instead of a
  reactive status-polling `createEffect` — the second built-in on the P3 bus. A non-zero (error)
  exit does not auto-advance (guard `!code`, locked by a new test). **P3 Done bar is now fully
  met** (both built-ins run through the hook API). **Verify:** `./dev.sh verify` green (92 Rust +
  15 frontend files). **Remaining roadmap:** P5 (extension/config-loaded hooks — trust model
  decision), P9 (cross-agent annotation — browser bridge), M1 (Pi spike — hands-on browser).
- **Prior task:** In-app Help & documentation (on `dev`). A self-contained **Help panel** (the
  **?** header button, `help-panel.tsx`) with a ToC sidebar + full guide covering every feature —
  workspaces, tasks, backends, difficulty/modes, Auto-split, agent statuses, worktrees, loops,
  settings, **Extending AutoDev** (custom backends w/ example spec, templates, skills), the
  `~/.autodev/` data layout, and troubleshooting. Content is a section array so the ToC can't
  drift. **Verify:** `./dev.sh verify` green (92 Rust + 15 frontend files). So end users need no
  external docs.
- **Prior task:** Prompt templates + skills dir (on `dev`) — P4 of `PI-PARITY-PLAN.md`. Two
  file-backed features: (1) **templates** — `*.md` files in `~/.autodev/templates/`; typing
  `/name` in the composer shows a suggestion row, click or **Tab** expands it into the task box
  (Rust `templates.rs` lists them; pure `templates.ts` expands). (2) **skills dir** — if
  `~/.autodev/skills/` has content it's added to every agent's `--add-dir` via a **P3 spawn
  hook** (`skills.ts` `installSkillsHook`, installed in `App.tsx`) — the first product feature
  riding the hook bus. **Verify:** `./dev.sh verify` green (92 Rust + frontend). To try it: drop
  `~/.autodev/templates/refactor.md` (any text) and type `/refactor`; drop a file in
  `~/.autodev/skills/` and launch an agent — the dir is on its context path. **Next per roadmap:**
  P9 (cross-agent structured annotation — the differentiator, bigger, wants a browser bridge),
  the deferred P3 loop-advance migration, or M1 (Pi spike, needs a hands-on browser session).
- **Prior task:** Public agent-lifecycle hook bus (on `dev`) — P3 of `PI-PARITY-PLAN.md`,
  increment 1. New `src/lib/hooks.ts` is a typed hook bus: `spawn` (a transform that rewrites
  `AgentOptions` before launch) + `output`/`idle`/`waiting`/`exit` observers, error-isolated,
  exposed as `store.hooks`. The agent store emits through it (spawn transform before
  `agentSpawn`; output/exit on `agent://*`; idle/waiting from `tick`). Dogfooded: the onboarding
  auto-accept moved from `pushOutput` into a built-in `output` hook (behavior unchanged).
  **Architecture decision (with the user):** frontend TS bus (matches where orchestration lives
  + Pi's TS extensions). **Verify:** `./dev.sh verify` green (89 Rust + 79 frontend). **Deferred:**
  migrate the loop's auto-advance onto an `exit` hook (it's a reactive effect in `loop-panel.tsx`
  today — a clean follow-up, kept out to protect the loop feature); loading hooks from
  config/extensions is P5. **Next per roadmap:** P4 (prompt templates + skills — a spawn hook is
  the natural injection point) or M1 (Pi spike — needs a hands-on browser session, can't be
  verified headless).
- **Prior task:** Pluggable agent backends via declarative specs (on `dev`) — the first step
  (M0/P1) of the new extensibility roadmap `PI-PARITY-PLAN.md`. Backend launch is now data, not
  a hardcoded `match`: each backend is a `BackendSpec` (program + flag mappings) built by one
  canonical `build_args` (`src-tauri/src/backend_spec.rs`). **Add a backend by dropping
  `~/.autodev/backends/<id>.json`** — `load_specs` merges disk specs over the bundled
  `claude`/`codex`/`antigravity` (disk can also retune a shipped backend by id). `AgentBackend`
  gained a `Custom(String)` variant (transparent string serde), a `backend_list` command feeds
  the composer's now-dynamic picker, and the existing `agent.rs` conformance tests pass
  unchanged. **Verify:** `./dev.sh verify` (87 Rust + 70 frontend green, lint clean); an
  end-to-end test (`a_disk_registered_backend_is_launchable_end_to_end`) drives the real
  disk→spec→command-line path a spawn uses. Only the literal in-app dropdown *click* is
  unverified (composer component tests + the `backend_list` path cover it indirectly).
  **Next per the roadmap:** M1 — the Pi spike (embed Pi via a spec, test whether `pi-annotate`
  bridges), then Track A: P3 hooks → P4 templates → P9 cross-agent annotation.
- **Prior task:** Auto-split — intelligent parallel decomposition (branch `feat/auto-decompose`,
  **Phase 10**, not yet merged). The composer's **✨ Auto-split** button runs a one-shot read-only
  classifier (`claude -p`) that infers difficulty and whether the task parallelizes, then pre-fills
  the per-agent fan-out (parallel → N units as per-agent prompts + Isolate; not parallel → 1 agent).
  Nothing launches until the user reviews and clicks Launch. Also shipped the **Auto-split on Launch**
  setting (opt-in, off by default): when on, the first Launch analyzes + pauses for review unless the
  task is already split or the count was hand-set. Rust `task_split.rs` (pure prompt+parse, 11 tests)
  + commands `task_split_prompt`/`task_split_parse` + `AppSettings.autoSplitOnLaunch`; frontend
  `lib/task-split.ts` (round-trip, 3 tests) + composer wiring (4 tests: units-win-over-difficulty,
  parallel vs single, analyze-on-launch pause/fan-out, hand-set count opts out). **Verify:**
  `./dev.sh verify` green (80 Rust + 70 frontend). **Proven live** (headless, real `claude -p`):
  batch task → `parallel:true` with one unit per real file (dir enumerated); cohesive bug fix →
  `parallel:false`, 1 unit — both parsed back through `parse_task_plan`. Only the in-app GUI *render*
  of the button/banner is unverified (covered by component tests).
- **Prior task:** Prompt-UX clarity (on `dev`) — the composer and each agent's `❯` input were
  both "the prompt". Labelled the composer as **"New task"** (launches agents) with a caption
  pointing follow-ups to the agent's terminal, and captioned each agent's terminal ("type here
  to reply to this agent"). Copy-only, verified in the running app.
- **Prior task:** "Open in editor" button (on `dev`) — each agent's session bar opens its
  worktree (or cwd) in your editor via a Rust `open_in_editor` command; editor is configurable
  (`editorCommand` setting, default `code`). New unit-tested `src-tauri/src/editor.rs`; verified
  end-to-end against a fake editor. This is phase 0 of the code-editor plan (embed a real editor
  later — see `~/.claude/plans/keen-crunching-puppy.md`).
- **Prior task:** Per-agent prompts in the composer (branch `feat/per-agent-prompts`), plus a
  dark-mode dropdown-contrast fix and a demo video. A fan-out can now hand each agent a different
  prompt: shared base + opt-in per-agent override boxes (blank inherits the base), `@`-mentions
  resolved per prompt, Isolate auto-on when enabled. Logic in `src/lib/agent-prompts.ts`; end-to-end
  covered by `prompt-composer.test.tsx`. Also fixed invisible Backend/Run-in `<select>`s in dark
  mode (missing `color-scheme` → WebKitGTK light-themed native controls on our dark bg; `src/App.css`).
  Recorded `demo/autodev-per-agent-prompts-demo.mp4` on a virtual display. Complete and tested;
  **not yet merged to `main`**. **One command to verify:** `./dev.sh test` (63 frontend + 64 Rust
  green). Note: `./dev.sh lint`/`verify` has a *pre-existing* failure in
  `browser-runner/browser-runner.mjs` (no-undef on node globals), unrelated to this change.
  Next possible step: the embedded code-editor plan (CodeMirror 6 + git-diff view) sketched in
  `~/.claude/plans/keen-crunching-puppy.md`.
- **Prior task:** Hardening pass — evaluator diff wiring, a settings UI, and richer agent
  status. Complete. Earlier in the session: loop auto-advance + hands-off auto-run, and a
  dark-mode text-input contrast fix. README now documents building a standalone executable.
- **Phases done:** 0–9, plus **Phase 10 (auto-split)** on branch `feat/auto-decompose`. The
  full ecosystem from `PLAN.md` is implemented: workspaces, multi-agent orchestration, composer,
  worktrees, voice, screenshot, browser handoff, the Planner/Generator/Evaluator loop, and now
  intelligent parallel decomposition in the composer.
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
- **Next up:** merge `feat/auto-decompose` to `main` (Auto-split + the analyze-on-launch setting are
  both done and verified). Remaining honest caveats: the `waiting` heuristic is still pattern-based
  (won't catch every exotic prompt); a live loop run depends on the agent models following the
  tightened prompts; and the Auto-split button/banner has been proven at the classifier + parse level
  but not eyeballed rendering in the running GUI. See `implement.md`.

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
- Auto-split: **✨ Auto-split** in the composer analyzes the typed task with a one-shot
  read-only classifier and pre-fills the fan-out — a parallelizable task (e.g. "convert every
  video in ./media") becomes N per-agent prompts with Isolate on; a cohesive task stays a single
  agent. It also infers the difficulty. Review the proposed split, then Launch (nothing runs
  until you do). Prompt + parsing are in Rust (`task_split.rs`). Optionally turn on **Auto-split on
  Launch** in ⚙ Settings so the first Launch analyzes + pauses for review automatically (unless the
  task is already split or you set the agent count by hand).
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
  Trust & durability: set a **Verify command** (e.g. `./dev.sh verify`) — its exit code is
  ground truth, so a loop can't pass on the evaluator's say-so if tests fail. The loop keeps a
  per-round met-count history, ends early with a **failure reason** when it stalls (no progress
  in 3 rounds) or runs out of **Max rounds** (default 8), and feeds a bounded progress memory
  into the generator/evaluator prompts.
- Feature-epic driver: a loop is an **epic** over a feature backlog. It starts by running a
  **Decomposer** (spec → ordered `FEATURES:` list), then runs the Planner→Generator→Evaluator
  sub-loop **per feature**; a passing feature advances to the next, and the epic passes when the
  whole backlog is done. Fail-fast: a stalled/exhausted feature fails the epic, naming it. The
  panel shows the backlog (done ✓ / current ▸) and `feature k/N`; Auto-run chains the whole thing.
  An empty backlog still works as a single ad-hoc contract. Tick **Continue on failure** so a stalled feature is skipped (marked ✗) and the epic keeps building the rest.
- Unattended runs: tick **Auto-onboard** (off by default) so a loop's role agents auto-accept
  Claude Code's "trust this folder?" prompt (the gate that stalls fresh worktrees) — the store
  detects the exact prompt and sends Enter once. Narrow by design: only the trust dialog, never
  the bypass warning. Long runs stay coherent: when the progress memory grows past a threshold a read-only **Summarizer** compacts it (auto in the chain, or the 🗜 Compact memory button). All Phase-2 autonomy items are done.
- **Validated live:** a real epic ran end-to-end with live `claude` agents and reached PASSED —
  3 features decomposed → planned → built → verified (`python3 test_strutils.py` green), retry path
  exercised, agents committed working code. Two bugs that only a live run exposes were fixed: loop
  roles now run one-shot (`claude -p` via `AgentOptions.print_mode`) so they exit and auto-advance
  fires; `strip_ansi` no longer blanks CRLF lines. Evidence: `demo/epic-passed.png`. The old "not
  driven live end-to-end" caveat is resolved.
- `./dev.sh test` — Rust `cargo test` (80 tests) + Vitest (70).
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
