import { describe, it, expect, vi } from "vitest";
import { createRoot } from "solid-js";
import {
  createAgentStore,
  detectWaiting,
  isTerminal,
  stripAnsi,
  type Subscribe,
} from "./agent-store";
import type * as ipc from "./ipc";

/** Base64 of a string's UTF-8 bytes — how the Rust core actually emits agent output (btoa
 *  alone throws on non-Latin1 chars like the ❯ menu cursor). */
const b64 = (s: string) => btoa(String.fromCharCode(...new TextEncoder().encode(s)));

/** Fake ipc + a controllable event bus so tests drive agent://* events deterministically. */
function harness() {
  let n = 0;
  const handlers: Record<string, (p: unknown) => void> = {};
  const subscribe: Subscribe = async (event, cb) => {
    handlers[event] = cb as (p: unknown) => void;
    return () => {};
  };
  const emit = (event: string, payload: unknown) => handlers[event]?.(payload);
  const api = {
    agentSpawn: vi.fn(async () => `agent-${++n}`),
    agentKill: vi.fn(async () => {}),
    agentKillAll: vi.fn(async () => 0),
  } as unknown as typeof ipc;
  let clock = 1000;
  return { subscribe, emit, api, now: () => clock, setClock: (t: number) => (clock = t) };
}

describe("agent store", () => {
  it("spawn adds an agent, focuses it, running", async () => {
    await createRoot(async (dispose) => {
      const h = harness();
      const store = createAgentStore({ api: h.api, subscribe: h.subscribe, now: h.now });
      const id = await store.spawn({ backend: "claude", cwd: "/p" }, "proj");
      expect(id).toBe("agent-1");
      expect(store.state.agents).toHaveLength(1);
      expect(store.state.focusedId).toBe("agent-1");
      expect(store.focused()?.status).toBe("running");
      dispose();
    });
  });

  it("buffers output and replays it to a terminal attached later", async () => {
    await createRoot(async (dispose) => {
      const h = harness();
      const store = createAgentStore({ api: h.api, subscribe: h.subscribe, now: h.now });
      await store.spawn({ backend: "mock", cwd: "/p" }, "m");
      h.emit("agent://output", { id: "agent-1", data: btoa("hello ") });
      h.emit("agent://output", { id: "agent-1", data: btoa("world") });

      const got: string[] = [];
      store.attach("agent-1", (b) => got.push(new TextDecoder().decode(b)));
      expect(got.join("")).toBe("hello world");

      // live chunk after attach reaches the subscriber
      h.emit("agent://output", { id: "agent-1", data: btoa("!") });
      expect(got.join("")).toBe("hello world!");
      dispose();
    });
  });

  it("marks an agent idle after silence, running again on new output", async () => {
    await createRoot(async (dispose) => {
      const h = harness();
      const store = createAgentStore({ api: h.api, subscribe: h.subscribe, now: h.now });
      await store.spawn({ backend: "claude", cwd: "/p" }, "p");
      h.emit("agent://output", { id: "agent-1", data: btoa("x") });
      expect(store.focused()?.status).toBe("running");

      h.setClock(1000 + 2000); // advance past IDLE_AFTER_MS
      store.tick();
      expect(store.focused()?.status).toBe("idle");

      h.emit("agent://output", { id: "agent-1", data: btoa("y") });
      expect(store.focused()?.status).toBe("running");
      dispose();
    });
  });

  it("exit event sets exited + code and does not flip back to running", async () => {
    await createRoot(async (dispose) => {
      const h = harness();
      const store = createAgentStore({ api: h.api, subscribe: h.subscribe, now: h.now });
      await store.spawn({ backend: "claude", cwd: "/p" }, "p");
      h.emit("agent://exit", { id: "agent-1", code: 0 });
      expect(store.focused()?.status).toBe("exited");
      expect(store.focused()?.exitCode).toBe(0);

      // late output must not resurrect an exited agent
      h.emit("agent://output", { id: "agent-1", data: btoa("z") });
      expect(store.focused()?.status).toBe("exited");
      dispose();
    });
  });

  it("a non-zero exit code is an error, and does not resurrect on late output", async () => {
    await createRoot(async (dispose) => {
      const h = harness();
      const store = createAgentStore({ api: h.api, subscribe: h.subscribe, now: h.now });
      await store.spawn({ backend: "claude", cwd: "/p" }, "p");
      h.emit("agent://exit", { id: "agent-1", code: 1 });
      expect(store.focused()?.status).toBe("error");
      expect(isTerminal(store.focused()!.status)).toBe(true);

      h.emit("agent://output", { id: "agent-1", data: btoa("late") });
      expect(store.focused()?.status).toBe("error");
      dispose();
    });
  });

  it("classifies silence as waiting when parked on a prompt, else idle; output clears it", async () => {
    await createRoot(async (dispose) => {
      const h = harness();
      const store = createAgentStore({ api: h.api, subscribe: h.subscribe, now: h.now });
      await store.spawn({ backend: "claude", cwd: "/p" }, "p");

      // A Claude-style multi-line approval menu (prompt line + options), still actively drawing.
      h.emit("agent://output", {
        id: "agent-1",
        data: b64("Do you want to proceed?\n❯ 1. Yes\n  2. No, and tell Claude what to do\n"),
      });
      expect(store.focused()?.status).toBe("running");

      // It goes quiet at the menu → waiting (not idle).
      h.setClock(1000 + 2000);
      store.tick();
      expect(store.focused()?.status).toBe("waiting");

      // The user answers; the agent acts again → running, and staying quiet now is plain idle.
      h.emit("agent://output", { id: "agent-1", data: btoa("Editing src/app.ts\n".repeat(8)) });
      expect(store.focused()?.status).toBe("running");
      h.setClock(1000 + 5000);
      store.tick();
      expect(store.focused()?.status).toBe("idle");
      dispose();
    });
  });

  it("detectWaiting matches prompts and TUI menus but not ordinary output", () => {
    expect(detectWaiting("... Do you want to proceed?")).toBe(true);
    expect(detectWaiting("Continue? (y/n)")).toBe(true);
    expect(detectWaiting("Press enter to continue")).toBe(true);
    // Claude/Codex selection menu, prompt line followed by options (not at the very end).
    expect(
      detectWaiting("Do you want to proceed?\n❯ 1. Yes\n  2. No, and keep going\n"),
    ).toBe(true);
    // Colourised menu — ANSI codes must not defeat detection.
    expect(detectWaiting("\x1b[1mSelect an option\x1b[0m\n\x1b[36m❯ 1.\x1b[0m Approve")).toBe(
      true,
    );
    expect(detectWaiting("(Use arrow keys)")).toBe(true);
    expect(detectWaiting("Running tests, 42 passed.")).toBe(false);
    expect(detectWaiting("What is the meaning of life?")).toBe(false);
    expect(detectWaiting("Step 1. clone the repo")).toBe(false);
  });

  it("stripAnsi removes escape codes and applies carriage-return redraws", () => {
    expect(stripAnsi("\x1b[31mred\x1b[0m text")).toBe("red text");
    expect(stripAnsi("\x1b]0;title\x07hi")).toBe("hi");
    expect(stripAnsi("loading...\rdone")).toBe("done");
  });

  it("close removes an agent and reselects", async () => {
    await createRoot(async (dispose) => {
      const h = harness();
      const store = createAgentStore({ api: h.api, subscribe: h.subscribe, now: h.now });
      await store.spawn({ backend: "claude", cwd: "/a" }, "a");
      await store.spawn({ backend: "codex", cwd: "/b" }, "b");
      expect(store.state.agents).toHaveLength(2);
      store.close("agent-2");
      expect(store.state.agents.map((a) => a.id)).toEqual(["agent-1"]);
      expect(store.state.focusedId).toBe("agent-1");
      dispose();
    });
  });

  it("killAll delegates to the core", async () => {
    await createRoot(async (dispose) => {
      const h = harness();
      const store = createAgentStore({ api: h.api, subscribe: h.subscribe, now: h.now });
      await store.spawn({ backend: "claude", cwd: "/a" }, "a");
      await store.killAll();
      expect(h.api.agentKillAll).toHaveBeenCalled();
      dispose();
    });
  });
});
