import { createSignal, createMemo, createEffect, on, onMount, For, Show } from "solid-js";
import {
  addPromptHistory,
  getPromptHistory,
  type AgentBackend,
  type Workspace,
} from "../lib/ipc";
import { suggestForDifficulty } from "../lib/difficulty";
import { resolveMentions } from "../lib/mentions";
import type { createAgentStore } from "../lib/agent-store";

/** Compose a prompt, pick a difficulty (which suggests agent count + modes), attach
 *  `@`-mentioned projects as context, and fan the task out to N agents. */
export function PromptComposer(props: {
  workspace: Workspace | null;
  agents: ReturnType<typeof createAgentStore>;
}) {
  const [text, setText] = createSignal("");
  const [backend, setBackend] = createSignal<AgentBackend>("claude");
  const [difficulty, setDifficulty] = createSignal(3);
  const [agentCount, setAgentCount] = createSignal(1);
  const [planMode, setPlanMode] = createSignal(false);
  const [bypass, setBypass] = createSignal(false);
  const [ultrathink, setUltrathink] = createSignal(false);
  const [runIn, setRunIn] = createSignal<string>("");
  const [history, setHistory] = createSignal<string[]>([]);
  const [error, setError] = createSignal<string | null>(null);

  onMount(async () => {
    try {
      setHistory(await getPromptHistory());
    } catch {
      /* no history yet */
    }
  });

  // Moving the difficulty slider re-applies the suggested count and modes.
  createEffect(
    on(difficulty, (d) => {
      const s = suggestForDifficulty(d);
      setAgentCount(s.agents);
      setPlanMode(s.planMode);
      setUltrathink(s.ultrathink);
    }),
  );

  // Default the working directory to the workspace's first project.
  createEffect(
    on(
      () => props.workspace?.id,
      () => setRunIn(props.workspace?.projects[0]?.name ?? ""),
    ),
  );

  const mentions = createMemo(() =>
    resolveMentions(text(), props.workspace?.projects ?? []),
  );

  async function launch() {
    setError(null);
    const ws = props.workspace;
    if (!ws || ws.projects.length === 0) {
      setError("Add a project directory to this workspace first.");
      return;
    }
    const cwdProject = ws.projects.find((p) => p.name === runIn()) ?? ws.projects[0];
    const addDirs = mentions()
      .resolved.map((p) => p.path)
      .filter((path) => path !== cwdProject.path);

    const raw = text().trim();
    let prompt = raw;
    if (ultrathink() && backend() === "claude") prompt = prompt ? `${prompt} ultrathink` : "ultrathink";

    const n = Math.max(1, agentCount());
    try {
      for (let i = 0; i < n; i++) {
        await props.agents.spawn(
          {
            backend: backend(),
            cwd: cwdProject.path,
            planMode: planMode(),
            bypassPermissions: bypass(),
            addDirs,
            initialPrompt: prompt || null,
          },
          n > 1 ? `${cwdProject.name} #${i + 1}` : cwdProject.name,
        );
      }
      if (raw) setHistory(await addPromptHistory(raw));
      setText("");
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <section class="composer">
      <textarea
        class="composer-text"
        value={text()}
        onInput={(e) => setText(e.currentTarget.value)}
        placeholder="Describe the task. @mention a project to add it as context…"
        rows={3}
      />

      <Show when={mentions().resolved.length || mentions().unresolved.length}>
        <div class="mention-row">
          <For each={mentions().resolved}>
            {(p) => <span class="chip ok" title={p.path}>@{p.name}</span>}
          </For>
          <For each={mentions().unresolved}>
            {(t) => <span class="chip bad" title="no matching project">@{t}?</span>}
          </For>
        </div>
      </Show>

      <div class="composer-controls">
        <label class="control">
          Backend
          <select value={backend()} onChange={(e) => setBackend(e.currentTarget.value as AgentBackend)}>
            <option value="claude">Claude</option>
            <option value="codex">Codex</option>
          </select>
        </label>

        <label class="control">
          Run in
          <select value={runIn()} onChange={(e) => setRunIn(e.currentTarget.value)}>
            <For each={props.workspace?.projects ?? []}>
              {(p) => <option value={p.name}>{p.name}</option>}
            </For>
          </select>
        </label>

        <label class="control grow">
          Difficulty {difficulty()}
          <input
            type="range"
            min="1"
            max="10"
            value={difficulty()}
            onInput={(e) => setDifficulty(Number(e.currentTarget.value))}
          />
        </label>

        <label class="control">
          Agents
          <input
            type="number"
            min="1"
            max="16"
            value={agentCount()}
            onInput={(e) => setAgentCount(Number(e.currentTarget.value))}
          />
        </label>
      </div>

      <div class="composer-toggles">
        <label><input type="checkbox" checked={planMode()} onChange={(e) => setPlanMode(e.currentTarget.checked)} /> Plan mode</label>
        <label><input type="checkbox" checked={bypass()} onChange={(e) => setBypass(e.currentTarget.checked)} /> Bypass permissions</label>
        <label><input type="checkbox" checked={ultrathink()} onChange={(e) => setUltrathink(e.currentTarget.checked)} /> Ultrathink</label>
        <span class="spacer" />
        <button class="primary" onClick={launch}>
          Launch {agentCount()} agent{agentCount() > 1 ? "s" : ""}
        </button>
      </div>

      <Show when={error()}>{(e) => <p class="error">{e()}</p>}</Show>

      <Show when={history().length}>
        <details class="history">
          <summary class="muted">Prompt history ({history().length})</summary>
          <ul>
            <For each={history()}>
              {(h) => (
                <li>
                  <button class="history-item" onClick={() => setText(h)}>
                    {h.length > 90 ? h.slice(0, 90) + "…" : h}
                  </button>
                </li>
              )}
            </For>
          </ul>
        </details>
      </Show>
    </section>
  );
}
