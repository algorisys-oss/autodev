# Scaling plan: many agents, cheap when idle

Status: planned, not started. Written 2026-11-24.

## Why

The question was whether the ideas in Rui ([repo](https://github.com/DivyanshGolyan/rui),
[post](https://divyanshgolyan.com/posts/hundred-agents/)) apply to AutoDev. Rui is a headless
agent host in Zig. It runs every agent loop in one process, so a waiting agent costs about
2.5 KB. Its main idea needs AutoDev to host the agent loop itself, not wrap Claude Code or Codex
in a PTY. That is a separate decision, deferred to its own plan (see "Out of scope").

Some of the smaller ideas do apply, along with one gap found while checking them: each PTY
read becomes its own IPC event, and the webview does work on output from every agent, even
the ones nobody is looking at. CLAUDE.md already asks for "batch PTY bytes in Rust", and the
code doesn't do it yet. The goal of this plan is that an agent nobody is watching costs close
to nothing in the webview, with numbers to show it.

## What already matches Rui

No work needed here; these are listed so nobody redoes them.

- One clock for all agents: a single state-watcher thread polls every agent every 500 ms
  (`src-tauri/src/lib.rs`, `start_state_watcher`).
- Send only what changed: `AgentManager::poll_states` emits state transitions only.
- Bounded memory per agent: frontend scrollback is capped at 1 MB (`MAX_BUFFER_BYTES` in
  `src/lib/agent-store.ts`), and the detection screen is capped in `src-tauri/src/detection.rs`.

## Current output path

1. `src-tauri/src/agent.rs`, `spawn_session`: one reader thread per agent reads up to 8 KB
   and calls `on_output` once per read.
2. `src-tauri/src/commands.rs`, `on_output`: writes the disk log, feeds the Rich driver,
   base64-encodes the chunk and emits one `agent://output` event.
3. `src/lib/agent-store.ts`, `pushOutput`: for every chunk from every agent, decodes base64,
   appends to the 1 MB replay buffer, decodes UTF-8 into a 600-char tail, runs the
   `onOutput` hooks, and writes to the xterm if a pane is attached.

So the cost is one IPC message plus JS work per read, per agent, whether or not the agent is
on screen.

## Steps

Each step follows LOOPS: tests first, verify by launching, then update `changelog.md`,
`implement.md` and `handoff.md`.

### 1. Benchmark (do first)

Without a baseline, later steps can't be shown to help.

- Add a way to start N mock agents that write output at a set rate. The `Mock` backend
  plus a small output generator script under `examples/` or `src-tauri/src/bin/` is enough.
  Drive it through headless mode (`./dev.sh headless`) for the core numbers, and through the
  GUI for webview numbers.
- Measure, for N = 1, 10, 50, 100:
  - Private memory (PSS from `/proc/<pid>/smaps_rollup`), summed over the app, the webview
    process and all agent children. Not RSS: RSS counts shared pages once per process. This
    is the method from the Rui post.
  - CPU of the app process and the webview process.
  - `agent://output` events per second.
  - State-detection lag: time from the mock agent printing a prompt to the `agent://state`
    event. Today this can be up to `TICK_MS` (500 ms) after the `QUIET_AFTER_MS` (1500 ms)
    quiet window.
- Put the script in `dev.sh` (for example `./dev.sh bench [N]`) and record the baseline in
  `implement.md`.

Done when: one command prints the table above for a given N, and the baseline is recorded.

### 2. Batch PTY output in Rust

- Keep the reader thread per agent (the reads block, and a thread per PTY is cheap at this
  scale). Change what happens after the read: append the bytes to a per-agent pending buffer
  and flush it as one event every 16 ms, or sooner once it reaches 64 KB.
- The disk log and the detection tracker can stay per read (they're in-process and cheap).
  The Rich driver keeps working on whole lines, so it is unaffected by batching.
- Flush the pending buffer before `agent://exit`, so the last bytes arrive before the exit
  event. Add a test for that ordering.
- Put the batcher in the core, not in the Tauri layer, so headless mode gets it too (it
  uses the same `spawn_session` callbacks).
- No new dependency: a `Mutex<Vec<u8>>` plus a deadline check is enough. One flusher thread
  for all agents (the same pattern as the state watcher) is preferable to one timer per agent.

Tests (`cargo test`):
- Many small writes within one window come out as one event with the same bytes in order.
- A write over 64 KB flushes immediately.
- Bytes written just before exit are emitted before the exit callback.
- No bytes are lost or reordered across flushes (property-style test with random chunk sizes).

Done when: the benchmark shows events per second per agent bounded by about 60 under
continuous output, and terminals still render correctly (check by launching with real Claude
and Codex sessions).

### 3. Hidden agents cost nothing in the webview

Move the replay buffer into the core and stream bytes only for terminals that are mounted.

- Core keeps the 1 MB ring buffer per agent (same cap as today).
- New commands, with types in Rust mirrored in the shared TS module:
  - `agent_attach(id)`: returns the buffered bytes and starts streaming that agent's output.
  - `agent_detach(id)`: stops streaming.
- `TerminalPane` calls attach on mount and detach on cleanup. `agent-store.ts` drops its
  `buffers` and `bufferBytes` maps.
- Race to handle: output that arrives between the replay snapshot and the start of streaming.
  Take the snapshot and switch on streaming under the same lock. Test it.

The catch is the `onOutput` hook. It is part of the public extension API
(`src/lib/hooks.ts`, documented in the help panel), and it receives the output tail of every
agent. The built-in onboarding auto-accept in `agent-store.ts` is its first user. Plan:

- Move the onboarding-gate check into the core (it already has the per-agent screen in
  `detection.rs`), so the built-in behavior no longer needs every byte in JS.
- Keep `onOutput` for extensions, but feed it from a low-rate event: the core sends each
  agent's current tail with state ticks, only when the tail changed. Document the change in
  delivery rate (the hook then sees snapshots, not every chunk) in the help panel and the
  changelog. If an extension needs every byte, that's a new, opt-in subscription; don't
  add it until someone asks.

Done when: with 100 agents running and one terminal open, the webview does no per-chunk
work for the other 99 (the benchmark's webview CPU and event rate show it), the onboarding
gate still auto-accepts, and switching between agents still shows full scrollback.

### 4. Cap concurrent agents (admission slots)

Today `task_split` caps how many units one request creates, but nothing caps the total
number of live agents. Loops and fan-outs can spawn without limit.

- Add a setting `maxConcurrentAgents` (default high, for example 32; 0 = no limit).
- A spawn over the cap is queued in the core as data (its options only, no process). It
  appears in the UI as "Queued" and starts when a slot frees up. The user can cancel it.
- Manual spawns from the UI could bypass the cap; decide when implementing. Default: they
  respect it, and the UI says why the agent is queued.
- Queued agents need no ProcessManager entry. Killing the app drops the queue.

Tests: spawning over the cap queues; an exit starts the next queued agent in FIFO order;
cancelling a queued agent removes it without starting it.

Done when: a loop that wants 50 agents with a cap of 8 runs 8 at a time and finishes all 50.

### 5. Tune state detection latency (only if step 1 says so)

The tick is 500 ms and the quiet window is 1500 ms. If the benchmark shows the lag matters
(for example, the bell or the Waiting badge feels late), consider waking the state watcher
when a reader goes quiet instead of waiting for the next tick. Skip this step if the numbers
say it doesn't matter. The Rui post measured its own polling as 9.5% of turn time and still
kept polling.

## Out of scope

- **One process runs every agent loop (native backend).** Means AutoDev hosts the agent loop:
  model API clients, tools, context management, session persistence, API-key billing instead
  of subscription login. To be scoped in its own plan later. The normalized `AgentEvent` model
  and the Rich view are the seam it would plug into.
- **Saving state before acting on it (Rui's durability).** PTY children die with the app, so
  there is little to recover. Revisit with the native backend.
- **Replacing the reader thread per agent with async I/O.** Not worth it at 100 agents; the
  threads are mostly idle and blocked in `read`.

## Order and size

| Step | Size | Depends on |
|---|---|---|
| 1. Benchmark | small | — |
| 2. Batch output in Rust | small | 1 (to show the gain) |
| 3. Hidden agents cost nothing | medium | 2 |
| 4. Admission slots | small to medium | — |
| 5. Detection latency | small | 1, only if needed |

Steps 1 and 2 can ship together as a patch release. Step 3 changes the `onOutput` hook's
delivery rate, so ship it as a minor release with a clear changelog note.
