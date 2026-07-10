# CLAUDE.md — AutoDev

Guidance for any AI agent working in this repo. Read this, then read `PLAN.md` for the
build roadmap. **The engineering method is not optional: follow `LOOPS.md` on every
task.** This file is project-specific; `LOOPS.md` is the general discipline.

## What this is

AutoDev is a desktop agentic development environment. It runs and manages many terminal
coding agents (Claude Code, Codex) in parallel across multiple project workspaces, plus
voice-to-text, screenshot/annotate, and browser-handoff tools around them. See `PLAN.md`
for capabilities and the phased plan.

## Method — read `LOOPS.md` first

`LOOPS.md` holds 40 rules across 7 tiers. Non-negotiable highlights for this repo:

- **Scope lock (IV)** is rule #1. Touch only what the task needs. Never change stacks,
  configs, or deps that were not asked for.
- **Read before you write (I)**, **think before you code (II)**, **plan for 3+ steps (XIII)**.
- **TDD (XII)**: tests first, RED-GREEN-REFACTOR. Rust core with `cargo test`, UI with
  Vitest. Every behavior that can break has a failing-without-the-change test.
- **Verify (V)**: never say "works" without running it. This is a GUI app driving real
  processes; prove it by launching it, not by reading the diff.
- **No stubs (XI)**: complete code, no TODO/placeholder. Grep changed files before "done".
- **Traceability (Tier 5)**: update `CHANGELOG.md`, `IMPLEMENT.md`, and `HANDOFF.md` as
  the last step of every functional task.

## Stack (do not change without asking — LOOPS IV)

- **Tauri (Rust core)** — native shell, process control, PTYs, filesystem, git, capture.
- **SolidJS + TypeScript** — frontend UI only. Fine-grained reactivity for many live
  terminals and status cards.
- **portable-pty** — spawn agent CLIs as real PTYs.
- **xterm.js** — terminal rendering in the frontend; mount into a DOM node, stream PTY
  bytes in over a Tauri event channel, send keystrokes back via a command.
- **Flat JSON on disk** for state (`~/.autodev/`); reach for SQLite only when JSON hurts.
- **Shell out to `git`** from Rust for worktrees; no libgit2 dependency.
- Voice + browser-handoff backends are pluggable; keep the seam, add one first.

Minimize dependencies (LOOPS VIII). The standard library and the crates above cover most
needs. Justify any new crate or npm package in the PR/changelog.

## Architecture rules

- **Hard boundary: Rust core = all process/fs/PTY/git; frontend = pure UI.** No process
  spawning or filesystem access from the frontend. Everything crosses via typed Tauri
  commands (request/response) and events (streams).
- **Typed contract in one place.** Every command's args and return type is defined once
  in Rust and mirrored in a shared TS type module. Frontend and core must not drift.
- **Agent backends live behind an adapter.** Nothing outside `AgentAdapter`
  implementations knows how Claude Code or Codex is invoked (flags, plan mode, bypass,
  effort). Add a backend = add an adapter, change nothing else.
- **ProcessManager owns every child process lifetime (LOOPS XXXVIII).** Explicit owner,
  explicit kill on quit. No orphaned agent processes — that is a tested invariant.
- **Time and randomness only in the Rust core**, never the frontend.
- **Continuous input is a correctness concern (LOOPS XXXIX).** Terminal streams and
  status updates: batch PTY bytes in Rust, update only the changed signal in Solid, do
  not re-render the world per byte.

## Security (LOOPS XV)

This app spawns processes and can run agents in bypass/yolo mode. Be strict:
- Never build a shell command by string-concatenating user/agent input. Pass args as a
  vector to the process API; no `sh -c` with interpolated paths.
- Validate and canonicalize any path that comes from the UI before touching the fs
  (path traversal).
- Bypass/yolo mode is powerful. Make its state obvious in the UI and never the silent
  default.

## Working docs (create in Phase 0, keep current)

- `CHANGELOG.md` — every functional change, newest first, dated (LOOPS XXIV).
- `IMPLEMENT.md` — decision-to-code audit trail (LOOPS XXV).
- `HANDOFF.md` — one rolling doc: current state, what runs, what's broken, the one
  command to verify, the next step (LOOPS XXVI).

## Build / test / run

Use `dev.sh` — it is the single entry point and handles the snap env scrub needed to
launch the app on this machine:

- `./dev.sh setup` — install npm + cargo dependencies.
- `./dev.sh dev` — run the app with hot reload (do NOT call `npm run tauri dev` directly
  from a snap shell; it crashes on snap's libpthread — see README).
- `./dev.sh test` — Vitest (frontend) + `cargo test` (core).
- `./dev.sh lint` — eslint + tsc + clippy + rustfmt check.
- `./dev.sh verify` — everything CI runs; run this before every commit.
- `./dev.sh release X.Y.Z` — bump the version, tag `vX.Y.Z`, and push; CI builds the
  GitHub release. This is how versioned binaries are distributed (see README).

Keep this section accurate as it changes (LOOPS XXVI).

**"Ship it" means sync the code.** When the user says "ship it" (or "ship"), run
`./dev.sh verify`, then commit the work and push to `origin` (fast-forward-merge the branch
into `main` first if on a feature branch). It is not a request to cut a version release —
that is only `./dev.sh release X.Y.Z`.

## Naming

Frontend files/folders, scripts, configs, and on-disk state files are
**lowercase-hyphenated** (`terminal-pane.tsx`, `feature-list.json`, `dev.sh`). **Rust
module files use snake_case** (`agent_adapter.rs`, `process_manager.rs`) — hyphens are
invalid in Rust module names, and per LOOPS XXXVI we use the target language's idioms
rather than fight them. Uppercase docs `CLAUDE.md` and `LOOPS.md` keep their fixed names.

## Style

Match the surrounding code. Default to no comments; add one only when the *why* is
non-obvious (LOOPS XXII). Any text that leaves the app or repo (README, PR bodies,
release notes) reads like a human wrote it — no AI slop (LOOPS XX).
