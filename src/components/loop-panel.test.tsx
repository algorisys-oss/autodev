import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@solidjs/testing-library";
import { createAgentStore, type Subscribe } from "../lib/agent-store";
import type * as ipcTypes from "../lib/ipc";
import { LoopPanel } from "./loop-panel";

// Mock the whole ipc surface the panel touches; each test sets return values per case.
vi.mock("../lib/ipc", () => ({
  loopList: vi.fn(),
  loopCreate: vi.fn(),
  loopCurrentPrompt: vi.fn(),
  loopApplyDecomposer: vi.fn(),
  loopApplyPlanner: vi.fn(),
  loopApplyEvaluator: vi.fn(),
  loopReadyToEvaluate: vi.fn(),
  loopSetFeatures: vi.fn(),
  loopSetContract: vi.fn(),
  loopGrade: vi.fn(),
}));
import * as ipc from "../lib/ipc";

const mocked = ipc as unknown as Record<keyof typeof ipcTypes, ReturnType<typeof vi.fn>>;

function loop(over: Partial<ipcTypes.LoopState> = {}): ipcTypes.LoopState {
  return {
    id: "l1",
    spec: "build a thing",
    projectDir: "/proj",
    phase: "planning",
    iteration: 0,
    maxIterations: 5,
    contract: [],
    features: [],
    progress: "",
    ...over,
  };
}

/** A real agent store wired to a controllable fake ipc + event bus. */
function agentHarness() {
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
  } as unknown as typeof ipcTypes;
  const store = createAgentStore({ api, subscribe, now: () => 1000 });
  return { store, emit };
}

describe("loop panel auto-advance", () => {
  beforeEach(() => {
    for (const fn of Object.values(mocked)) fn.mockReset();
  });

  it("applies the planner's contract when its agent exits, advancing to generating", async () => {
    mocked.loopList.mockResolvedValue([loop()]);
    mocked.loopCurrentPrompt.mockResolvedValue({ role: "planner", prompt: "PLAN" });
    const generating = loop({
      phase: "generating",
      contract: [{ text: "rejects a bad email", met: null }],
    });
    mocked.loopApplyPlanner.mockResolvedValue(generating);

    const { store, emit } = agentHarness();
    const { getByText, findByText } = render(() => (
      <LoopPanel agents={store} defaultProjectDir="/proj" />
    ));

    await findByText(/Run planner/);
    fireEvent.click(getByText(/Run planner/));
    await waitFor(() => expect(store.state.agents).toHaveLength(1));

    // The planner agent finishes.
    emit("agent://exit", { id: "agent-1", code: 0 });

    await waitFor(() => expect(mocked.loopApplyPlanner).toHaveBeenCalledWith("l1", "agent-1"));
    await findByText(/rejects a bad email/);
    expect(mocked.loopReadyToEvaluate).not.toHaveBeenCalled();
  });

  it("grades via the evaluator when its agent exits", async () => {
    mocked.loopList.mockResolvedValue([
      loop({ phase: "evaluating", contract: [{ text: "has tests", met: null }] }),
    ]);
    mocked.loopCurrentPrompt.mockResolvedValue({ role: "evaluator", prompt: "EVAL" });
    mocked.loopApplyEvaluator.mockResolvedValue(
      loop({ phase: "passed", contract: [{ text: "has tests", met: true }] }),
    );

    const { store, emit } = agentHarness();
    const { getByText, findByText } = render(() => (
      <LoopPanel agents={store} defaultProjectDir="/proj" />
    ));

    await findByText(/Run evaluator/);
    fireEvent.click(getByText(/Run evaluator/));
    await waitFor(() => expect(store.state.agents).toHaveLength(1));

    emit("agent://exit", { id: "agent-1", code: 0 });

    await waitFor(() => expect(mocked.loopApplyEvaluator).toHaveBeenCalledWith("l1", "agent-1"));
    await findByText(/Contract met/);
  });

  it("with auto-run on, chains straight into the next role after an advance", async () => {
    mocked.loopList.mockResolvedValue([loop()]);
    mocked.loopCurrentPrompt
      .mockResolvedValueOnce({ role: "planner", prompt: "PLAN" })
      .mockResolvedValueOnce({ role: "generator", prompt: "GEN" });
    mocked.loopApplyPlanner.mockResolvedValue(
      loop({ phase: "generating", contract: [{ text: "c1", met: null }] }),
    );

    const { store, emit } = agentHarness();
    const { getByText, getByLabelText, findByText } = render(() => (
      <LoopPanel agents={store} defaultProjectDir="/proj" />
    ));

    await findByText(/Run planner/);
    fireEvent.click(getByLabelText("Auto-run"));
    fireEvent.click(getByText(/Run planner/));
    await waitFor(() => expect(store.state.agents).toHaveLength(1));

    // Planner exits → contract applied → generator spawned automatically (no click).
    emit("agent://exit", { id: "agent-1", code: 0 });

    await waitFor(() => expect(store.state.agents).toHaveLength(2));
    expect(mocked.loopCurrentPrompt).toHaveBeenCalledTimes(2);
    expect(mocked.loopReadyToEvaluate).not.toHaveBeenCalled();
  });

  it("surfaces a parse failure and leaves the phase for manual entry", async () => {
    mocked.loopList.mockResolvedValue([loop()]);
    mocked.loopCurrentPrompt.mockResolvedValue({ role: "planner", prompt: "PLAN" });
    mocked.loopApplyPlanner.mockRejectedValue("no contract criteria found");

    const { store, emit } = agentHarness();
    const { getByText, findByText } = render(() => (
      <LoopPanel agents={store} defaultProjectDir="/proj" />
    ));

    await findByText(/Run planner/);
    fireEvent.click(getByText(/Run planner/));
    await waitFor(() => expect(store.state.agents).toHaveLength(1));

    emit("agent://exit", { id: "agent-1", code: 0 });

    await findByText(/Auto-advance failed/);
    // Still planning: the manual "Set contract" fallback is present.
    expect(getByText(/Set contract/)).toBeTruthy();
  });

  it("passes the verify command and max rounds when creating a loop", async () => {
    mocked.loopList.mockResolvedValue([]);
    mocked.loopCreate.mockResolvedValue(loop({ verifyCommand: "npm test", maxIterations: 12 }));

    const { store } = agentHarness();
    const { getByText, getByPlaceholderText, container } = render(() => (
      <LoopPanel agents={store} defaultProjectDir="/proj" />
    ));

    fireEvent.input(getByPlaceholderText(/Spec for a new autonomous loop/), {
      target: { value: "build a URL shortener" },
    });
    fireEvent.input(getByPlaceholderText(/dev\.sh verify/), { target: { value: "npm test" } });
    fireEvent.input(container.querySelector('input[type="number"]')!, {
      target: { value: "12" },
    });
    fireEvent.click(getByText("New loop"));

    await waitFor(() =>
      expect(mocked.loopCreate).toHaveBeenCalledWith("build a URL shortener", "/proj", "npm test", 12),
    );
  });

  it("applies the decomposer's backlog when its agent exits, advancing to planning", async () => {
    mocked.loopList.mockResolvedValue([loop({ phase: "decomposing" })]);
    mocked.loopCurrentPrompt.mockResolvedValue({ role: "decomposer", prompt: "DECOMPOSE" });
    mocked.loopApplyPlanner.mockClear();
    mocked.loopApplyDecomposer.mockResolvedValue(
      loop({
        phase: "planning",
        currentFeature: 0,
        features: [
          { title: "user auth", done: false },
          { title: "create posts", done: false },
        ],
      }),
    );

    const { store, emit } = agentHarness();
    const { getByText, findByText } = render(() => (
      <LoopPanel agents={store} defaultProjectDir="/proj" />
    ));

    await findByText(/Run decomposer/);
    fireEvent.click(getByText(/Run decomposer/));
    await waitFor(() => expect(store.state.agents).toHaveLength(1));

    emit("agent://exit", { id: "agent-1", code: 0 });

    await waitFor(() => expect(mocked.loopApplyDecomposer).toHaveBeenCalledWith("l1", "agent-1"));
    // The backlog renders with both features, and the loop is now planning the first.
    await findByText(/user auth/);
    await findByText(/create posts/);
    expect(mocked.loopApplyPlanner).not.toHaveBeenCalled();
  });

  it("shows the recorded failure reason on a failed loop", async () => {
    mocked.loopList.mockResolvedValue([
      loop({
        phase: "failed",
        failureReason: "no progress in 3 rounds (1/4 criteria met; tests failing)",
        contract: [{ text: "c1", met: false }],
      }),
    ]);

    const { store } = agentHarness();
    const { findByText } = render(() => <LoopPanel agents={store} defaultProjectDir="/proj" />);

    await findByText(/no progress in 3 rounds/);
  });
});
