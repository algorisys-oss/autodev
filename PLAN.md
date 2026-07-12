# AutoDev — Build Plan

An agentic development environment: a desktop app for running and managing many
terminal coding agents (Claude Code, Codex) in parallel across multiple project
workspaces. Modeled on the multi-agent workflow shown in the reference video, plus the
voice, screenshot, and browser-handoff tools around it.

Dev methodology for every task in this plan lives in `LOOPS.md`. This file is the
*what and in what order*; `LOOPS.md` is the *how*.

---

## What we are building (from the source video)

The workflow the product has to support, in the builder's own terms:

- Open a **workspace** = a set of project directories on disk. Keep many projects
  side by side (API, app, UI, MCP) so any of them can be `@`-referenced for context.
- **Launch N agents at once**, mixing backends (e.g. 4 Claude Code + 4 Codex). Each
  agent runs a real CLI in a real directory.
- Drive each agent with a prompt. Add context by `@`-mentioning directories/files
  (the agent then "listed directory, listed directory" to pull the right files).
- **Difficulty heuristic**: task difficulty 1→10 decides how many agents and whether
  to plan first. Difficulty 1 = one agent, no plan. Difficulty 6 = plan mode, single
  strong model. Difficulty 10 = many sub-agents + plan mode + ultrathink.
- Per-agent knobs: **plan mode**, **bypass/yolo permissions** (walk away and let it
  run), **effort level** (high / extra-high), **ultrathink**.
- **Git worktree isolation** when agents might collide; skip it when they won't.
- **Voice-to-text** prompting that saves spoken prompts for reuse.
- **Screenshot + annotate** to attach visual context, cuts hallucinations.
- **Browser handoff**: an agent writes a handoff prompt, a browser AI executes it.
  Reproduce as a generated-handoff + optional in-app browser automation.
- Fix production errors by pasting the error + a screenshot back into the agent.

## Core capabilities we will ship

1. Workspace + multi-project management with `@`-mention context resolution.
2. Multi-agent orchestration: spawn, monitor, focus, and prompt many CLI agents.
3. Pluggable agent backends (Claude Code, Codex) behind one adapter interface.
4. Prompt composer with difficulty→agents heuristic, mode toggles, prompt history.
5. Git worktree isolation with a merge-back flow.
6. Voice-to-text capture.
7. Screenshot + annotate → attach to a prompt.
8. Browser handoff / automation.
9. Autonomous loop engine from `LOOPS.md` (Planner / Generator / Evaluator).

---

## Architecture

Tauri, split cleanly with a hard boundary between process control (Rust core) and UI
(web frontend). All process spawning, filesystem, and PTY work lives in Rust; the
frontend is pure UI talking to it over typed Tauri commands and events.

```
┌─ Core (Rust / Tauri backend) ──────────────────────────────────┐
│  ProcessManager   spawn/kill agent CLIs via portable-pty       │
│  AgentAdapters    ClaudeCode | Codex — launch flags, status    │
│  WorkspaceStore   projects, presets, on-disk state (JSON)      │
│  GitService       worktree create / status / merge-back        │
│  CaptureService   screen/region screenshots                    │
│  VoiceService     mic → transcription backend (pluggable)      │
│  LoopEngine       Planner/Generator/Evaluator (later phase)    │
└────────── Tauri commands (request) + events (stream) ──────────┘
┌─ Frontend (SolidJS + TS) ──────────────────────────────────────┐
│  WorkspaceTree · AgentGrid (cards) · TerminalPane (xterm.js)   │
│  PromptComposer (@-mention, difficulty, modes) · Annotator     │
│  Settings                                                       │
└────────────────────────────────────────────────────────────────┘
```

PTY bytes stream Rust → frontend over a Tauri event channel and render into xterm.js;
keystrokes go frontend → Rust via a command that writes to the PTY.

### Stack (decided)

| Concern | Choice | Why |
|---|---|---|
| Shell | **Tauri (Rust core)** + Vite | lightweight native shell; small binary, low memory vs Electron |
| UI | **SolidJS + TypeScript** | fine-grained reactivity for many live terminals + status cards; tiny runtime |
| PTYs | **portable-pty** (wezterm crate) | mature Rust PTY; agents behave exactly as in a terminal |
| Terminal render | **xterm.js** | framework-agnostic; mount into a DOM node, stream PTY bytes in |
| State (frontend) | **Solid stores/signals** | built in; no extra state lib (LOOPS III) |
| State (disk) | **flat JSON first**, SQLite only if it hurts | simplicity first (LOOPS III) |
| Git | shell out to `git` from Rust | worktrees are a CLI feature; no libgit2 dep (LOOPS VIII) |
| Browser handoff | **Playwright** (sidecar) | later phase; scriptable browser control |
| Voice | pluggable: local whisper.cpp *or* API | start with one, keep the seam |
| Tests | **Rust `cargo test`** (core) + **Vitest** (UI) + **WebDriver/tauri-driver** (e2e) | TDD per LOOPS XII |

Time and randomness live in the Rust core only. The frontend stays pure UI.

---

## Phases

Ordered so the riskiest, most load-bearing piece (driving a real CLI agent from a PTY)
is proven early on a few files before anything is built on top of it (LOOPS XXXVI).
Each phase ships working, tested, and is a natural stopping point.

### Phase 0 — Foundation
- Tauri + Vite + SolidJS + TS skeleton; Rust core / frontend split.
- Typed command + event bridge; one module that defines every command's contract,
  mirrored by a shared TS type set so the frontend and core agree.
- Disk-state module (Rust): `~/.autodev/` with `workspaces.json`, `settings.json`.
- Test harness (`cargo test` + Vitest + tauri-driver e2e), lint (clippy + eslint), CI.
- `git init` the repo (currently not version-controlled — do this first, LOOPS XXIII).
- Seed `CHANGELOG.md`, `IMPLEMENT.md`, `HANDOFF.md` (LOOPS Tier 5).
- **Done:** app boots to an empty window; `npm test` and lint pass in CI.

### Phase 1 — Workspaces & projects
- Workspace model: a named set of project directories. Add / open / remove projects.
- Presets that open a workspace pointed at a directory.
- WorkspaceTree UI; project browser.
- `@`-mention resolver: given `@ProjectName` or a path, resolve the files an agent
  should see.
- **Done:** create a workspace with 2+ projects, restart, state restored from disk.

### Phase 2 — Single agent session (the core — de-risk here)
- `AgentAdapter` trait (Rust): `launch(dir, opts) → session`, status parsing, teardown.
- Claude Code adapter: spawn via portable-pty into a project dir with the right flags
  (plan mode, bypass permissions, effort).
- TerminalPane renders the session with xterm.js; keystrokes go back to the PTY via a
  Tauri command, output streams in over an event channel.
- Send a prompt, capture output, detect idle vs running vs waiting-for-input.
- **Done:** launch one Claude Code agent in a real dir, prompt it, watch it work and
  finish, all inside the app. Prove this on a throwaway project before Phase 3.

### Phase 3 — Multi-agent orchestration
- Spawn N sessions at once; AgentGrid of cards with live status.
- Focus/switch between agents; per-agent scrollback and logs to disk.
- Codex adapter behind the same interface.
- Mixed launch (e.g. 4 + 4). Clean shutdown of all sessions on quit.
- **Done:** run 6+ agents across 2 projects, each independently promptable, statuses
  correct, no orphaned processes after quit.

### Phase 4 — Prompt composer, context, modes
- Composer with `@`-mention picker (autocomplete over workspace projects/files).
- Difficulty selector 1→10 that suggests agent count + plan mode (the video heuristic).
- Mode toggles: plan mode, bypass/yolo, ultrathink, effort (high / extra-high).
- Prompt history, saved/reusable prompts.
- **Done:** compose a prompt with two `@`-mentions and mode toggles, fan it to the
  suggested number of agents, verify each launched with the right flags.

### Phase 5 — Git worktree isolation
- Per-agent option to run in an isolated worktree (`git worktree add`).
- Show worktree/branch per agent; detect dirty state.
- Merge-back flow: review diff, merge into target branch, prune worktree.
- Keep destructive git out of any automated lane (LOOPS XXXVI).
- **Done:** two agents edit the same repo in separate worktrees without collision,
  then merge back cleanly through the UI.

### Phase 6 — Voice-to-text
- Mic capture; pluggable transcription backend (local whisper.cpp first, API seam).
- Insert transcript into the composer; saved spoken-prompt library.
- **Done:** speak a prompt, see accurate text in the composer, replay a saved one.

### Phase 7 — Screenshot + annotate
- Region/window capture via desktopCapturer.
- Canvas annotator: arrow, box, text, freehand.
- Attach the image to a prompt for image-capable agents.
- **Done:** capture a region, draw an arrow, attach it, agent receives the image.

### Phase 8 — Browser handoff
- Agent generates a structured handoff prompt for a browser task.
- Optional Playwright-driven execution of that handoff in a controlled browser.
- **Done:** an agent writes a handoff, the browser executes a simple scripted task
  (e.g. fill a form) end to end.

### Phase 9 — Autonomous loop engine (LOOPS Tier 6)
- Roles in separate contexts: **Planner** → spec, **Generator** → code, **Evaluator**
  → grades the diff only, told the code is broken and to prove it.
- Contract-first: `contract.md` of testable "done" assertions negotiated before code.
- On-disk loop state: `feature-list.json`, `progress.md`, `contract.md`, `log.md`.
- Restart-on-sideways; taste rubric scoring for subjective work.
- **Done:** point the loop at a small spec, walk away, come back to a build graded
  against a contract with a full trace on disk.

### Phase 10 — Auto-split: intelligent parallel decomposition
- The app decides *on its own* whether a task fans out across independent agents, instead of
  the user guessing the count — the missing "smart enough to use many agents even when not
  asked" layer (large batch jobs, independent edits).
- A pre-launch, one-shot **read-only classifier** (reuses the decomposer round-trip) inspects
  the prompt and — permitted — the working dir, and returns a `TaskPlan`: inferred **difficulty**
  (1–10), a **parallel** verdict, and one self-contained sub-prompt per unit. Distinct from the
  loop's decomposer, which emits a *serial* backlog.
- Prompt + parsing in Rust (`task_split.rs`, pure); the composer's **✨ Auto-split** button
  pre-fills the existing per-agent fan-out (parallel → N units + Isolate; not parallel → 1) and
  the inferred difficulty. Human reviews the split before Launch — nothing runs automatically.
- Opt-in **Auto-split on Launch** setting: the first Launch analyzes + pauses for review, unless
  the task is already split or the agent count was set by hand.
- **Done:** type "convert every video in ./media", Auto-split, see one unit per real file
  pre-filled across agents; type a cohesive bug fix, see it stay a single agent.

---

## Risks & how we handle them

- **Agent status detection is fuzzy.** CLIs signal "done" differently. Adapters parse
  per-backend; start with idle-timeout + prompt-marker detection, refine from real
  traces (LOOPS XXXIII), do not over-engineer up front.
- **Process leaks.** Many PTYs = zombie risk. ProcessManager owns lifecycle; kill-all
  on quit is a Phase 3 acceptance test.
- **Backend CLIs change flags.** Isolate every flag inside its adapter; nothing else
  knows how Claude Code or Codex is invoked.
- **Scope creep.** Full ecosystem is large. Phases 6–9 are independent; the product is
  useful and shippable after Phase 5. Do not start a later phase until the current one
  is done and tested (LOOPS IV, XI).
- **Tauri PTY plumbing.** Streaming many PTYs over Tauri events is the load-bearing
  bit. Prove one clean stream (Phase 2) before fanning out. portable-pty is mature;
  the risk is our event/backpressure wiring, so keep byte-batching in the Rust core.
- **Rust learning curve on adapters.** The agent adapters and ProcessManager are the
  gnarliest Rust. De-risk them on one backend first (LOOPS XXXVI), lifetimes and
  process ownership owned explicitly (LOOPS XXXVIII).

## First step

Phase 0. Confirm this plan, then scaffold the Tauri/Vite/SolidJS/TS skeleton, wire one
typed command + event round-trip, `git init`, and stand up the test harness. No product
features until the skeleton boots green.
