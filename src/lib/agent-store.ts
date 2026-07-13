import { createStore } from "solid-js/store";
import * as ipc from "./ipc";
import { base64ToBytes } from "./bytes";
import { createHookBus } from "./hooks";

export type AgentStatus = "running" | "idle" | "waiting" | "exited" | "error";

/** A process that has stopped for good — clean or crashed. */
export function isTerminal(status: AgentStatus): boolean {
  return status === "exited" || status === "error";
}

/** Strip ANSI escape sequences and carriage-return redraws so prompt text is legible. Mirrors
 *  the Rust `loop_engine::strip_ansi`; the frontend needs it to read prompts out of raw PTY
 *  bytes (TUI agents wrap everything in colour and cursor codes). */
export function stripAnsi(input: string): string {
  let out = "";
  let i = 0;
  while (i < input.length) {
    if (input[i] === "\x1b") {
      const next = input[i + 1];
      if (next === "[") {
        // CSI: ESC [ ... <final byte 0x40..0x7e>
        i += 2;
        while (i < input.length && !(input[i] >= "\x40" && input[i] <= "\x7e")) i++;
        i++;
      } else if (next === "]") {
        // OSC: ESC ] ... terminated by BEL or ST (ESC \)
        i += 2;
        while (i < input.length) {
          if (input[i] === "\x07") {
            i++;
            break;
          }
          if (input[i] === "\x1b" && input[i + 1] === "\\") {
            i += 2;
            break;
          }
          i++;
        }
      } else {
        i += 2; // other two-char escape
      }
      continue;
    }
    out += input[i];
    i++;
  }
  // Apply carriage-return overwrites per line — the last NON-EMPTY `\r`-segment, so a trailing
  // `\r` from CRLF (`foo\r\n`) keeps `foo` instead of blanking the line.
  return out
    .split("\n")
    .map((line) => {
      const parts = line.split("\r").filter((s) => s !== "");
      return parts.length ? parts[parts.length - 1] : "";
    })
    .join("\n");
}

// Signals that an agent is parked on the user: shell y/n prompts, "press enter", and the
// interactive selection menus Claude Code and Codex render for permission/approval (a `❯`/`>`
// cursor on a numbered option, or the arrow-key hint). Matched against the last few lines of
// recent output; clearing is handled by "any output ⇒ running", not by the prompt scrolling off.
const WAITING_PATTERNS = [
  /Do you want to proceed\?/i,
  /Do you want to continue\?/i,
  /\bPress\s+enter\s+to\s+continue/i,
  /\by\s*\/\s*n\b/i,
  /\(y\/n\)/i,
  /\[y\/N\]/i,
  /[❯▶►›➤]\s*\d+[.)]\s/, // selection cursor on a numbered option (Claude/Codex approval menu)
  /\(use\s+arrow\s+keys\)/i,
  /\bNo,\s+and\s+(tell|keep)\b/i, // the "No, and tell Claude…" menu option
];

const WAITING_SCAN_LINES = 6; // how many trailing non-empty lines to inspect for a prompt

/** Does the recent output tail look like the agent is waiting for the user to answer a prompt?
 *  Only the last few non-empty lines are considered — a prompt means the agent is currently
 *  parked at it. */
export function detectWaiting(tail: string): boolean {
  const lines = stripAnsi(tail)
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  const recent = lines.slice(-WAITING_SCAN_LINES).join("\n");
  return WAITING_PATTERNS.some((re) => re.test(recent));
}

// Onboarding gates an unattended agent can safely clear itself, and the keystrokes that accept
// them. Kept deliberately narrow: only Claude Code's "trust this folder?" dialog, shown on first
// access to a new directory — the gate that stalls loops in fresh worktrees/projects. Enter
// selects its default "Yes, proceed". NOT the bypass-permissions warning (whose default is No).
const ONBOARDING_ACCEPTS: { pattern: RegExp; reply: string }[] = [
  { pattern: /Is this a project you created or one you trust\?/i, reply: "\r" },
  { pattern: /Do you trust the files in this folder\?/i, reply: "\r" },
];

/** If the recent output is a known, safe onboarding gate, the keystrokes that accept it (so an
 *  unattended run doesn't stall on it); otherwise null. */
export function onboardingReply(tail: string): string | null {
  const text = stripAnsi(tail);
  for (const { pattern, reply } of ONBOARDING_ACCEPTS) {
    if (pattern.test(text)) return reply;
  }
  return null;
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
  /** Launched in Rich mode: renders as structured cards from `events` rather than a terminal. */
  rich: boolean;
  /** Normalized structured events for a Rich session, in arrival order (empty for terminal ones). */
  events: ipc.AgentEvent[];
  /** The backend's conversation id (from a Rich session's SessionInit), used to resume for a
   *  follow-up turn. Undefined until the first SessionInit arrives. */
  sessionId?: string;
  /** Tool allow/deny posture this session launched with, carried into follow-up turns so a
   *  resumed turn can't silently regain a blocked tool (B1). */
  allowedTools?: string[];
  disallowedTools?: string[];
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
  const autoOnboard = new Set<string>(); // agents allowed to auto-clear onboarding gates
  const onboardSent = new Set<string>(); // debounce: a reply is in flight for this agent's gate
  const decoder = new TextDecoder();
  // A Rich follow-up runs as a fresh one-shot backend process (a new core id) that `--resume`s
  // the same conversation. This maps that follow-up process's id back to the conversation agent
  // so its output/events/exit land on the original card, keeping one continuous stream.
  const resumeMap = new Map<string, string>();
  const conv = (coreId: string) => resumeMap.get(coreId) ?? coreId;

  // Public agent-lifecycle hook bus (P3). The store emits spawn/output/idle/waiting/exit
  // through it; built-in behaviors and extensions register as consumers.
  const hooks = createHookBus();

  const indexOf = (id: string) => state.agents.findIndex((a) => a.id === id);

  function setStatus(id: string, status: AgentStatus) {
    const i = indexOf(id);
    if (i >= 0 && state.agents[i].status !== status) setState("agents", i, "status", status);
  }

  function pushOutput(rawId: string, bytes: Uint8Array) {
    const id = conv(rawId); // route a follow-up process's output onto its conversation card
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
    // Fresh output ⇒ running. This also reliably clears an earlier "waiting"/"idle" the moment
    // the agent acts again. Whether it is *now* waiting on a prompt is decided in tick(), once
    // it goes quiet — so the prompt-text lingering in the tail can't keep it stuck on waiting.
    if (i >= 0 && !isTerminal(state.agents[i].status)) setStatus(id, "running");
    hooks.emitOutput(id, tail);
    subscribers.get(id)?.(bytes);
  }

  // Built-in output hook: on unattended runs, auto-accept a known onboarding gate once so the
  // agent doesn't stall. The first consumer of the hook bus — proof the seam carries real work.
  hooks.onOutput((id, tail) => {
    const i = indexOf(id);
    if (i < 0 || isTerminal(state.agents[i].status) || !autoOnboard.has(id)) return;
    const reply = onboardingReply(tail);
    if (reply && !onboardSent.has(id)) {
      onboardSent.add(id);
      void Promise.resolve(api.agentWrite(id, reply)).catch(() => {});
    } else if (!reply) {
      onboardSent.delete(id); // gate cleared — ready for the next one
    }
  });

  const unlistens: Promise<() => void>[] = [
    subscribe<{ id: string; data: string }>("agent://output", (p) =>
      pushOutput(p.id, base64ToBytes(p.data)),
    ),
    subscribe<{ id: string; code: number | null }>("agent://exit", (p) => {
      const id = conv(p.id);
      resumeMap.delete(p.id); // a follow-up turn's process is done
      const i = indexOf(id);
      if (i >= 0) {
        setState("agents", i, "exitCode", p.code);
        // A non-zero code is a crash/failure; a clean or signalled (killed) exit is "exited".
        setStatus(id, p.code && p.code !== 0 ? "error" : "exited");
        hooks.emitExit(id, p.code);
      }
    }),
    // Rich sessions also emit normalized structured events; append them for the card view.
    subscribe<{ id: string; event: ipc.AgentEvent }>("agent://event", (p) => {
      const i = indexOf(conv(p.id));
      if (i < 0) return;
      setState("agents", i, "events", state.agents[i].events.length, p.event);
      // Capture the backend conversation id so a follow-up can resume it.
      if (p.event.kind === "sessionInit" && p.event.sessionId) {
        setState("agents", i, "sessionId", p.event.sessionId);
      }
    }),
  ];

  /** Once an agent goes quiet, classify the silence: a trailing prompt in its output means it
   *  is blocked on the user (waiting); otherwise it is merely idle. Called by an interval in the
   *  app; tests call it. Fresh output flips it back to running via pushOutput. */
  function tick() {
    const t = now();
    for (const a of state.agents) {
      if (isTerminal(a.status)) continue;
      if (t - (lastActivity.get(a.id) ?? 0) > IDLE_AFTER_MS) {
        const next = detectWaiting(tails.get(a.id) ?? "") ? "waiting" : "idle";
        if (a.status !== next) {
          setStatus(a.id, next);
          if (next === "waiting") hooks.emitWaiting(a.id);
          else hooks.emitIdle(a.id);
        }
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
    // Let spawn hooks rewrite the launch options (e.g. inject a context dir) before launch.
    const opts = hooks.applySpawn(options);
    const id = await api.agentSpawn(opts);
    buffers.set(id, []);
    bufferBytes.set(id, 0);
    lastActivity.set(id, now());
    setState("agents", state.agents.length, {
      id,
      label,
      backend: opts.backend,
      cwd: opts.cwd,
      status: "running",
      exitCode: null,
      worktree,
      rich: !!opts.rich,
      events: [],
      allowedTools: opts.allowedTools,
      disallowedTools: opts.disallowedTools,
    });
    setState("focusedId", id);
    return id;
  }

  /** Send a follow-up turn in a Rich conversation. Spawns a fresh one-shot backend process that
   *  `--resume`s the captured session and routes its events back onto this same card, so the
   *  conversation reads as one continuous stream. No-op if the agent has no captured session id. */
  async function followUp(agentId: string, text: string): Promise<void> {
    const i = indexOf(agentId);
    if (i < 0) return;
    const a = state.agents[i];
    if (!a.sessionId) return;
    // Show the user's turn inline and flip the conversation back to running.
    setState("agents", i, "events", a.events.length, { kind: "userMessage", text });
    setState("agents", i, "exitCode", null);
    setStatus(agentId, "running");
    lastActivity.set(agentId, now());
    const opts = hooks.applySpawn({
      backend: a.backend,
      cwd: a.cwd,
      rich: true,
      resumeSessionId: a.sessionId,
      initialPrompt: text,
      allowedTools: a.allowedTools,
      disallowedTools: a.disallowedTools,
    });
    const coreId = await api.agentSpawn(opts);
    resumeMap.set(coreId, agentId);
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
    autoOnboard.delete(id);
    onboardSent.delete(id);
    for (const [coreId, convId] of resumeMap) if (convId === id) resumeMap.delete(coreId);
    if (state.focusedId === id) setState("focusedId", state.agents[0]?.id ?? null);
  }

  function focus(id: string) {
    setState("focusedId", id);
  }

  /** Allow (or stop) an agent from auto-accepting known onboarding gates (unattended runs). */
  function setAutoOnboard(id: string, on: boolean) {
    if (on) autoOnboard.add(id);
    else autoOnboard.delete(id);
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
    hooks,
    spawn,
    followUp,
    kill,
    killAll,
    close,
    focus,
    setAutoOnboard,
    attach,
    detach,
    tick,
    start,
    dispose,
  };
}
