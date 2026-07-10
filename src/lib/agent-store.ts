import { createStore } from "solid-js/store";
import * as ipc from "./ipc";
import { base64ToBytes } from "./bytes";

export type AgentStatus = "running" | "idle" | "waiting" | "exited" | "error";

/** A process that has stopped for good — clean or crashed. */
export function isTerminal(status: AgentStatus): boolean {
  return status === "exited" || status === "error";
}

// Conservative signals that an agent is blocked on the user (Claude Code / shell confirmation
// prompts). Anchored to the END of the output so a prompt only counts while it is the last
// thing printed; once the agent gets an answer and prints more, it is no longer waiting. Kept
// narrow to avoid false positives.
const WAITING_PATTERNS = [
  /Do you want to proceed\?\s*$/i,
  /Do you want to continue\?\s*$/i,
  /\bPress\s+enter\s+to\s+continue\.?\s*$/i,
  /\(y\/n\)\s*$/i,
  /\[y\/N\]\s*$/i,
];

/** Does the recent output tail look like the agent is waiting for the user to answer a prompt? */
export function detectWaiting(tail: string): boolean {
  return WAITING_PATTERNS.some((re) => re.test(tail));
}

export interface AgentView {
  id: string;
  label: string;
  backend: ipc.AgentBackend;
  cwd: string;
  status: AgentStatus;
  exitCode: number | null;
  /** Set when the agent runs in an isolated git worktree (Phase 5). */
  worktree?: ipc.WorktreeInfo;
}

/** Injectable event subscription, so tests can drive `agent://*` events without Tauri. */
export type Subscribe = <T>(event: string, cb: (payload: T) => void) => Promise<() => void>;

const IDLE_AFTER_MS = 1500;
const MAX_BUFFER_BYTES = 1_000_000; // ~1 MB scrollback kept in memory per agent
const TAIL_CHARS = 600; // recent decoded output kept per agent for waiting-prompt detection

async function tauriSubscribe<T>(event: string, cb: (payload: T) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(event, (e) => cb(e.payload));
}

/** Central store for all live agents: their status, a replayable output buffer, and the
 *  actions to spawn/kill/focus them. One global pair of listeners feeds every agent, so a
 *  terminal that mounts later still replays everything the agent has printed. */
export function createAgentStore(deps?: {
  api?: typeof ipc;
  subscribe?: Subscribe;
  now?: () => number;
}) {
  const api = deps?.api ?? ipc;
  const subscribe = deps?.subscribe ?? tauriSubscribe;
  const now = deps?.now ?? (() => Date.now());

  const [state, setState] = createStore<{ agents: AgentView[]; focusedId: string | null }>({
    agents: [],
    focusedId: null,
  });

  const buffers = new Map<string, Uint8Array[]>();
  const bufferBytes = new Map<string, number>();
  const subscribers = new Map<string, (chunk: Uint8Array) => void>();
  const lastActivity = new Map<string, number>();
  const tails = new Map<string, string>();
  const decoder = new TextDecoder();

  const indexOf = (id: string) => state.agents.findIndex((a) => a.id === id);

  function setStatus(id: string, status: AgentStatus) {
    const i = indexOf(id);
    if (i >= 0 && state.agents[i].status !== status) setState("agents", i, "status", status);
  }

  function pushOutput(id: string, bytes: Uint8Array) {
    const arr = buffers.get(id);
    if (arr) {
      arr.push(bytes);
      let size = (bufferBytes.get(id) ?? 0) + bytes.length;
      while (size > MAX_BUFFER_BYTES && arr.length > 1) {
        size -= arr.shift()!.length;
      }
      bufferBytes.set(id, size);
    }
    lastActivity.set(id, now());
    const tail = ((tails.get(id) ?? "") + decoder.decode(bytes)).slice(-TAIL_CHARS);
    tails.set(id, tail);
    const i = indexOf(id);
    // Output never resurrects a stopped agent; otherwise reflect whether it now looks blocked
    // on the user (waiting) or is actively producing output (running).
    if (i >= 0 && !isTerminal(state.agents[i].status)) {
      setStatus(id, detectWaiting(tail) ? "waiting" : "running");
    }
    subscribers.get(id)?.(bytes);
  }

  const unlistens: Promise<() => void>[] = [
    subscribe<{ id: string; data: string }>("agent://output", (p) =>
      pushOutput(p.id, base64ToBytes(p.data)),
    ),
    subscribe<{ id: string; code: number | null }>("agent://exit", (p) => {
      const i = indexOf(p.id);
      if (i >= 0) {
        setState("agents", i, "exitCode", p.code);
        // A non-zero code is a crash/failure; a clean or signalled (killed) exit is "exited".
        setStatus(p.id, p.code && p.code !== 0 ? "error" : "exited");
      }
    }),
  ];

  /** Mark long-silent running agents idle. Called by an interval in the app; tests call it. */
  function tick() {
    const t = now();
    for (const a of state.agents) {
      if (a.status === "running" && t - (lastActivity.get(a.id) ?? 0) > IDLE_AFTER_MS) {
        setStatus(a.id, "idle");
      }
    }
  }

  let timer: ReturnType<typeof setInterval> | undefined;
  function start() {
    if (!timer) timer = setInterval(tick, 750);
  }
  function dispose() {
    if (timer) clearInterval(timer);
    unlistens.forEach((p) => void p.then((u) => u()));
  }

  async function spawn(
    options: ipc.AgentOptions,
    label: string,
    worktree?: ipc.WorktreeInfo,
  ): Promise<string> {
    const id = await api.agentSpawn(options);
    buffers.set(id, []);
    bufferBytes.set(id, 0);
    lastActivity.set(id, now());
    setState("agents", state.agents.length, {
      id,
      label,
      backend: options.backend,
      cwd: options.cwd,
      status: "running",
      exitCode: null,
      worktree,
    });
    setState("focusedId", id);
    return id;
  }

  async function kill(id: string) {
    try {
      await api.agentKill(id);
    } catch {
      /* already gone */
    }
  }

  async function killAll() {
    try {
      await api.agentKillAll();
    } catch {
      /* nothing running */
    }
  }

  /** Remove a (usually exited) agent from the list and drop its buffer. */
  function close(id: string) {
    const i = indexOf(id);
    if (i < 0) return;
    setState(
      "agents",
      state.agents.filter((a) => a.id !== id),
    );
    buffers.delete(id);
    bufferBytes.delete(id);
    subscribers.delete(id);
    lastActivity.delete(id);
    tails.delete(id);
    if (state.focusedId === id) setState("focusedId", state.agents[0]?.id ?? null);
  }

  function focus(id: string) {
    setState("focusedId", id);
  }

  /** A terminal calls this on mount: replay the buffer, then receive live chunks. */
  function attach(id: string, write: (bytes: Uint8Array) => void) {
    const arr = buffers.get(id);
    if (arr) for (const chunk of arr) write(chunk);
    subscribers.set(id, write);
  }
  function detach(id: string) {
    subscribers.delete(id);
  }

  const focused = () => state.agents.find((a) => a.id === state.focusedId) ?? null;

  return {
    state,
    focused,
    spawn,
    kill,
    killAll,
    close,
    focus,
    attach,
    detach,
    tick,
    start,
    dispose,
  };
}
