import type { AgentOptions } from "./ipc";

/** AutoDev's public agent-lifecycle hook bus (P3).
 *
 *  Extensions and built-in behaviors participate in an agent's life through a small typed
 *  set of hooks. `spawn` is a *transform* — it may rewrite the launch options before the
 *  process starts (the analog of Pi's `before_provider_headers`) — while the rest are
 *  *observers*. A throwing hook is contained: it never breaks a launch or the other hooks,
 *  so a bad extension degrades to a no-op rather than taking down the app.
 *
 *  The seam lives in the frontend because that is where orchestration already happens (the
 *  agent store classifies output and reacts to exits); the Rust core stays pure process/PTY.
 */

/** Rewrite launch options before the agent starts. Hooks compose in registration order —
 *  each receives the previous hook's result. */
export type SpawnHook = (options: AgentOptions) => AgentOptions;
/** Observe an agent's streamed output. `tail` is the recent decoded (still-raw) text. */
export type OutputHook = (id: string, tail: string) => void;
/** React to an agent settling (idle) or parking on a prompt (waiting). */
export type StatusHook = (id: string) => void;
/** React to an agent exiting; `code` is its exit code (null if unknown/signalled). */
export type ExitHook = (id: string, code: number | null) => void;

export interface HookBus {
  /** Register a spawn transform. Returns an unregister function. */
  onSpawn(fn: SpawnHook): () => void;
  onOutput(fn: OutputHook): () => void;
  onIdle(fn: StatusHook): () => void;
  onWaiting(fn: StatusHook): () => void;
  onExit(fn: ExitHook): () => void;

  /** Run every spawn transform in order and return the final options. */
  applySpawn(options: AgentOptions): AgentOptions;
  emitOutput(id: string, tail: string): void;
  emitIdle(id: string): void;
  emitWaiting(id: string): void;
  emitExit(id: string, code: number | null): void;
}

export function createHookBus(): HookBus {
  const spawn: SpawnHook[] = [];
  const output: OutputHook[] = [];
  const idle: StatusHook[] = [];
  const waiting: StatusHook[] = [];
  const exit: ExitHook[] = [];

  const register = <T>(list: T[], fn: T): (() => void) => {
    list.push(fn);
    return () => {
      const i = list.indexOf(fn);
      if (i >= 0) list.splice(i, 1);
    };
  };

  const safe = (fn: () => void) => {
    try {
      fn();
    } catch (e) {
      console.warn("[hooks] handler threw", e);
    }
  };

  return {
    onSpawn: (fn) => register(spawn, fn),
    onOutput: (fn) => register(output, fn),
    onIdle: (fn) => register(idle, fn),
    onWaiting: (fn) => register(waiting, fn),
    onExit: (fn) => register(exit, fn),

    applySpawn(options) {
      return spawn.reduce((opts, fn) => {
        try {
          return fn(opts) ?? opts;
        } catch (e) {
          console.warn("[hooks] spawn handler threw", e);
          return opts;
        }
      }, options);
    },
    emitOutput(id, tail) {
      for (const fn of output) safe(() => fn(id, tail));
    },
    emitIdle(id) {
      for (const fn of idle) safe(() => fn(id));
    },
    emitWaiting(id) {
      for (const fn of waiting) safe(() => fn(id));
    },
    emitExit(id, code) {
      for (const fn of exit) safe(() => fn(id, code));
    },
  };
}
