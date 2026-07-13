# Changelog

Newest first. Functional changes only (LOOPS XXIV).

## [2026-07-13]

### Loop auto-advance on the hook bus (P3 complete)
- The autonomous loop's auto-advance now reacts to the public `exit` hook instead of a reactive
  `createEffect` polling agent status — the **second** built-in on the P3 bus, alongside
  onboarding auto-accept. A non-zero (error) exit still does not auto-advance (manual controls
  take over); a new loop-panel test locks that guard. This closes P3's Done bar: both built-in
  behaviors run through the hook API. (Config/extension-loaded hooks remain P5.)

### In-app Help & documentation
- Added a self-contained **Help panel** (the **?** button in the header) so end users never need
  external docs. A table-of-contents sidebar + full guide covering: what AutoDev is, workspaces
  & projects, starting a task (backends, difficulty, modes, per-agent prompts, voice/screenshot/
  handoff), Auto-split, agents & terminal statuses, git worktree isolation, autonomous loops,
  settings, **Extending AutoDev** (custom backends with an example `BackendSpec`, prompt
  templates, skills — all file-based under `~/.autodev/`), the on-disk data layout, and
  tips/troubleshooting. New `help-panel.tsx` (content as a section array so the ToC can't drift)
  + styles; tests cover rendering the extensibility docs and closing.

### Prompt templates + skills dir (P4 — extensibility track)
- **Prompt templates:** drop `*.md` files in `~/.autodev/templates/`; in the composer, type
  `/name` and a suggestion row appears — click it or press **Tab** to expand the template into
  the task box (any text typed after `/name` is kept). Rust `templates.rs` reads the dir
  (`list_templates`); pure frontend `templates.ts` (`expandTemplate`/`templateMatches`) drives
  the UX. No code to add a template — it's a file.
- **Skills dir:** if `~/.autodev/skills/` exists and has content, it is added to *every* agent's
  context via a `--add-dir` — on every backend — through a **P3 spawn hook** (`skills.ts`
  `installSkillsHook`, wired in `App.tsx`). The first real feature built on the hook bus:
  skills reach agents through the same seam as `@`-mentions, with no per-launch wiring.
- Tests: `templates.rs` (list/sort/ignore-non-md, skills-dir present-only-when-non-empty);
  `templates.ts` + `skills.ts` (expansion, prefix-match, dir injection + dedup, hook install);
  composer (`/ref` → click suggestion → body expands). 92 Rust + frontend green.

### Public agent-lifecycle hook bus (P3 — extensibility track)
- Adds `src/lib/hooks.ts`, a typed hook bus so built-in behaviors and (later) extensions can
  participate in an agent's life. Five lifecycle points: `spawn` (a *transform* that may
  rewrite `AgentOptions` before launch — the analog of Pi's `before_provider_headers`),
  `output`, `idle`, `waiting`, and `exit` (observers). A throwing hook is contained — it can't
  break a launch or the other hooks.
- The agent store now emits through the bus: `spawn` applies the transform before
  `agentSpawn`; `output`/`exit` fire on the `agent://*` events; `idle`/`waiting` fire from
  `tick` on a real status change. The bus is exposed as `store.hooks` for registration.
- **Dogfooded:** the onboarding auto-accept (auto-answer the trust-folder prompt on unattended
  runs) moved out of `pushOutput` into a built-in `output` hook — the first consumer of the
  seam, proving it carries real side-effecting work. Behavior unchanged (existing onboarding
  tests pass as-is).
- Tests: `hooks.test.ts` (compose transforms, error isolation, unregister, per-event routing);
  `agent-store.test.ts` (spawn hook rewrites options, output/exit/waiting emitted). Chosen
  architecture: frontend TS bus (where orchestration already lives; matches Pi's TS
  extensions). Follow-ups: migrate the loop's auto-advance onto an `exit` hook (2nd built-in);
  loading hooks from config/extensions is P5.

### Pluggable agent backends via declarative specs (P1 — extensibility track)
- First step (M0/P1) of `PI-PARITY-PLAN.md`: making the backend adapter real. Backends are
  no longer a hardcoded `match` in `command_line` — each is a declarative `BackendSpec`
  (program + flag mappings) and the argument vector is built by one canonical `build_args`.
  Bundled specs for `claude`/`codex`/`antigravity` reproduce their exact previous command
  lines; the existing `agent.rs` tests are the conformance suite and pass unchanged.
- **A new backend needs no code.** Drop `~/.autodev/backends/<id>.json`; `load_specs` merges
  disk specs over the bundled defaults (a disk file may also retune a shipped backend by id).
  `AgentBackend` gained a `Custom(String)` variant, serialized transparently as its string id,
  so the on-the-wire/on-disk contract is unchanged and an unknown id round-trips to `Custom`.
- New `backend_list` command feeds the composer's backend picker dynamically (bundled +
  disk-registered); the hardcoded `<option>` list is gone. If the fetch fails the composer
  keeps its default.
- Tests: `backend_spec.rs` — JSON→args, disk registration + builtin override, missing-dir
  fallback; `agent.rs` — `AgentBackend` serde round-trip incl. `Custom`, and an end-to-end
  drop-in test proving a JSON-only backend is launchable through the real spawn arg-builder.
  87 Rust + 70 frontend tests green; lint clean.

## [2026-07-12]

### Auto-split: intelligent parallel decomposition (Phase 10)
- The composer can now decide *on its own* whether a task fans out across independent agents,
  instead of the user guessing the count. A new **✨ Auto-split** button runs a one-shot
  classifier agent (`claude -p`, read-only — it may inspect the working dir to enumerate real
  work items, e.g. "transcode every video in ./media" → one unit per file) that returns a
  fenced `TASKPLAN` JSON: an inferred **difficulty** (1–10), a **parallel** verdict, and one
  self-contained sub-prompt per unit. A parallel plan pre-fills the existing per-agent fan-out
  (per-agent prompts + Isolate auto-on, agent count = units); a non-parallel verdict collapses
  to a single agent. Nothing launches — the user reviews the proposed split, then Launch.
  Closes two gaps: difficulty is now *inferred*, not dialed, and the split is *parallel* (vs the
  loop engine's serial backlog).
- Prompt construction and output parsing live in Rust (`task_split.rs`, pure + unit-tested):
  `split_prompt` builds the classifier prompt; `parse_task_plan` strips terminal escapes, takes
  the last fenced block, deserializes, and validates (clamps difficulty, drops blank units, caps
  at 12, forces `parallel` off for a lone unit). Two commands (`task_split_prompt`,
  `task_split_parse`) mirror the loop's decomposer round-trip. Frontend controller
  `lib/task-split.ts` spawns the invisible classifier, waits for its exit (with a timeout that
  kills a hung run), and parses. The apply logic is ordered so the concrete unit count wins over
  the difficulty→agents heuristic — covered by `prompt-composer.test.tsx`.
### Auto-split on launch (opt-in)
- Added an **Auto-split on Launch** setting (⚙ Settings; `autoSplitOnLaunch`, off by default). When
  on, the first **Launch** on a task that isn't already split and whose agent count wasn't set by
  hand runs the classifier and pauses for review (fills the per-agent prompts + shows the split
  banner) instead of fanning out; a second Launch then goes. Editing the Agents number, or having
  already used the ✨ button, opts that task out of the auto-analysis. The setting is read fresh on
  each Launch, so toggling it mid-session takes effect immediately. New Rust `AppSettings`
  field + settings-panel toggle; composer gating covered by `prompt-composer.test.tsx`.

## [2026-07-11]

### Prompt UX: disambiguate the composer from an agent's own prompt
- Two things were both called "the prompt" and it wasn't clear which did what: the composer
  at the top (which *launches* new agents) and each agent's `❯` input inside its terminal
  (Claude Code's own TUI, for continuing that agent). Labelled both: the composer now has a
  **"New task"** heading with "Launches a fresh agent for each. To continue an agent that's
  already running, type in its terminal below — not here.", and each agent's session shows
  "This is <label>'s own terminal — type here to reply to this agent." above the terminal.
  Placeholder tweaked to "Describe the task to start…". Copy-only; no behavior change.

### "Open in editor" — open an agent's worktree/cwd in your editor
- Added an **Open in editor** button on each agent's session bar. It opens that agent's git
  worktree (or its cwd if not isolated) in your editor — closing the review loop without
  hunting for the path. Process spawning stays in the Rust core (`open_in_editor` command);
  the editor is configurable via a new **Editor** setting (`editorCommand`, default `code`;
  e.g. `code -n`, `cursor`, `subl`). The command is split on whitespace and the canonicalized
  path appended as the final arg — never run through a shell (LOOPS XV). New `editor.rs` with
  a unit-tested `build_open_command`; verified end-to-end by driving the app against a fake
  editor (the path was passed correctly).

### Dropdown contrast fix (dark mode) + a per-agent prompts demo video
- **Fixed invisible dropdowns in dark mode.** The Backend / Run-in `<select>`s rendered dark-on-dark
  because no `color-scheme` was declared, so WebKitGTK themed native controls light while our CSS
  painted a dark background behind them. Added `color-scheme: light` on `:root` and `color-scheme:
  dark` in the dark media query. Verified on a dark virtual display: both selects now show their
  values in legible white. `src/App.css`.
- **Added `demo/autodev-per-agent-prompts-demo.mp4`** — a real screen recording of the per-agent
  prompts flow (shared task → 2 agents → per-agent overrides + auto-isolate → fan out to two
  `claude` agents in separate worktrees). Captured headlessly per `docs/recording-a-demo.md`.

### Per-agent prompts — divide one project across a fan-out
The Prompt Composer used to send the *same* prompt to every agent in a fan-out. Added
opt-in **Per-agent prompts**: the main textarea is a shared base, and a "Per-agent prompts"
toggle reveals one override box per agent (shown when Agents > 1). A blank override inherits
the shared prompt, so the single-prompt path is unchanged. `@`-mentions now resolve from each
agent's *own* effective prompt, so agents can be pointed at different projects in one Launch.
Enabling per-agent prompts defaults **Isolate (worktree)** on (divergent tasks in one working
dir would collide), with a visible hint if the prompts differ and Isolate is off. History
records each distinct prompt. Selection logic extracted to `src/lib/agent-prompts.ts`
(`selectPrompts` / `withUltrathink` / `promptsDiffer`), unit-tested; the launch path is
covered by a new `prompt-composer.test.tsx` that drives the real fan-out. Frontend-only.

## [2026-07-10]

### Browser handoff: a real Playwright browserCommand runner + docs
- Added `browser-runner/` — a self-contained Playwright runner for the Browser handoff feature:
  reads AutoDev's handoff file, opens a real Chromium at the `## Starting point` URL, screenshots
  it, and prints a report AutoDev shows in the modal (`HEADLESS=0` to drive it by hand). Its own
  `package.json` so it doesn't add Playwright to the app's deps; `node_modules` gitignored.
  Verified live against a real page (example.com → title + screenshot). Honest scope: a launcher/
  scaffold, not an autonomous agent — the README shows where to add an LLM loop for full autonomy.
- Documented the Browser handoff feature in the README (with a live demo screenshot,
  `demo/browser-handoff.png`) and pointed it at the runner.

### Ran a real autonomous epic — two live bugs fixed, end-to-end validated
Drove a full epic in the real app with live `claude` agents (Python string-utils lib). It
completed **PASSED** — 3 features decomposed → each planned → built → verified — with the retry
path exercised (a feature hit 9/10, retried, reached 10/10 gated by `python3 test_strutils.py`).
The agents produced correct, tested, committed code. This closes the long-standing "not driven
live end-to-end" caveat. Two real bugs surfaced only by running it, both fixed + regression-tested:
- **Loop role agents never exited.** They spawned interactive, but auto-advance fires on agent
  *exit* — so the chain stalled at `decomposing` forever. Fix: `AgentOptions.print_mode` runs
  loop roles as `claude -p` (one-shot: run, print, exit). Generator/evaluator bypass permission
  prompts so they can write/test; decomposer/planner stay read-only via their prompts.
- **`strip_ansi` blanked CRLF lines.** Agents print `FEATURES:\r\n1. …`; the carriage-return
  overwrite logic took the text *after* the trailing `\r` (empty), so every line became "" and
  `parse_features`/`parse_contract`/`parse_verdicts` found nothing. Fix: use the last **non-empty**
  `\r`-segment (Rust + the TS mirror). Never caught before because the unit tests used `\n`.
- Evidence: `demo/epic-passed.png` (the PASSED epic — 14 agents all `exited (0)`).

### LLM context compaction (long-run memory) — last Phase-2 item
Over a long epic the naive bounded progress tail loses cross-feature context. A **Summarizer**
role now compresses it.
- Engine: `Role::Summarizer` + `summarizer_prompt` (compact "what's built / key decisions / what
  FAILED and why" into a `SUMMARY:` block); `parse_summary`, `compact_progress` (replace the
  progress memory with a bounded digest), and `needs_compaction`/`MAX_PROGRESS_CHARS` (2500). +3
  tests.
- Commands: `loop_needs_compaction`, `loop_compact_prompt` (Summarizer role + prompt — a
  maintenance step, not a phase), `loop_compact` (parse the summarizer's output → replace progress).
- Frontend: in the hands-off **Auto-run** chain, when a loop's progress has grown past the
  threshold the panel spawns a read-only summarizer to compact memory **before** the next
  plan/generate/evaluate role; a manual **🗜 Compact memory** button appears when the memory is
  large. +1 test. **All Phase-2 autonomy items are now done.**

### Continue-on-failure toggle for epics
- New opt-in **Continue on failure** loop option (off by default): when a feature stalls or runs
  out of rounds, the epic **skips it and keeps building the rest of the backlog** instead of
  failing outright. The epic finishes `Passed` if every feature succeeded, else `Failed` with a
  partial-success summary (`epic finished: 2/3 features done; failed: auth, search`).
- Engine: `Feature.failed` + `LoopState.continue_on_failure`; `grade_and_advance`'s give-up branch
  refactored into `advance_or_finalize(succeeded)` + `finalize_epic` (marks the feature done/failed,
  moves to the next or closes out the epic). Fail-fast (the default) is unchanged. `loop_create`
  gains a `continue_on_failure` param. +2 Rust tests.
- Frontend: composer checkbox; the backlog now shows failed features with a red ✗. +1 frontend test.
- Deferred (last Phase-2 item): LLM-based context compaction for very long runs.

### Onboarding auto-responder (unattended runs don't stall)
- New opt-in **Auto-onboard** toggle in the loop composer. When on, a loop's role agents
  auto-accept Claude Code's "trust this folder?" dialog — the gate that stalls agents in fresh
  worktrees/project dirs (it's why the first demo recording hung). The agent-store detects the
  exact trust prompt in the streamed output and sends Enter once (pure, exported `onboardingReply`;
  debounced so it fires once per gate, and again only for a genuinely new one).
- Chosen over mutating `~/.claude.json`: that file is used by the running Claude Code session, so
  rewriting it risks a concurrent-write clobber. The responder handles the prompt like a human
  would, with no global-config side effect, and is **off by default** — sending keystrokes to an
  agent is never silent. Deliberately narrow: only the trust dialog (Enter = its default "Yes"),
  never the bypass-permissions warning (whose default is No). +3 frontend tests.

### Feature-epic driver (autonomous multi-feature builds)
Turns a loop from "one bounded contract" into an epic that builds a whole backlog to completion.
- New **Decomposer** role + **Decomposing** phase: a loop now starts by breaking the spec into an
  ordered `FEATURES:` backlog (`decomposer_prompt`, `parse_features`). Then, per feature, the
  existing Planner → Generator → Evaluator sub-loop runs; the planner/generator/evaluator prompts
  carry the current feature title and a backlog overview.
- `LoopState.features` is now `Vec<Feature { title, done }>` + `current_feature`. On a feature
  passing (contract met **and** verify not failing), `grade_and_advance` marks it done and either
  advances to plan the next feature (resetting per-feature round state) or completes the epic when
  the backlog is exhausted. **Fail-fast:** a stalled/exhausted feature fails the whole epic, naming
  it. An empty backlog still behaves as a single ad-hoc contract (backward compatible).
- Commands: `loop_apply_decomposer` (parse backlog from the decomposer log → planning),
  `loop_set_features` (manual fallback); `loop_set_contract` drops its legacy `features` param.
- Frontend: Decomposing phase drives a decomposer agent (auto-applied on exit; manual textarea
  fallback); the detail view shows the feature backlog (done ✓ / current ▸) and `feature k/N`.
  Hands-off Auto-run chains decompose → per-feature plan/generate/evaluate → next feature.
- +5 Rust tests, +1 frontend. Deferred (Phase 2 remainder): LLM context compaction,
  onboarding/permission pre-flight, continue-on-feature-failure option.

### Loop trust & durability core (autonomous long-running builds)
Closes three gaps that made the autonomous loop untrustworthy over long runs.
- **Ground-truth verify gate.** New `verify.rs` `run_verify(command, project_dir)` runs a
  user-configured test command via `sh -c` (exit 0 = pass). `LoopState.verify_command` +
  reworked `grade_and_advance(state, verdicts, verify)`: a loop reaches `Passed` only when every
  criterion is met AND the tests did not fail — so failing tests block a pass even if the
  evaluator rated every criterion PASS. The model no longer grades its own homework unchecked.
- **Stuck detection + escalation.** `LoopState.history` (met-count per round) + pure
  `is_stuck(history, window=3)`; a stalled loop now fails early with a recorded
  `failure_reason` ("no progress in 3 rounds …" / "out of iterations …; tests failing") instead
  of silently burning the whole budget.
- **Bounded progress memory.** `append_progress` accumulates a per-round summary
  (`round k: M/T met; verify=pass/fail; failing: …`, last 15 lines) that is fed into the
  generator (don't repeat what failed; run the verify command) and evaluator (treat the test
  command as ground truth) prompts.
- **Configurable cap.** `loop_create(spec, project_dir, verify_command?, max_iterations?)`;
  default raised 5 → 8. Loop-new form gains **Verify command** + **Max rounds** inputs; the
  detail view shows the verify command and the failure reason. +12 Rust tests, +2 frontend.
- Deferred to Phase 2: feature-epic driver (many contracts to completion), LLM context
  compaction, onboarding/permission pre-flight.

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
