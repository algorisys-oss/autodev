# Implementation Tracking

Audit trail from decision to code (LOOPS XXV). Newest first.

## Prompt templates + skills dir (P4 — extensibility track) — COMPLETE (branch `dev`)

**Context:** Third step of `PI-PARITY-PLAN.md` (Track A). Cheap, high-ROI, and it exercises the
P3 hook bus end to end — the skills-dir feature *is* a spawn hook, proving the seam carries a
real product feature (not just the migrated onboarding built-in).

**Approach — both halves are file-backed (code-free), like backend specs:**

- **Rust `templates.rs`** (new, hermetic + tested): `list_templates_from(dir)` reads
  `templates/*.md` (name = filename stem, body = contents, trailing newline trimmed), sorted;
  `skills_dir_from(dir)` returns the abs path of `skills/` only when it exists and has ≥1 entry
  (an empty dir is nothing worth adding). Commands `list_templates`/`skills_dir`.
- **Templates UX (frontend):** pure `templates.ts` — `expandTemplate(text, tpls)` turns a
  leading `/name` into the body (keeping trailing text), `templateMatches` prefix-matches while
  the command is still being typed. The composer fetches templates on mount, shows a suggestion
  row when typing `/…`, and expands on click or **Tab**.
- **Skills injection (frontend):** `skills.ts` — `withSkillsDir(opts, path)` appends the dir
  (dedup); `installSkillsHook(bus)` fetches the skills dir and, if present, registers a P3
  `spawn` hook. `App.tsx` installs it on mount. Skills therefore reach every backend through the
  existing `--add-dir` path with zero per-launch code.

**Files:** `src-tauri/src/templates.rs` (new) + `commands.rs`/`lib.rs` registration;
`src/lib/templates.ts` + `skills.ts` (new, + tests); `src/lib/ipc.ts` (`PromptTemplate`,
`listTemplates`, `skillsDir`); `src/components/prompt-composer.tsx` (suggestion row + Tab
expand, placeholder copy); `src/App.tsx` (install hook).

**Verification:** `./dev.sh verify` green (92 Rust + frontend, 14 files). Pure logic unit-tested
both sides; the composer test drives the real UI (`/ref` → click suggestion → body expands).
Not exercised: writing into the user's real `~/.autodev` (avoided on purpose — hermetic tests
cover the disk logic). Design note: the skills-dir feature is the proof that P3's frontend TS
bus was the right call — a product feature slotted onto it with a one-line `onSpawn`.

## Public hook lifecycle (P3 — extensibility track) — INCREMENT 1 COMPLETE (branch `dev`)

**Context:** Second step of `PI-PARITY-PLAN.md` (Track A spine). The plan's event stream
(`agent://output`/`exit`) was informal; P3 turns it into a public, typed hook API that
extensions and built-ins can hook into — the prerequisite for P9 (cross-agent annotation) and
P5 (extension loading).

**Decision (asked the user):** where does the hook seam live? Chose **frontend TS bus** over a
Rust-core registry or declarative config, because (a) the two behaviors the plan wants to
migrate — loop auto-advance and onboarding auto-accept — already live in the frontend, (b) Pi's
extensions are TS, so P5 extension-loading lands there anyway, (c) shortest path to the Done
bar. Trade-off accepted: orchestration logic stays frontend-side (as onboarding already was);
the Rust core stays pure process/PTY.

**Approach:** `src/lib/hooks.ts` — `createHookBus()` returns a `HookBus` with `onSpawn`
(transform, composed via reduce) + `onOutput`/`onIdle`/`onWaiting`/`onExit` (observers), each
returning an unregister fn, all error-isolated (a throwing hook degrades to a no-op).
`agent-store.ts` creates a per-store bus, emits through it (spawn transform before
`agentSpawn`; output/exit on the `agent://*` handlers; idle/waiting from `tick` on status
change), and exposes it as `store.hooks`.

**Dogfooding (proof the seam carries real work):** the onboarding auto-accept block moved out
of `pushOutput` into a registered built-in `output` hook. Behavior identical — the existing
`auto-onboard…` and `does not touch input…` tests pass unchanged.

**Files:** `src/lib/hooks.ts` (new) + `hooks.test.ts` (new, 6 tests); `agent-store.ts`
(bus + emission + onboarding-as-hook), `agent-store.test.ts` (+3 tests: spawn rewrite,
output/exit emit, waiting emit).

**Verification:** `./dev.sh verify` green (89 Rust + 79 frontend, lint clean). The Done bar is
substantially met — a registered hook rewrites launch flags and reacts to exit (tests prove
it), and one built-in runs through the API. **Deferred (honest scope):** (1) migrating the
loop's auto-advance — it's a reactive `createEffect` on status in `loop-panel.tsx`, not a
direct exit subscription, so moving it risks the loop feature; it's a clean follow-up (register
an `exit` hook). (2) Loading hooks from config/disk is P5, not P3.

## Pluggable backends via `BackendSpec` (P1 — extensibility track) — COMPLETE (branch `dev`)

**Context:** A new extensibility roadmap (`PI-PARITY-PLAN.md`) was agreed with the user after
comparing AutoDev to the Pi harness. Its foundation (M0/P1) is making the "agent adapter" real:
`CLAUDE.md`/`PLAN.md` promised *"add a backend = add an adapter, change nothing else,"* but
`command_line` was a hardcoded `match` over an `AgentBackend` enum — adding a backend meant
editing the enum + match + tests. P1 turns backend launch into data so a new CLI is a JSON file.

**Approach — one declarative spec, one canonical builder, disk-loaded registry.** Done in two
scope-locked steps so the riskiest claim was proven first:

- **Step 1 (de-risk the builder):** `src-tauri/src/backend_spec.rs` (new) defines `BackendSpec`
  (program, print/plan/bypass flags, model flag, add-dir flag + cwd rule, `ImageMode`,
  `PromptMode`) and `build_args`, a single fixed-order algorithm
  (print|plan → bypass → model → add-dirs → image-flags → prompt+appended-images). Bundled
  specs for the three real backends. `command_line` delegates to it; `Mock` stays a special
  case (arbitrary command, no spec). The existing 10 `agent.rs` conformance tests passed
  **unchanged** — proof one algorithm reproduces every backend's exact arg vector.
- **Step 2 (make it data-driven):** `AgentBackend` became an enum with a `Custom(String)`
  variant + manual `Serialize`/`Deserialize` (transparent string id), preserving every existing
  call site and the wire contract while letting unknown ids round-trip. `command_line(opts,
  specs)` now resolves against an injected registry (hermetic tests); `build_command` feeds it
  `load_specs()`. `load_specs_from(dir)` reads `<data_dir>/backends/*.json`, sorts for
  determinism, and overrides bundled specs by id. New `backend_list` command +
  `BackendInfo`/`backendList()` in `ipc.ts`; the composer fetches the list on mount and renders
  options dynamically, falling back to its default backend if the list lacks it.

**Files:** `src-tauri/src/backend_spec.rs` (new), `agent.rs` (enum + `command_line` signature +
tests), `commands.rs` (`backend_list`), `lib.rs` (module + command registration),
`src/lib/ipc.ts` (`AgentBackend` widened, `BackendInfo`, `backendList`),
`src/components/prompt-composer.tsx` (dynamic picker).

**Verification:** `backend_spec.rs` and `agent.rs` tests cover JSON→args, disk registration +
override, missing-dir fallback, serde round-trip incl. `Custom`, and an end-to-end drop-in test
(`a_disk_registered_backend_is_launchable_end_to_end`) that writes a JSON spec and drives the
real `load_specs_from` + `command_line` path a spawn would use. 87 Rust + 70 frontend green,
lint clean. Not exercised: the literal GUI dropdown click (covered indirectly by the composer
tests + the `backend_list` unit path).

## Auto-split — intelligent parallel decomposition (Phase 10) — COMPLETE (branch `feat/auto-decompose`)

**Context:** AutoDev could fan out to N agents, but the human decided N (difficulty was a
manual slider, never inferred from the task), and the only automatic task-splitting — the loop
engine's decomposer — produces a *serial* backlog worked one feature at a time, never parallel.
Goal: let the app decide *on its own* whether a task parallelizes (large batch jobs, independent
edits) and pre-fill the fan-out, without the user asking. Two product forks confirmed with the
user: (1) ship an explicit **✨ Auto-split** button now + an opt-in "analyze on launch" setting
as a fast follow ("Both"); (2) the classifier **may read the working dir** (read-only) so it can
enumerate concrete work items (20 videos → 20 units), not just abstract splits.

**Approach — reuse the decomposer round-trip, but pre-launch and parallel.** The decomposer
already proves the pattern: spawn a one-shot agent → read its `~/.autodev/logs/<id>.log` → pure
Rust parser → structured result. Mirrored it for a stateless, pre-launch classifier:

- `src-tauri/src/task_split.rs` (new, pure, TDD — 11 tests written first):
  - `split_prompt(task, projects)` — builds the classifier prompt: defines parallel-vs-sequential,
    permits read-only cwd inspection, asks for a fenced `<<<TASKPLAN … TASKPLAN` JSON block.
  - `parse_task_plan(output)` — reuses `loop_engine::strip_ansi`, takes the **last** fenced block
    (model may restate the template), `serde_json`-deserializes a lenient `RawPlan`, then
    normalizes: clamp difficulty 1–10, drop blank-prompt units, title falls back to prompt, cap
    at `MAX_UNITS` (12), and force `parallel=false` whenever ≤1 unit survives. Chose JSON over the
    decomposer's line-list because sub-prompts are long/multi-line — regex-parsing that is fragile.
  - `TaskPlan`/`TaskUnit` (serde, camelCase) — the typed contract.
- `commands.rs`: `task_split_prompt` (frontend gets the prompt from Rust — wording lives once) and
  `task_split_parse` (reads the log via the existing `read_agent_log`, returns `Option<TaskPlan>`;
  `None` = no plan block, so the UI degrades to manual). Registered in `lib.rs`. Types mirrored in
  `src/lib/ipc.ts`.
- `src/lib/task-split.ts` (new, 3 tests): `analyzeTask` spawns the classifier **invisibly**
  (`printMode`, not added to the agent grid — a throwaway; the Rust ProcessManager still owns it),
  waits for `agent://exit` (injectable `Subscribe`), and parses. A timeout kills a hung classifier
  and still attempts a parse (so a slow-but-complete or missed-event run still yields a plan).
- `prompt-composer.tsx`: an `autoSplit()` handler + a `plan` signal driving a review banner and an
  apply effect. **Ordering subtlety (the load-bearing bit):** the existing `on(difficulty)` effect
  owns `agentCount`, so applying a plan sets `setDifficulty(...)` first (each Solid setter flushes
  its effects synchronously) *then* `setPlan(...)`; the plan-apply effect is created after the
  difficulty effect and thus wins the count — parallel → per-agent prompts + count = units + Isolate
  on; non-parallel → 1. Locked by `prompt-composer.test.tsx` ("units win over the difficulty
  heuristic"). Nothing launches: human reviews first (consistent with the bypass/parallel-power
  security posture, LOOPS XV).

**Scope guard (LOOPS IV):** new files + two commands + composer button/banner + CSS. Untouched:
the fan-out loop, worktree flow, agent adapters, the loop engine (only reused its `pub strip_ansi`).
No new deps (`serde_json` already present).

**Analyze-on-launch (fast follow, now built):** added `AppSettings.auto_split_on_launch` (Rust,
`#[serde(default)]` false; mirrored `autoSplitOnLaunch` in TS) + a settings-panel checkbox. In
`launch()`, a gate `!plan() && !countTouched() && text().trim()` reads the setting **fresh** each
Launch (so a mid-session toggle applies) and, when on, runs `autoSplit()` (now returning a bool) and
returns early on success — the user reviews, then a second Launch fans out (plan now set → gate skips
re-analysis). A `countTouched` signal, flipped only by the Agents number input, opts a hand-counted
task out; it resets after a successful launch. Launch is disabled while analyzing. Covered by
`prompt-composer.test.tsx`: pause-then-fan-out, and "hand-set count opts out". The `save_then_load`
Rust roundtrip test gained the new field.

**Status:** complete on `feat/auto-decompose`. `./dev.sh verify` green (80 Rust + 70 frontend + lint).
**Verified live (headless):** ran the real `claude -p` classifier with the faithful `split_prompt`
against a dir of 5 `.mov` files → it enumerated all 5 read-only and emitted a valid `TASKPLAN`
(`parallel:true`, 5 units, one ffmpeg command per file); a cohesive "fix the login redirect bug" →
`parallel:false`, 1 unit. Both parsed cleanly through `parse_task_plan`. **Unverified:** only the
in-app GUI *render* of the button/banner (component tests cover the wiring).

## Per-agent prompts in the composer — COMPLETE

**Context:** A fan-out (Agents > 1) launched N copies of one prompt — there was no way to
hand each parallel agent a different sub-task. Added per-agent prompts so one Launch can
divide a project across agents. UX confirmed with the user: shared base + opt-in overrides
(blank inherits the base); `@`-mentions resolved per agent's own prompt; Isolate defaults on
when per-agent is enabled (collision safety, LOOPS XV).

**Approach:** kept the component thin by extracting pure selection logic to
`src/lib/agent-prompts.ts` (`selectPrompts`, `withUltrathink` — lifted from the inline rule
in `launch()` —, `promptsDiffer`), mirroring the `difficulty.ts`/`mentions.ts` pattern. TDD:
RED tests in `composer.test.ts` first, then the helper. Wired `perAgent`/`prompts` signals,
a `setPromptAt` grower, and an `on(perAgent)` auto-isolate effect into `prompt-composer.tsx`;
`launch()` now selects per-agent bases once, resolves `addDirs` inside the loop, applies the
ultrathink suffix per prompt, and records each distinct prompt in history. UI: a toggle + a
`<For>` of override textareas + a non-blocking collision hint. Styles in `App.css`.

**Verification:** `selectPrompts`/`withUltrathink`/`promptsDiffer` unit-tested (composer.test.ts).
New `prompt-composer.test.tsx` mounts the real component with an injected agent store and
asserts the end-to-end launch: boxes appear per agent, Isolate auto-checks, and a fan-out
spawns two agents with distinct `initialPrompt`s (blank override inheriting the shared one).
tsc + vite build clean; 63 frontend + 64 Rust tests green.

**Status:** complete. Frontend-only; Rust core untouched.

## "Open in editor" button — COMPLETE

**Context:** First slice toward the in-app code-editor idea (Part 2 of the per-agent-prompts
plan): rather than embed an editor, let the user open an agent's changes in the editor they
already use. Cheapest thing that closes the review loop.

**Approach (respects the Rust/UI boundary):** new `src-tauri/src/editor.rs` with a pure,
unit-tested `build_open_command(editor, path)` — splits the editor command on whitespace,
canonicalizes the path (must exist; path-traversal/typo guard), appends it as the final arg,
never a shell (LOOPS XV). A thin `open_in_editor(path)` Tauri command loads settings, defaults
the editor to `code`, and spawns it detached. New `editor_command` field on `AppSettings`
(state.rs + ipc.ts + settings-panel field). UI: an "Open in editor" button on the agent bar
in `App.tsx` opening `a.worktree?.path ?? a.cwd`, errors surfaced inline.

**Verification:** `build_open_command` unit tests (4). Drove the release app on a virtual
display with `editorCommand` pointed at a fake editor script; clicking the button logged
`opened: /tmp/demo-web-shop` — the full chain (button → IPC → command → spawn → correct
canonical path) confirmed. 68 Rust + 63 frontend tests green; lint clean.

**Status:** complete.

## Dark-mode dropdown contrast fix — COMPLETE

**Context:** In dark mode the composer's Backend / Run-in `<select>`s showed no visible text
(dark-on-dark). Root cause: no `color-scheme` was declared anywhere, so WebKitGTK rendered native
controls with its default *light* theme while the app's CSS painted a dark `#2a2a2a` background —
the native value text stayed dark and vanished.

**Fix:** declared `color-scheme: light` on base `:root` and `color-scheme: dark` in the
`prefers-color-scheme: dark` `:root` block (`src/App.css`), so native controls theme with the page.

**Verification:** ran the release build on a dark virtual display (`GTK_THEME=Adwaita:dark`) and
captured a frame — both selects now render "Claude" / "web-shop" in legible white. Also visible in
`demo/autodev-per-agent-prompts-demo.mp4`.

## Live epic validation + two fixes it surfaced — COMPLETE

**Context:** Every autonomy piece was unit-tested but never driven live end-to-end. Ran a real
epic (tiny Python string-utils lib) in the app on a virtual display with live `claude` agents.
It surfaced two bugs that only a real run exposes; both fixed with regression tests, then the epic
re-ran to **PASSED** (3 features decomposed → planned → built → verified; retry path exercised;
`python3 test_strutils.py` green; agents committed real code). Resolves the "not driven live" caveat.

**Fixes:**
1. **Loop roles must exit for auto-advance to fire.** Interactive `claude` never exits, so the
   chain stalled at `decomposing`. Added `AgentOptions.print_mode` → `command_line` emits
   `claude -p` (one-shot; runs, prints, exits; plan mode dropped since read-only roles are enforced
   by their prompts). loop-panel spawns all loop roles with `printMode: true`; generator/evaluator
   also `bypassPermissions` (they write files / run tests), decomposer/planner read-only. +1 test.
2. **`strip_ansi` CRLF regression.** `-p` output is `FEATURES:\r\n1. …`; the CR-overwrite took the
   text after the trailing `\r` (empty) → every line blanked → parsers found nothing. Fixed to use
   the last non-empty `\r`-segment (Rust `loop_engine::strip_ansi` + TS `agent-store.stripAnsi`).
   +2 tests (strip_ansi CRLF, parse_features CRLF). Missed before because unit tests used `\n`.

**Status:** complete. `./dev.sh verify` green (64 Rust + 48 frontend). Live epic PASSED; evidence
in `demo/epic-passed.png`.

## LLM context compaction — COMPLETE (Phase 2 done)

**Decided:** Fix long-run context loss (the naive last-15-lines progress tail drops cross-feature
memory). Implemented as a **Summarizer** role — same role-separation pattern as decomposer/planner
/evaluator — rather than a non-LLM structured memory, since the user asked for LLM compaction and
a model produces a far richer digest (design decisions, file layout, what failed and why).
Modelled as a **maintenance step, not a phase**: it doesn't move the phase machine; it just
rewrites `progress`, then the deferred phase role runs. Threshold-triggered (`MAX_PROGRESS_CHARS`
= 2500) so it only fires on genuinely long runs.

**Built:**
- `loop_engine.rs` (+3 tests) — `Role::Summarizer`; `summarizer_prompt(spec, backlog, progress)`;
  `parse_summary` (after a `SUMMARY:` header, else the tail; bounded); `compact_progress` (replace
  memory with a marked, bounded digest); `needs_compaction` + `MAX_PROGRESS_CHARS`.
- `commands.rs` — `loop_needs_compaction`, `loop_compact_prompt` (Summarizer role + prompt),
  `loop_compact` (parse the agent's summary → replace progress); registered.
- Frontend — ipc `summarizer` role + `needsCompaction`/`MAX_PROGRESS_CHARS` + bindings; loop-panel
  `runNextRole` inserts a read-only summarizer before the next phase role in the Auto-run chain
  when progress is large, `autoAdvance` handles the `summarizer` role via `loopCompact`, and a
  manual **🗜 Compact memory** button appears over the threshold (+1 test).

**Status:** complete. `./dev.sh verify` green (62 Rust + 48 frontend). **All Phase-2 autonomy items
are done** — trust/durability, feature-epic driver, onboarding auto-responder, continue-on-failure,
and context compaction.

**Honest boundary (unchanged):** all logic is unit-tested; a full multi-hour live epic with real
agents has not been driven end-to-end (needs authenticated CLIs + the GUI).

## Continue-on-failure for epics — COMPLETE

**Decided:** Make long epics resilient — one hard feature shouldn't kill the whole backlog. Opt-in
(off by default, so fail-fast stays the honest default). Chose to represent a given-up feature with
`Feature.failed` and finish the epic as `Failed` with a partial-success summary rather than add a
new "Partial" phase (avoids enum/frontend churn; the failure_reason carries the nuance, and the
backlog shows ✓/✗ per feature).

**Built:**
- `loop_engine.rs` (+2 tests) — `Feature.failed`, `LoopState.continue_on_failure` (serde-default);
  refactored the give-up branch of `grade_and_advance` into `advance_or_finalize(succeeded)` (mark
  done/failed → next feature or finalize) + `finalize_epic` (all done ⇒ Passed; else Failed +
  "N/M features done; failed: …"). Fail-fast path unchanged when the toggle is off.
- `commands.rs`/`ipc.ts` — `loop_create` + `loopCreate` gain a `continue_on_failure` param;
  `LoopState`/`Feature` types updated.
- Frontend — composer "Continue on failure" checkbox; backlog renders failed features with a red ✗
  (+1 test).

**Status:** complete. `./dev.sh verify` green (59 Rust + 47 frontend).

**Deferred (last Phase-2 item):** LLM-based context compaction for very long runs.

## Onboarding pre-flight (auto-responder) — COMPLETE

**Decided:** Stop unattended loops from stalling on Claude Code's "trust this folder?" dialog (the
gate that hung the first demo). Two options weighed:
1. **Config pre-set** — write `hasTrustDialogAccepted` into `~/.claude.json` before spawning.
   Rejected: that file is used by the *running* Claude Code session, so a read-modify-write risks
   clobbering it concurrently, and it couples us to Claude's config schema.
2. **Auto-responder** — detect the exact trust prompt in the agent's streamed output and send Enter,
   like a human. Chosen: no global side effect, backend-agnostic mechanism, reuses the store's
   existing tail tracking.

**Built:** `agent-store.ts` — pure exported `onboardingReply(tail)` (narrow: only the trust dialog,
Enter accepts its default "Yes"; explicitly NOT the bypass warning); a per-agent `autoOnboard` set
+ `onboardSent` debounce so it fires once per gate and re-arms only when the gate clears;
`setAutoOnboard(id, on)`. loop-panel: an opt-in **Auto-onboard** toggle (off by default) that calls
`setAutoOnboard(agentId, true)` on each spawned role agent. +3 tests (pure matcher, once-then-again
behaviour, no-write-unless-enabled).

**Status:** complete. `./dev.sh verify` green (57 Rust + 46 frontend).

**Deferred (Phase 2 remainder):** LLM context compaction, continue-on-feature-failure toggle,
per-feature disk companion files.

## Feature-epic driver — COMPLETE

**Decided:** Make a loop an *epic* over a feature backlog rather than a single contract — the
Phase-2 item that enables genuinely long-running multi-feature builds. Kept it additive: the
existing Planner→Generator→Evaluator sub-loop is reused unchanged as "work one feature"; a new
Decomposer/Decomposing phase feeds the backlog first. **Fail-fast** on a failed feature (name it,
end the epic) chosen as the honest default; continue-on-failure is a later toggle. `features`
became `Vec<Feature>` (breaks old ephemeral loop state — acceptable, loops are throwaway).

**Built:**
- `loop_engine.rs` (+5 tests) — `Feature`, `Role::Decomposer`, `LoopPhase::Decomposing`,
  `current_feature`, `set_features`, `feature_title`/`backlog_overview`; `decomposer_prompt`,
  `parse_features` (generalized `parse_list(output, header)`); feature-aware planner/generator/
  evaluator prompts; `advance_feature` inside `grade_and_advance` (mark done → next feature with
  per-feature state reset, or complete epic); `prompt_for_phase` handles Decomposing.
- `commands.rs` — `loop_apply_decomposer`, `loop_set_features`; `loop_set_contract` drops its
  legacy `features` param; registered in `lib.rs`.
- Frontend — ipc `Feature`/`currentFeature`/`decomposer` role + `loopApplyDecomposer`/
  `loopSetFeatures`; loop-panel Decomposing UI (run/auto-apply/manual fallback), backlog display
  (done/current), `feature k/N` meta, epic-aware pass message; Auto-run chains through it (+1 test).

**Status:** complete. `./dev.sh verify` green (57 Rust + 43 frontend).

**Deferred (Phase 2 remainder):** LLM-based context compaction, onboarding/permission pre-flight
for unattended runs, a continue-on-feature-failure option, and disk companion files per feature.

## Loop trust & durability core — COMPLETE

**Decided:** Make the autonomous loop trustworthy over long runs by closing the three highest-
value gaps from the earlier honest assessment, keeping all decision logic pure/TDD in
`loop_engine.rs` and reusing the `transcribe.rs` pluggable-shell-command pattern for the verify
command. Scope deliberately excludes the feature-epic driver and LLM context compaction (Phase 2).

**Built:**
- `verify.rs` (new, +4 tests) — `run_verify` via `sh -c` in the project dir, exit 0 = pass, bounded
  output tail; a command that can't launch counts as a failure, not a pass.
- `loop_engine.rs` (+8 tests) — `LoopState` gains `verify_command`/`history`/`failure_reason`
  (serde-default); `met_count`, `is_stuck` (flat window OR no new best), `append_progress`
  (bounded). `grade_and_advance(state, verdicts, verify)` gates `Passed` on `all_met && verify !=
  Some(false)`, escalates a stall/out-of-cap to `Failed` with a reason. Prompts thread progress
  memory + the verify command. `new()` removed (only tests used it → clippy dead_code under
  `--all-targets`); tests use `with_options`/a `mk` helper. Default cap 5 → 8.
- `commands.rs` — `loop_create` takes `verify_command`/`max_iterations`; a shared `apply_grade`
  runs verify, records a progress line, and grades, used by both `loop_grade` and
  `loop_apply_evaluator` so manual and auto paths behave identically.
- Frontend — `ipc.ts` types + `loopCreate` params; loop-new **Verify command** + **Max rounds**
  inputs; failure-reason + verify-command shown in the detail view (+2 tests).

**Status:** complete. `./dev.sh verify` green (52 Rust + 42 frontend).

**Verification:** the verify gate is proven by unit tests end-to-end at the logic layer
(`run_verify` exit-code handling × `grade_and_advance` gating: all-PASS verdicts + `verify=false`
⇒ never `Passed`). A full live loop driving real agents was not run (same boundary as prior work —
needs authenticated CLIs + the GUI).

**Deferred (Phase 2):** feature-epic driver, LLM-based progress summarization, onboarding/permission
pre-flight so unattended agents don't stall at Claude's trust prompt.

## Open-folder-as-workspace + release helper — COMPLETE

**Decided:** Two small UX/ops conveniences. (1) A one-step "open an existing folder as a
workspace" — the store already composed `createWorkspace` + `addProject`, so this is a thin
`createFromFolder` that derives the workspace name from the folder basename and reuses the native
folder picker already wired for `+dir`. (2) Make cutting a versioned GitHub release one command
rather than manual version edits + tag, so distribution matches standard OSS practice.

**Built:**
- `workspace-store.createFromFolder(path)` (+2 tests: basename naming, trailing-slash/Windows
  separators); an **Open folder as workspace…** button in the sidebar using the dialog plugin.
- `./dev.sh release X.Y.Z` — validates semver + clean tree, bumps the version line in
  `package.json` and `src-tauri/tauri.conf.json` (targeted sed, formatting preserved — verified
  the JSON stays valid), commits, tags `vX.Y.Z`, pushes. Pairs with the existing
  `release.yml`. README/CLAUDE.md updated.
- Documented the **"ship it" = verify + commit + push** convention in CLAUDE.md (and saved it to
  agent memory); it is distinct from `./dev.sh release`.

**Status:** complete. `./dev.sh verify` green (42 Rust + 40 frontend).

## Google Antigravity backend — COMPLETE

**Decided:** Support Google Antigravity's terminal agent (`agy`). The architecture makes this a
one-adapter change (AgentAdapter seam: "add a backend = add an adapter, change nothing else"),
so the work was: confirm the *real* `agy` flags (not guess them), add an enum arm + match arm,
one dropdown option, and tests. Verified `agy`'s flags against Google's published CLI guide
(interactive `-i`/`--prompt-interactive`, `-m`, `--add-dir`, `--dangerously-skip-permissions`)
rather than inventing them — this is the only backend whose flags aren't verified against a
local install, and that caveat is called out in the `command_line` doc comment.

**Built:** `AgentBackend::Antigravity` + its `command_line` arm in `agent.rs` (+2 tests: full
flag mapping with interactive prompt/images, and the bare `agy` session); `AgentBackend` TS type
+ composer dropdown option; README agent mentions.

**Status:** complete. `./dev.sh verify` green (42 Rust + 38 frontend).

**Deliberate deferrals / notes:**
- No documented `agy` plan/read-only flag, so the composer's plan-mode toggle is a no-op for this
  backend (Claude/Codex still honor it). Map it if `agy` gains one.
- No documented `agy` image flag; screenshots are referenced in the prompt text (same as Claude).
- Flags follow Google's published guide; reconfirm against the installed `agy` if they drift.

## Gap-closing — waiting detection + signing docs — COMPLETE

**Decided:** Of the three documented gaps, one was genuinely code-fillable; be honest about the
other two.

- **`waiting` detection (filled).** The end-anchored heuristic cleared reliably but missed
  Claude/Codex's multi-line approval menus, and a whole-tail match would get *stuck* (prompt
  text lingers in the raw-byte buffer; a real terminal would have redrawn over it, but the store
  works on raw bytes, not an emulated screen). Reframed `waiting` as a **silence-derived** state:
  `pushOutput` always sets `running` (so any activity clears waiting *reliably*), and `tick()` —
  once an agent is quiet past the idle threshold — classifies the tail as `waiting` (trailing
  prompt) vs `idle`. This decouples "is it a prompt" from "has it cleared", so detection can now
  afford richer, multi-line, ANSI-stripped patterns without getting stuck. Added `stripAnsi` (TS
  mirror of the Rust one) and selection-menu patterns.
- **Code signing (not code-fillable — documented).** Requires Apple Developer ID / Windows certs,
  which are secrets. Can't be committed. Expanded the README into an actionable recipe (exact env
  vars / config keys) so a maintainer enables it by adding credentials; the build path already
  supports it.
- **Live-LLM dependency (inherent — mitigated).** The loop drives real agents, so correctness
  depends on the model following the prompts. Not "fillable"; already mitigated by tolerant
  parsers (bullets/checkboxes/`criterion N:` forms, FAIL-by-default) and the manual fallbacks.

**Built:** `agent-store` `stripAnsi` + reworked `detectWaiting` + silence-derived `tick`; README
"Code signing & notarization" subsection. `./dev.sh verify` green (40 Rust + 38 frontend).

## Hardening — remaining items (diff wiring, settings UI, status) — COMPLETE

**Decided:** Clear the rest of the deferrals in one pass.

- **Evaluator diff.** Give the evaluator the actual changes of a generation round. Chose to
  record the project's HEAD as `base_commit` when the loop *enters* Generating (a deterministic
  save point) and compute `git diff <base>` (work tree vs. base — committed *and* uncommitted)
  when the evaluator prompt is built. A retry re-bases on the new HEAD so each round's diff is
  just that round. Non-repo project dirs degrade to an empty diff. Parsing/logic stays in the
  tested `git` module; `loop_current_prompt` composes it.
- **Settings UI.** The pluggable commands were only editable by hand in the JSON. Added a
  `SettingsPanel` modal over the existing `get_settings`/`set_settings` commands — no new core
  surface. Blank command fields save as `null` so "configured" vs "empty command" stays
  unambiguous.
- **Richer status.** Split terminal state into `exited` (clean/killed) vs `error` (non-zero
  code) — deterministic and useful. Added `waiting` from a conservative, END-anchored prompt
  heuristic (`detectWaiting`, pure + unit-tested): a prompt only counts while it is the last
  thing printed, so answering it flips back to `running`. Kept patterns narrow to avoid false
  positives; the Claude TUI's multi-line approval prompt may need a dedicated pattern later.

**Built:** `git::head_commit`/`diff_since` (+1 test); `LoopState.base_commit` + capture/diff in
`commands.rs`; `settings-panel.tsx` + ⚙ button (+2 tests); `agent-store` `error`/`waiting`
statuses, `detectWaiting`, `isTerminal` (+4 tests); README distribution section.

**Status:** complete. `./dev.sh verify` green (40 Rust + 37 frontend).

**Deliberate deferrals:**
- `waiting` detection is heuristic and end-anchored; it won't catch every TUI prompt shape.
- Code signing/notarization for release bundles is left unconfigured (documented in the README).

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
