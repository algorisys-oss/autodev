import { describe, it, expect, vi } from "vitest";
import { analyzeTask } from "./task-split";
import type { Subscribe } from "./agent-store";
import type { AgentOptions, TaskPlan } from "./ipc";

const PLAN: TaskPlan = {
  difficulty: 6,
  parallel: true,
  units: [
    { title: "a", prompt: "do a", mentions: [] },
    { title: "b", prompt: "do b", mentions: [] },
  ],
  rationale: "independent",
};

/** Fake ipc + a controllable `agent://exit` bus. */
function harness(plan: TaskPlan | null) {
  const handlers: Record<string, (p: unknown) => void> = {};
  const subscribe: Subscribe = async (event, cb) => {
    handlers[event] = cb as (p: unknown) => void;
    return () => delete handlers[event];
  };
  const emitExit = (id: string) => handlers["agent://exit"]?.({ id, code: 0 });
  const api = {
    taskSplitPrompt: vi.fn<(task: string, projects: string[]) => Promise<string>>(() =>
      Promise.resolve("CLASSIFIER PROMPT"),
    ),
    agentSpawn: vi.fn<(opts: AgentOptions) => Promise<string>>(() => Promise.resolve("agent-1")),
    taskSplitParse: vi.fn<(id: string) => Promise<TaskPlan | null>>(() => Promise.resolve(plan)),
    agentKill: vi.fn<(id: string) => Promise<void>>(() => Promise.resolve()),
  };
  return { subscribe, emitExit, api };
}

describe("analyzeTask", () => {
  it("spawns a print-mode classifier with the Rust-built prompt, then returns the parsed plan", async () => {
    const h = harness(PLAN);
    const p = analyzeTask(
      { task: "transcode all videos", cwd: "/p", backend: "claude", projects: ["media"] },
      { api: h.api, subscribe: h.subscribe, timeoutMs: 5000 },
    );
    // Let the spawn + subscription settle, then fire the exit event.
    await Promise.resolve();
    await Promise.resolve();
    h.emitExit("agent-1");
    const plan = await p;

    expect(plan).toEqual(PLAN);
    expect(h.api.taskSplitPrompt).toHaveBeenCalledWith("transcode all videos", ["media"]);
    const opts = h.api.agentSpawn.mock.calls[0][0];
    expect(opts.printMode).toBe(true);
    expect(opts.initialPrompt).toBe("CLASSIFIER PROMPT");
    expect(opts.cwd).toBe("/p");
    expect(h.api.taskSplitParse).toHaveBeenCalledWith("agent-1");
    expect(h.api.agentKill).not.toHaveBeenCalled();
  });

  it("returns null when the classifier produced no plan block", async () => {
    const h = harness(null);
    const p = analyzeTask(
      { task: "x", cwd: "/p", backend: "claude", projects: [] },
      { api: h.api, subscribe: h.subscribe, timeoutMs: 5000 },
    );
    await Promise.resolve();
    await Promise.resolve();
    h.emitExit("agent-1");
    expect(await p).toBeNull();
  });

  it("on timeout kills the hung classifier but still attempts a parse", async () => {
    vi.useFakeTimers();
    try {
      const h = harness(null);
      const p = analyzeTask(
        { task: "x", cwd: "/p", backend: "claude", projects: [] },
        { api: h.api, subscribe: h.subscribe, timeoutMs: 1000 },
      );
      await vi.advanceTimersByTimeAsync(1001);
      expect(await p).toBeNull();
      expect(h.api.agentKill).toHaveBeenCalledWith("agent-1");
      expect(h.api.taskSplitParse).toHaveBeenCalledWith("agent-1");
    } finally {
      vi.useRealTimers();
    }
  });
});
