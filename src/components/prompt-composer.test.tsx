import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, waitFor } from "@solidjs/testing-library";
import { createAgentStore } from "../lib/agent-store";
import type * as ipcTypes from "../lib/ipc";
import { PromptComposer } from "./prompt-composer";

// The composer reaches ipc directly for history + git checks; stub just those.
vi.mock("../lib/ipc", () => ({
  getPromptHistory: vi.fn(() => Promise.resolve([])),
  addPromptHistory: vi.fn(() => Promise.resolve([])),
  gitIsRepo: vi.fn(() => Promise.resolve(false)),
  gitCreateWorktree: vi.fn(),
  transcribeAudio: vi.fn(),
  captureScreen: vi.fn(),
  saveShot: vi.fn(),
}));

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
    const { container } = render(() => <PromptComposer workspace={workspace} agents={agents} />);
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
    fireEvent.input(getByPlaceholderText("Describe the task. @mention a project to add it as context…"), {
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
