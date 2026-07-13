import { describe, it, expect, vi } from "vitest";
import { createHookBus } from "./hooks";
import type { AgentOptions } from "./ipc";

const opts = (over: Partial<AgentOptions> = {}): AgentOptions => ({
  backend: "claude",
  cwd: "/p",
  ...over,
});

describe("hook bus", () => {
  it("applySpawn composes transforms in registration order", () => {
    const bus = createHookBus();
    bus.onSpawn((o) => ({ ...o, addDirs: [...(o.addDirs ?? []), "/skills"] }));
    bus.onSpawn((o) => ({ ...o, model: "big" }));
    const out = bus.applySpawn(opts());
    expect(out.addDirs).toEqual(["/skills"]);
    expect(out.model).toBe("big");
  });

  it("applySpawn returns the input unchanged when no hooks are registered", () => {
    const bus = createHookBus();
    const o = opts();
    expect(bus.applySpawn(o)).toBe(o);
  });

  it("a throwing spawn hook is skipped, later hooks still run", () => {
    const bus = createHookBus();
    bus.onSpawn(() => {
      throw new Error("boom");
    });
    bus.onSpawn((o) => ({ ...o, cwd: "/recovered" }));
    expect(bus.applySpawn(opts()).cwd).toBe("/recovered");
  });

  it("emit fans out to observers, and unregister stops them", () => {
    const bus = createHookBus();
    const seen: string[] = [];
    const off = bus.onOutput((id, tail) => seen.push(`${id}:${tail}`));
    bus.emitOutput("a1", "hi");
    off();
    bus.emitOutput("a1", "bye");
    expect(seen).toEqual(["a1:hi"]);
  });

  it("a throwing observer does not stop the others", () => {
    const bus = createHookBus();
    const good = vi.fn();
    bus.onExit(() => {
      throw new Error("boom");
    });
    bus.onExit(good);
    bus.emitExit("a1", 0);
    expect(good).toHaveBeenCalledWith("a1", 0);
  });

  it("routes idle, waiting, and exit to their own observers", () => {
    const bus = createHookBus();
    const idle = vi.fn();
    const waiting = vi.fn();
    const exit = vi.fn();
    bus.onIdle(idle);
    bus.onWaiting(waiting);
    bus.onExit(exit);
    bus.emitIdle("a1");
    bus.emitWaiting("a2");
    bus.emitExit("a3", 1);
    expect(idle).toHaveBeenCalledWith("a1");
    expect(waiting).toHaveBeenCalledWith("a2");
    expect(exit).toHaveBeenCalledWith("a3", 1);
  });
});
