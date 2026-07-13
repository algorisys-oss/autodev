import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@solidjs/testing-library";
import { createAgentStore } from "../lib/agent-store";
import type * as ipcTypes from "../lib/ipc";
import { PromptComposer } from "./prompt-composer";

// The composer reaches ipc directly for history + git + settings checks; stub just those.
const getSettingsMock = vi.fn(() => Promise.resolve({ autoSplitOnLaunch: false }));
vi.mock("../lib/ipc", () => ({
  getPromptHistory: vi.fn(() => Promise.resolve([])),
  addPromptHistory: vi.fn(() => Promise.resolve([])),
  getSettings: () => getSettingsMock(),
  gitIsRepo: vi.fn(() => Promise.resolve(false)),
  gitCreateWorktree: vi.fn(),
  transcribeAudio: vi.fn(),
  captureScreen: vi.fn(),
  saveShot: vi.fn(),
  backendList: vi.fn(() => Promise.resolve([{ id: "claude", label: "Claude", models: [] }])),
  listTemplates: vi.fn(() =>
    Promise.resolve([{ name: "refactor", body: "Refactor this for clarity." }]),
  ),
}));

// Auto-split calls the classifier controller; stub it so the composer's apply logic is what's
// under test (not the real agent round-trip).
const analyzeMock = vi.fn();
vi.mock("../lib/task-split", () => ({ analyzeTask: (...a: unknown[]) => analyzeMock(...a) }));

const workspace: ipcTypes.Workspace = {
  id: "ws1",
  name: "demo",
  projects: [
    { name: "web", path: "/p/web" },
    { name: "api", path: "/p/api" },
  ],
};

// A store whose spawn records the options it was handed, without touching Tauri.
function storeWithRecorder() {
  const spawned: ipcTypes.AgentOptions[] = [];
  let n = 0;
  const api = {
    agentSpawn: (options: ipcTypes.AgentOptions) => {
      spawned.push(options);
      return Promise.resolve(`agent-${n++}`);
    },
  } as unknown as typeof import("../lib/ipc");
  const agents = createAgentStore({ api, subscribe: () => Promise.resolve(() => {}) });
  return { agents, spawned };
}

function labelInput(container: HTMLElement, text: string): HTMLInputElement {
  const label = [...container.querySelectorAll(".composer-toggles label")].find((l) =>
    l.textContent?.includes(text),
  );
  return label!.querySelector("input") as HTMLInputElement;
}

describe("PromptComposer per-agent prompts", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows no per-agent boxes until enabled with count > 1", async () => {
    const { agents } = storeWithRecorder();
    const { container, getByText } = render(() => <PromptComposer workspace={workspace} agents={agents} />);
    // The composer is labelled as a launcher so it reads distinctly from an agent's own prompt.
    expect(getByText("New task")).toBeTruthy();
    expect(container.querySelectorAll('textarea[placeholder="same as shared prompt"]')).toHaveLength(0);
  });

  it("reveals one box per agent and auto-enables Isolate when turned on", async () => {
    const { agents } = storeWithRecorder();
    const { container } = render(() => <PromptComposer workspace={workspace} agents={agents} />);

    const agentsInput = container.querySelector('input[type="number"]') as HTMLInputElement;
    fireEvent.input(agentsInput, { target: { value: "2" } });
    fireEvent.change(labelInput(container, "Per-agent prompts"), { target: { checked: true } });

    await waitFor(() =>
      expect(
        container.querySelectorAll('textarea[placeholder="same as shared prompt"]'),
      ).toHaveLength(2),
    );
    expect(labelInput(container, "Isolate").checked).toBe(true);
  });

  it("fans out distinct prompts, blank override inheriting the shared prompt", async () => {
    const { agents, spawned } = storeWithRecorder();
    const { container, getByPlaceholderText, getByText } = render(() => (
      <PromptComposer workspace={workspace} agents={agents} />
    ));

    fireEvent.input(container.querySelector('input[type="number"]') as HTMLInputElement, {
      target: { value: "2" },
    });
    fireEvent.change(labelInput(container, "Per-agent prompts"), { target: { checked: true } });
    fireEvent.input(getByPlaceholderText("Describe the task to start. @mention a project, or /command to expand a template…"), {
      target: { value: "shared task" },
    });

    const boxes = () =>
      [...container.querySelectorAll('textarea[placeholder="same as shared prompt"]')] as HTMLTextAreaElement[];
    await waitFor(() => expect(boxes()).toHaveLength(2));
    // Leave box #1 blank (inherits shared), give box #2 its own task.
    fireEvent.input(boxes()[1], { target: { value: "special task" } });

    fireEvent.click(getByText("Launch 2 agents"));

    await waitFor(() => expect(spawned).toHaveLength(2));
    expect(spawned[0].initialPrompt).toBe("shared task");
    expect(spawned[1].initialPrompt).toBe("special task");
  });
});

describe("PromptComposer auto-split", () => {
  beforeEach(() => vi.clearAllMocks());

  const typeTask = (getByPlaceholderText: (t: string) => HTMLElement) => {
    fireEvent.input(
      getByPlaceholderText("Describe the task to start. @mention a project, or /command to expand a template…"),
      { target: { value: "transcode every video in ./media" } },
    );
  };

  it("a parallel plan fills per-agent prompts, and units win over the difficulty heuristic", async () => {
    // difficulty 9 alone suggests 5 agents; the plan's 3 units must override that.
    analyzeMock.mockResolvedValue({
      difficulty: 9,
      parallel: true,
      units: [
        { title: "a", prompt: "do a", mentions: [] },
        { title: "b", prompt: "do b", mentions: [] },
        { title: "c", prompt: "do c", mentions: [] },
      ],
      rationale: "three independent files",
    });
    const { agents } = storeWithRecorder();
    const { container, getByPlaceholderText, getByText } = render(() => (
      <PromptComposer workspace={workspace} agents={agents} />
    ));
    typeTask(getByPlaceholderText);
    fireEvent.click(getByText("✨ Auto-split"));

    const boxes = () =>
      [...container.querySelectorAll('textarea[placeholder="same as shared prompt"]')] as HTMLTextAreaElement[];
    await waitFor(() => expect(boxes()).toHaveLength(3));
    expect(boxes().map((b) => b.value)).toEqual(["do a", "do b", "do c"]);
    expect((container.querySelector('input[type="number"]') as HTMLInputElement).value).toBe("3");
    expect(labelInput(container, "Isolate").checked).toBe(true);
    expect(getByText(/Split into 3 independent units/)).toBeTruthy();
  });

  it("analyze-on-launch: first Launch analyzes and pauses; second Launch fans out", async () => {
    getSettingsMock.mockResolvedValueOnce({ autoSplitOnLaunch: true });
    analyzeMock.mockResolvedValue({
      difficulty: 5,
      parallel: true,
      units: [
        { title: "a", prompt: "do a", mentions: [] },
        { title: "b", prompt: "do b", mentions: [] },
      ],
      rationale: "two independent files",
    });
    const { agents, spawned } = storeWithRecorder();
    const { getByPlaceholderText, getByText } = render(() => (
      <PromptComposer workspace={workspace} agents={agents} />
    ));
    typeTask(getByPlaceholderText);

    // First Launch: the setting is on and nothing is split yet, so it analyzes and does NOT spawn.
    fireEvent.click(getByText(/Launch \d agent/));
    await waitFor(() => expect(analyzeMock).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(getByText(/Split into 2 independent units/)).toBeTruthy());
    expect(spawned).toHaveLength(0);

    // Second Launch: a plan is applied, so it fans out for real (no re-analyze).
    fireEvent.click(getByText("Launch 2 agents"));
    await waitFor(() => expect(spawned).toHaveLength(2));
    expect(analyzeMock).toHaveBeenCalledTimes(1);
    expect(spawned.map((s) => s.initialPrompt)).toEqual(["do a", "do b"]);
  });

  it("does not analyze on launch when the agent count was set by hand", async () => {
    getSettingsMock.mockResolvedValue({ autoSplitOnLaunch: true });
    const { agents, spawned } = storeWithRecorder();
    const { container, getByPlaceholderText, getByText } = render(() => (
      <PromptComposer workspace={workspace} agents={agents} />
    ));
    typeTask(getByPlaceholderText);
    fireEvent.input(container.querySelector('input[type="number"]') as HTMLInputElement, {
      target: { value: "2" },
    });
    fireEvent.click(getByText("Launch 2 agents"));
    await waitFor(() => expect(spawned).toHaveLength(2));
    expect(analyzeMock).not.toHaveBeenCalled();
  });

  it("a non-parallel plan collapses to a single agent", async () => {
    analyzeMock.mockResolvedValue({
      difficulty: 8,
      parallel: false,
      units: [{ title: "x", prompt: "the whole task", mentions: [] }],
      rationale: "one cohesive change",
    });
    const { agents } = storeWithRecorder();
    const { container, getByPlaceholderText, getByText } = render(() => (
      <PromptComposer workspace={workspace} agents={agents} />
    ));
    typeTask(getByPlaceholderText);
    fireEvent.click(getByText("✨ Auto-split"));

    await waitFor(() =>
      expect((container.querySelector('input[type="number"]') as HTMLInputElement).value).toBe("1"),
    );
    expect(getByText(/Best as a single agent/)).toBeTruthy();
  });
});

describe("PromptComposer templates", () => {
  beforeEach(() => vi.clearAllMocks());

  it("expands a /template to its body when the suggestion is clicked", async () => {
    const { agents } = storeWithRecorder();
    const { container, findByText } = render(() => (
      <PromptComposer workspace={workspace} agents={agents} />
    ));
    const textarea = () => container.querySelector(".composer-text") as HTMLTextAreaElement;
    fireEvent.input(textarea(), { target: { value: "/ref" } });
    // Once templates load, the matching suggestion appears; clicking it expands to the body.
    fireEvent.click(await findByText("/refactor"));
    await waitFor(() => expect(textarea().value).toBe("Refactor this for clarity."));
  });
});
