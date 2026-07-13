# AutoDev — Extensibility Plan (Pi parity + beyond)

A design proposal for making AutoDev **as extensible as Pi**
([earendil-works/pi](https://github.com/earendil-works/pi)) — and, where AutoDev's
orchestrator nature allows, *past* what Pi can structurally do. This is a *proposal to
decide on*, not committed work. Nothing is scheduled until we pick phases and fold them
into `PLAN.md`. Method for any adopted phase lives in `LOOPS.md`.

---

## End goal

> **AutoDev is extensible at both layers, and rich context is a fleet-wide primitive.**
>
> 1. **Orchestrator layer** — you add a backend, a lifecycle hook, or a composer template
>    by dropping a file, never by editing core.
> 2. **Harness layer** — Pi's ecosystem (tools, MCP, `pi-annotate`, …) is reachable by
>    embedding Pi as one backend, wherever its extensions bridge cleanly.
> 3. **Beyond Pi** — structured, live-DOM annotation is captured *once* and dispatched to
>    *many* agents across *any* backend — a capability a single harness like Pi cannot
>    have.

Everything below is the path to those three sentences.

---

## The reframe: two layers, and why it governs the whole plan

Pi and AutoDev are **different categories of tool**.

- **Pi is a *harness*.** It owns the LLM loop: `pi-ai` (provider API), `pi-agent-core`
  (turn loop, tools, state, events), `pi-tui`, `pi-coding-agent`. `read`/`write`/`bash`
  are Pi's *own* tools.
- **AutoDev is an *orchestrator*.** It runs no LLM loop. It spawns **other people's agent
  CLIs** (`claude`, `codex`, `agy`) as real PTYs and supervises many of them in parallel
  (`src-tauri/src/agent.rs`, `src/lib/agent-store.ts`).

So there are **two distinct extensibility layers**, and every decision in this plan comes
back to which one a capability lives in:

| Layer | What it extends | How AutoDev gets it | Scope |
|---|---|---|---|
| **Harness** | tools, MCP, `pi-annotate`, sub-agents — *inside one agent's loop* | **Embed Pi** as a backend (P7); inherit its extensions | Pi cells only |
| **Orchestrator** | backends, cross-agent hooks, fan-out, composer templates, **fleet-wide context** | **Build it** (P1/P3/P4/P5/P6/P9) | All backends |

**The load-bearing consequence:** embedding Pi (P7) is a cheap *force multiplier* but it
only ever lights up **inside Pi cells**. Anything that must work across `claude`, `codex`,
and `agy` too — including cross-agent annotation — is orchestrator-layer and must be built.
The orchestrator layer is also AutoDev's unique value; Pi has no equivalent. **So the
orchestrator track is the commitment; the Pi track is an opportunistic bonus.**

---

## Proof case: `pi-annotate` — and the capability it points to

[`nicobailon/pi-annotate`](https://github.com/nicobailon/pi-annotate) is a **third-party,
`npm`-installable Pi extension** (`pi install npm:pi-annotate`, invoked as
`/annotate [url]`). It adds a DevTools-style element picker that captures **structured DOM
context** — selectors, box model, accessibility (role/name/ARIA), computed styles — plus
per-element/full-page screenshots with numbered badges, draggable comment cards, and an
"Etch" mode that records DevTools edits into **before/after property diffs the agent maps
back to source**. Architecture: Pi extension (`index.ts`) ↔ native host (`host.cjs`, Unix
socket) ↔ Chrome extension (native messaging) → structured markdown.

Two things it establishes:

1. **It validates the extension model.** Nobody edited Pi's core to add a whole
   capability — the payoff of P1/P3/P5/P8.
2. **It points past itself.** `pi-annotate` is harness-scoped: it feeds *one* agent. The
   moment you want that context **across a fleet**, you've moved to the orchestrator layer
   — where AutoDev is differentiated and Pi structurally cannot follow. That is P9, and it
   is the sharpest reason to build native (see below).

---

## Current-state audit (grounded in the code)

**Already the right shape:**
- Hard Rust-core / Solid-UI split with one typed IPC contract (`src/lib/ipc.ts` mirrored
  in Rust) — Pi's layered separation, already present.
- A live event stream (`agent://output`, `agent://exit`, `src/lib/agent-store.ts:195`) —
  raw material for a public hook system.
- **A shared-context path already fans to many agents:** the composer attaches screenshots
  (`images` in `AgentOptions`) and spawns N agents from one prompt. Cross-agent context
  dispatch *exists today* — for images. P9 upgrades that artifact, it doesn't invent it.

**The gaps:**
1. **The "adapter" is aspirational.** `CLAUDE.md`/`PLAN.md` promise *"add a backend =
   add an adapter, change nothing else."* Reality: `command_line()` at
   `src-tauri/src/agent.rs:68` is a hardcoded `match` over an enum. Adding a backend edits
   the enum + match + tests.
2. **Features are baked into core, not composed over primitives** (`task_split`,
   `loop_engine`, onboarding auto-accept at `src/lib/agent-store.ts:183`).
3. **No extension loading, no public hook API, no headless/RPC mode, no shareable
   packages, no structured/cross-agent annotation.**

---

## The path to the end goal — one ordered sequence with gates

The trick that makes "do both" tractable: **the two layers share one root, P1.** Build the
adapter once; the Pi backend is just a spec on top of it, and the orchestrator hooks wrap
*all* backends including Pi. So the path is a single spine (M0), a cheap decision gate
(M1), then two tracks that interleave on that spine.

```
M0  P1  Real adapter ───────────────► shared root for BOTH layers
                │
M1  Pi spike (gate) ── does an extension bridge when Pi is embedded?
        ├─ pass ─► Track B viable (P7)
        └─ fail ─► Track B dropped; capability comes only from Track A
                │
        ┌───────┴───────────────────────────────┐
Track A (COMMIT, all backends)          Track B (BONUS, Pi cells)
  P3 hooks                                P7 embed Pi + inherit extensions
  P4 templates / skills                        (gated on M1 pass)
  P9 cross-agent structured annotation
  P5 extension loading
  P6 headless / RPC mode
  P8 shareable packages
```

### The steps, in order

1. **M0 — P1: make the adapter real.** *(foundation for both layers)*
2. **M1 — Pi spike (gate).** Time-boxed; decides how much of Track B is free.
3. **Track A, in order: P3 → P4 → P9 → P5 → P6 → P8.** The committed spine. Delivers the
   end goal's orchestrator layer and the "beyond Pi" annotation regardless of M1.
4. **Track B, gated on M1 pass: P7.** The bonus. Delivers the harness layer. Interleaves
   with Track A whenever it's ready — it depends only on P1, not on Track A's later phases.

Minimum path to a *meaningful* end goal: **P1 → P3 → P4 → P9.** P5/P6/P8 harden it into a
platform; P7 adds Pi's ecosystem on top.

---

## Phase details

### P1 — Real backend adapter *(M0; foundation for both layers)*
Replace the `match` in `command_line` with a **registry + declarative `BackendSpec`**
(schema below). A new CLI becomes a *data file*. Built-in `claude`/`codex`/`agy`/`mock`
ship as bundled specs; the existing `agent.rs` tests become the conformance suite and must
pass unchanged.
→ **Done:** add a backend by dropping one JSON file, get the right arg vector, zero Rust
edits.

### M1 — Pi spike *(the gate; not a phase, a few days)*
Write a throwaway Pi `BackendSpec` pointing at Pi's RPC/JSONL mode, launch Pi as an
AutoDev cell, and try to run `pi-annotate` **end to end** through it.
→ **Done:** a written verdict — does a Pi *extension* survive embedding, or just a bare Pi
session? Three exits: bridge works (P7 is cheap) / bridge fragile (P7 gives bare Pi only;
lean on P9) / feels wrong (drop P7).

### P3 — Public hook lifecycle *(Track A)*
Formalize the event stream into hooks that observe or mutate:
`on_spawn(options) -> options` (mutate launch args *before* start — analog of Pi's
`before_provider_headers`), `on_output`, `on_idle`/`on_waiting`, `on_exit`. Prove the seam
by reimplementing the loop's auto-advance and onboarding auto-accept as the **first two
hook consumers**.
→ **Done:** a config-registered hook changes an agent's launch flags and reacts to its
exit; the two built-ins run *through* the hook API.

### P4 — Prompt templates + skills *(Track A; cheap, high ROI)*
Composer slash-commands expanding to saved templates (`~/.autodev/templates/*.md`); a
skills dir passed to underlying agents via `--add-dir`.
→ **Done:** `/refactor` expands in the composer; a launched agent has the skills dir on its
context path.

### P9 — Cross-agent structured annotation *(Track A; the "beyond Pi" differentiator)*
**Not a `pi-annotate` port.** Build annotation as an **orchestrator-level capture
artifact**: capture structured DOM context *once* (selectors, box model, a11y, edit diffs —
`pi-annotate`'s payload) and dispatch it to *N agents on any backend*, reusing the existing
`images`/composer fan-out path. The cross-agent dimension is free (it's the orchestrator
layer); the **capture bridge is the real cost** — an element picker + native-host + browser
extension, which cannot be inherited cross-backend.

- **Additive to P7, not an alternative.** P7 gives per-Pi-cell annotate; P9 gives
  fleet-wide annotate on every backend. They solve different halves.
- **Reuse the M1 spike:** check whether `pi-annotate`'s *capture bridge* (the hard part)
  is portable enough to feed an AutoDev-side artifact, before building capture from
  scratch. Don't build capture twice (LOOPS IV).
- **Security (LOOPS XV):** a browser bridge + native host is an attack surface; scope the
  trust model up front.
→ **Done:** annotate a live page once, fan the *structured* context (not a PNG) to 3+
agents across two backends, each launched with it.

### P5 — Extension loading *(Track A; platform-scale)*
`~/.autodev/extensions/` of JS/TS modules loaded at startup, registering backend specs
(P1), hooks (P3), composer commands/templates (P4), UI panels. **Honest scope:** Tauri
makes this harder than Pi's single-process `jiti` — frontend extensions run in the Solid
app; core-side hooks need a defined Rust boundary or a sidecar. Its own `PLAN.md` phase,
not a smuggled task. Decide the trust/security model first (LOOPS XV).
→ **Done:** a third-party folder adds a backend + hook + composer command with no edits to
the AutoDev source tree.

### P6 — Headless / RPC mode *(Track A; turns app into platform)*
Expose the orchestrator over JSONL (stdin/stdout or a local socket) — Pi's RPC/SDK
equivalent.
→ **Done:** drive a full spawn → prompt → observe → kill cycle from a shell script, no GUI.

### P7 — Embed Pi as a backend *(Track B; gated on M1 pass)*
Promote the M1 spike to a real Pi `BackendSpec`. AutoDev inherits Pi's extension ecosystem
**inside Pi cells**.
→ **Done (the honest bar):** launch a Pi cell **and prove a Pi extension (`pi-annotate`)
runs through it** — not merely that Pi started.

### P8 — Shareable packages *(Track A; opportunistic)*
A manifest bundling backend specs + hooks + templates + themes; install by dropping a
folder. Mirrors Pi packages.
→ **Done:** install a package folder and get its backend, template, and hook at once.

---

## Sketch — `BackendSpec` (P1)

Illustrative; pinned down when P1 is picked. Must reproduce the *exact* arg vectors in
`agent.rs` today.

```json
{
  "id": "claude",
  "program": "claude",
  "models": ["claude-opus-4-8", "claude-sonnet-5"],
  "flags": {
    "printMode":         { "when": true,  "args": ["-p"] },
    "planMode":          { "when": true,  "args": ["--permission-mode", "plan"],
                           "skipWhen": "printMode" },
    "bypassPermissions": { "when": true,  "args": ["--dangerously-skip-permissions"] },
    "model":             { "template": ["--model", "{value}"] },
    "addDirs":           { "eachTemplate": ["--add-dir", "{value}"] }
  },
  "images":  { "mode": "appendToPrompt", "text": "\n\n[Screenshot attached: {path}]" },
  "prompt":  { "mode": "positional" }
}
```

`codex` sets `images.mode = "flag"` (`-i {path}`); `agy` adds the cwd-into-workspace rule.
The vocabulary is exactly what the four existing backends need — no more (LOOPS VIII).

---

## Guardrails & open decisions

**Guardrails:**
- **Scope (LOOPS IV):** P5 and P6 are platform-scale; don't let them ride in on a smaller
  task. Don't build the annotation capture twice (P7 inherit vs P9 native) — check
  portability at M1 first.
- **Security (LOOPS XV):** P5 (third-party JS) and P9 (browser bridge + native host) are
  attack surfaces; trust models are prerequisites, not afterthoughts.
- **Don't over-build the spec (LOOPS VIII):** `BackendSpec` describes today's four
  backends, not a hypothetical CLI grammar.

**Open decisions for us:**
1. Is the Pi track (P7) worth it, or is complementing Pi via P1–P4 + P9 enough?
2. Is headless/RPC (P6) a real goal, or is AutoDev GUI-only?
3. Where does the core-vs-extension line fall (P2/P5) — is `loop_engine` core or an
   extension?
4. Extension runtime for P5: reuse the frontend's TS, a sidecar, or Rust dylibs?
5. P9 capture bridge: reuse `pi-annotate`'s (if M1 shows it's portable) or build native?

---

*Status: proposal. Nothing scheduled. On decision, fold chosen phases into `PLAN.md` and
record the rationale in `IMPLEMENT.md`.*
