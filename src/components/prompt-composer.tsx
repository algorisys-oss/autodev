import { createSignal, createMemo, createEffect, on, onMount, For, Show } from "solid-js";
import {
  addPromptHistory,
  getPromptHistory,
  gitCreateWorktree,
  gitIsRepo,
  transcribeAudio,
  captureScreen,
  saveShot,
  type AgentBackend,
  type Workspace,
  type WorktreeInfo,
} from "../lib/ipc";
import { startRecording, extFromMime, type Recorder } from "../lib/recorder";
import { suggestForDifficulty } from "../lib/difficulty";
import { selectPrompts, withUltrathink, promptsDiffer } from "../lib/agent-prompts";
import { resolveMentions } from "../lib/mentions";
import { Annotator } from "./annotator";
import { BrowserHandoff } from "./browser-handoff";
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
  const [perAgent, setPerAgent] = createSignal(false);
  const [prompts, setPrompts] = createSignal<string[]>([]);
  const [planMode, setPlanMode] = createSignal(false);
  const [bypass, setBypass] = createSignal(false);
  const [ultrathink, setUltrathink] = createSignal(false);
  const [isolate, setIsolate] = createSignal(false);
  const [runIn, setRunIn] = createSignal<string>("");
  const [history, setHistory] = createSignal<string[]>([]);
  const [error, setError] = createSignal<string | null>(null);
  const [recorder, setRecorder] = createSignal<Recorder | null>(null);
  const [transcribing, setTranscribing] = createSignal(false);
  const [captured, setCaptured] = createSignal<string | null>(null);
  const [images, setImages] = createSignal<string[]>([]);
  const [showHandoff, setShowHandoff] = createSignal(false);

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

  // Set one agent's prompt override, growing the sparse array as needed. Trailing entries
  // beyond the current count are kept so they survive lowering then raising the count.
  const setPromptAt = (i: number, val: string) =>
    setPrompts((ps) => {
      const next = [...ps];
      next[i] = val;
      return next;
    });

  // Turning on per-agent prompts defaults Isolate on: divergent tasks in one working
  // directory would collide. The user can still turn it back off.
  createEffect(
    on(perAgent, (v) => {
      if (v) setIsolate(true);
    }),
  );

  // The effective (pre-suffix) prompt each agent will run, driving the collision hint below.
  const selected = createMemo(() =>
    selectPrompts(text(), prompts(), Math.max(1, agentCount()), perAgent()),
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

  async function toggleRecord() {
    setError(null);
    const rec = recorder();
    if (rec) {
      setRecorder(null);
      setTranscribing(true);
      try {
        const blob = await rec.stop();
        const bytes = new Uint8Array(await blob.arrayBuffer());
        const transcript = await transcribeAudio(bytes, extFromMime(blob.type));
        if (transcript) setText((t) => (t ? `${t} ${transcript}` : transcript));
      } catch (e) {
        setError(String(e));
      } finally {
        setTranscribing(false);
      }
    } else {
      try {
        setRecorder(await startRecording());
      } catch (e) {
        setError(`microphone unavailable: ${e}`);
      }
    }
  }

  async function takeScreenshot() {
    setError(null);
    try {
      setCaptured(await captureScreen());
    } catch (e) {
      setError(String(e));
    }
  }

  async function onAnnotated(pngBase64: string) {
    setCaptured(null);
    try {
      const path = await saveShot(pngBase64);
      setImages((imgs) => [...imgs, path]);
    } catch (e) {
      setError(String(e));
    }
  }

  async function launch() {
    setError(null);
    const ws = props.workspace;
    if (!ws || ws.projects.length === 0) {
      setError("Add a project directory to this workspace first.");
      return;
    }
    const cwdProject = ws.projects.find((p) => p.name === runIn()) ?? ws.projects[0];

    const n = Math.max(1, agentCount());
    const bases = selectPrompts(text(), prompts(), n, perAgent());
    const useWorktree = isolate() && (await gitIsRepo(cwdProject.path).catch(() => false));
    try {
      for (let i = 0; i < n; i++) {
        // Each agent's context dirs come from its own prompt's @mentions.
        const addDirs = resolveMentions(bases[i], ws.projects)
          .resolved.map((p) => p.path)
          .filter((path) => path !== cwdProject.path);
        const prompt = withUltrathink(bases[i].trim(), ultrathink(), backend());

        let cwd = cwdProject.path;
        let worktree: WorktreeInfo | undefined;
        if (useWorktree) {
          const slug = cwdProject.name.replace(/[^a-zA-Z0-9]+/g, "-");
          const branch = `autodev/${slug}-${Date.now().toString(36)}-${i}`;
          worktree = await gitCreateWorktree(cwdProject.path, branch);
          cwd = worktree.path;
        }
        await props.agents.spawn(
          {
            backend: backend(),
            cwd,
            planMode: planMode(),
            bypassPermissions: bypass(),
            addDirs,
            images: images(),
            initialPrompt: prompt || null,
          },
          n > 1 ? `${cwdProject.name} #${i + 1}` : cwdProject.name,
          worktree,
        );
      }
      // Record each distinct non-empty prompt in history.
      const seen = new Set<string>();
      for (const b of bases) {
        const raw = b.trim();
        if (raw && !seen.has(raw)) {
          seen.add(raw);
          setHistory(await addPromptHistory(raw));
        }
      }
      setText("");
      setPrompts([]);
      setImages([]);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <section class="composer">
      <div class="composer-head">
        <span class="composer-heading">New task</span>
        <span class="muted">
          Launches a fresh agent for each. To continue an agent that's already running, type in
          its terminal below — not here.
        </span>
      </div>
      <div class="composer-input">
        <textarea
          class="composer-text"
          value={text()}
          onInput={(e) => setText(e.currentTarget.value)}
          placeholder="Describe the task to start. @mention a project to add it as context…"
          rows={3}
        />
        <button
          class="mic"
          classList={{ recording: !!recorder() }}
          title={recorder() ? "Stop and transcribe" : "Record voice"}
          onClick={toggleRecord}
          disabled={transcribing()}
        >
          {transcribing() ? "…" : recorder() ? "◼" : "🎤"}
        </button>
        <button class="mic" title="Screenshot" onClick={takeScreenshot}>
          📷
        </button>
        <button class="mic" title="Browser handoff" onClick={() => setShowHandoff(true)}>
          🌐
        </button>
      </div>

      <Show when={showHandoff()}>
        <BrowserHandoff onClose={() => setShowHandoff(false)} />
      </Show>


      <Show when={images().length}>
        <div class="mention-row">
          <For each={images()}>
            {(path, i) => (
              <span class="chip ok" title={path}>
                📎 shot{" "}
                <button
                  class="chip-x"
                  onClick={() => setImages((imgs) => imgs.filter((_, j) => j !== i()))}
                >
                  ×
                </button>
              </span>
            )}
          </For>
        </div>
      </Show>

      <Show when={captured()}>
        {(img) => (
          <Annotator imageBase64={img()} onAttach={onAnnotated} onCancel={() => setCaptured(null)} />
        )}
      </Show>

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
            <option value="antigravity">Antigravity</option>
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

      <Show when={perAgent() && agentCount() > 1}>
        <div class="per-agent-prompts">
          <For each={Array.from({ length: agentCount() })}>
            {(_, i) => (
              <label class="per-agent-prompt">
                <span class="muted">#{i() + 1}</span>
                <textarea
                  class="composer-text"
                  rows={2}
                  value={prompts()[i()] ?? ""}
                  onInput={(e) => setPromptAt(i(), e.currentTarget.value)}
                  placeholder="same as shared prompt"
                />
              </label>
            )}
          </For>
        </div>
      </Show>

      <div class="composer-toggles">
        <label><input type="checkbox" checked={planMode()} onChange={(e) => setPlanMode(e.currentTarget.checked)} /> Plan mode</label>
        <label><input type="checkbox" checked={bypass()} onChange={(e) => setBypass(e.currentTarget.checked)} /> Bypass permissions</label>
        <label><input type="checkbox" checked={ultrathink()} onChange={(e) => setUltrathink(e.currentTarget.checked)} /> Ultrathink</label>
        <label><input type="checkbox" checked={isolate()} onChange={(e) => setIsolate(e.currentTarget.checked)} /> Isolate (worktree)</label>
        <label><input type="checkbox" checked={perAgent()} onChange={(e) => setPerAgent(e.currentTarget.checked)} /> Per-agent prompts</label>
        <span class="spacer" />
        <button class="primary" onClick={launch}>
          Launch {agentCount()} agent{agentCount() > 1 ? "s" : ""}
        </button>
      </div>

      <Show when={perAgent() && promptsDiffer(selected()) && !isolate()}>
        <p class="warn">Agents share one working directory — enable Isolate to avoid collisions.</p>
      </Show>

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
