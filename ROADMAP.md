# AutoDev — Roadmap / Backlog

Forward-looking work, newest thinking first. Companion to `PLAN.md` (the phased build plan) and
`PI-PARITY-PLAN.md` (the extensibility/parity phases). This file is the single place to see
what's queued after the current release; `handoff.md` holds the live per-task state.

_As of v0.12.0 (headless/RPC shipped)._

## Recently shipped

- **v0.11.0** — the Rich view: opt-in structured card view for agents (Claude), the **Codex**
  driver (multi-backend seam), **multi-turn follow-ups** (`--resume`), **tool permissions**
  (B1 pre-launch allow/deny lists, B2 per-action approve/deny via Claude's `PreToolUse` hook),
  the dynamic branch footer, and a new app icon.
- **v0.12.0** — **headless / RPC mode** (P6): drive the orchestrator over JSONL on stdin/stdout
  with no GUI (`autodev-headless` / `./dev.sh headless`).

## Next up — prioritized

### 1. Harden the Rich view (patch-sized, high user value)
The v0.11.0 features are new to real use; these are the edges they'll hit.
- **Approval-dir cleanup on session close** — `<data_dir>/approvals/<agent-id>/` currently
  accumulates; remove it when the agent is closed/killed.
- **Per-tool "always allow this tool"** — a third button on the approval card that adds the tool
  to the session's allow-list so it stops prompting (needs carrying an evolving allow-set).
- **`Edit`/`Write` tool cards as diffs** — render file-editing tool calls as a proper diff rather
  than a one-line arg summary.
- **Codex `reasoning` items → `Thinking`** — currently unmapped (no fixture was captured); add it.
- **Windows approval hook** — the B2 hook is a POSIX shell script; add a `.cmd`/PowerShell variant
  so approvals work on Windows.

### 2. Headless / RPC extensions (small, compounding)
Makes the P6 seam richer and lets more flows be verified from a script.
- **Structured-event variant** — parse Rich (stream-json) output server-side and emit typed
  `event: "richEvent"` lines, so scripts get normalized events instead of raw base64.
- **More commands** — `resize`, `backendList`, and a `respondApproval` command (so headless can
  drive the B2 approval loop too).
- **Local-socket transport** — the spec's alternative to stdin/stdout, for multi-client drivers.

### 3. Make Rich the default surface (medium)
The autonomous loop and Auto-split already run one-shot turns — exactly what the Rich view renders.
Wire the Rich card view into those flows so structured rendering isn't limited to manual sessions.

## Remaining parity roadmap (`PI-PARITY-PLAN.md`)

- **P9 — cross-agent structured annotation** *(the "beyond Pi" differentiator; large)_. Annotate a
  live page once and fan the *structured* context (selectors / box-model, not a PNG) to N agents.
  Needs a browser + native-host bridge — the one big open piece. The orchestrator-layer half
  (structured annotation fan-out) already shipped; the live-DOM element picker is the remainder.
- **P8 — shareable packages** *(opportunistic)_. A manifest bundling backend specs + hooks +
  templates + themes; install by dropping a folder.
- **P7 — embed Pi as a backend** *(Track B, deferred)_. Promote the M1 spike to a real Pi
  `BackendSpec` and prove a Pi extension (`pi-annotate`) runs through the cell. Large, Pi-only,
  overlaps P9 — kept deferred.

## Notes / guardrails

- Each item stays within the stack and architecture rules (`CLAUDE.md`): Rust core owns
  process/fs/PTY/git; typed contract mirrored in TS; backends behind the adapter; minimize deps.
- Ship in small, verified increments; prove behavior by exercising it (headless mode now makes
  many flows scriptably verifiable without the GUI).
