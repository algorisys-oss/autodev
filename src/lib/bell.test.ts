import { describe, it, expect, vi, afterEach } from "vitest";
import { createHookBus } from "./hooks";
import { installBellHook, ringBell } from "./bell";

function harness(enabled = true) {
  const bus = createHookBus();
  const ring = vi.fn();
  installBellHook(bus, { enabled: () => enabled, ring });
  return { bus, ring };
}

/** Minimal WebAudio stand-in: jsdom has no AudioContext, and the synth path is exactly the part
 *  that would fail silently. Records what was scheduled. */
function fakeAudio() {
  const started: number[] = [];
  const stopped: number[] = [];
  const close = vi.fn().mockResolvedValue(undefined);
  const node = () => ({ connect: vi.fn() });
  class Ctx {
    currentTime = 10;
    destination = {};
    createOscillator() {
      return {
        ...node(),
        type: "",
        frequency: { value: 0 },
        start: (t: number) => started.push(t),
        stop: (t: number) => stopped.push(t),
      };
    }
    createGain() {
      return {
        ...node(),
        gain: {
          setValueAtTime: vi.fn(),
          exponentialRampToValueAtTime: vi.fn(),
        },
      };
    }
    close = close;
  }
  (globalThis as { AudioContext?: unknown }).AudioContext = Ctx;
  return { started, stopped, close };
}

describe("ringBell", () => {
  afterEach(() => {
    delete (globalThis as { AudioContext?: unknown }).AudioContext;
  });

  it("schedules two decaying tones and releases the context", () => {
    const audio = fakeAudio();
    vi.useFakeTimers();
    ringBell();
    expect(audio.started).toEqual([10, 10.11]); // second note struck after the first
    expect(audio.stopped.every((t, i) => t > audio.started[i])).toBe(true);
    vi.runAllTimers();
    expect(audio.close).toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("is a no-op where the platform has no audio context", () => {
    delete (globalThis as { AudioContext?: unknown }).AudioContext;
    expect(() => ringBell()).not.toThrow();
  });
});

describe("installBellHook", () => {
  it("rings when an agent settles into idle", () => {
    const { bus, ring } = harness();
    bus.emitIdle("a1");
    expect(ring).toHaveBeenCalledTimes(1);
  });

  it("stays silent when the setting is off", () => {
    const { bus, ring } = harness(false);
    bus.emitIdle("a1");
    bus.emitExit("a1", 0);
    expect(ring).not.toHaveBeenCalled();
  });

  it("does not ring twice when an idle agent then exits", () => {
    const { bus, ring } = harness();
    bus.emitIdle("a1");
    bus.emitExit("a1", 0);
    expect(ring).toHaveBeenCalledTimes(1);
  });

  it("rings on exit when the agent never went idle first", () => {
    const { bus, ring } = harness();
    bus.emitExit("a1", 0);
    expect(ring).toHaveBeenCalledTimes(1);
  });

  it("rings on exit after a waiting agent is killed", () => {
    const { bus, ring } = harness();
    bus.emitWaiting("a1");
    bus.emitExit("a1", 1);
    expect(ring).toHaveBeenCalledTimes(1);
  });

  it("rings again on the next turn's completion", () => {
    const { bus, ring } = harness();
    bus.emitIdle("a1"); // first turn done
    bus.emitWaiting("a1"); // asks something
    bus.emitIdle("a1"); // second turn done
    expect(ring).toHaveBeenCalledTimes(2);
  });

  it("tracks agents independently", () => {
    const { bus, ring } = harness();
    bus.emitIdle("a1");
    bus.emitExit("a2", 0);
    expect(ring).toHaveBeenCalledTimes(2);
  });

  it("does not let a throwing ring break the hook bus", () => {
    const bus = createHookBus();
    installBellHook(bus, {
      enabled: () => true,
      ring: () => {
        throw new Error("no audio device");
      },
    });
    expect(() => bus.emitIdle("a1")).not.toThrow();
  });
});
