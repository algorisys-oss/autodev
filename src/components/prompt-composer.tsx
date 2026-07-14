import { createSignal, createMemo, createEffect, on, onMount, For, Show } from "solid-js";
import {
  addPromptHistory,
  getPromptHistory,
  getSettings,
  gitCreateWorktree,
  gitIsRepo,
  recordStart,
  recordStop,
  onTranscribeProgress,
  captureScreen,
  saveShot,
  backendList,
  listTemplates,
  type AgentBackend,
  type BackendInfo,
  type PromptTemplate,
  type Workspace,
  type WorktreeInfo,
  type TaskPlan,
} from "../lib/ipc";
import { expandTemplate, templateMatches } from "../lib/templates";
import { extensionCommands } from "../lib/extensions";
import { setVoiceStatus } from "../lib/status";
import { suggestForDifficulty } from "../lib/difficulty";
import { analyzeTask } from "../lib/task-split";
import { selectPrompts, composeAgentPrompt, promptsDiffer } from "../lib/agent-prompts";
import type { Annotation } from "../lib/annotate";
import { resolveMentions } from "../lib/mentions";
import { Annotator } from "./annotator";
import { BrowserHandoff } from "./browser-handoff";
import type { createAgentStore } from "../lib/agent-store";

/** Compose a prompt, pick a difficulty (which suggests agent count + modes), attach
 *  `@`-mentioned projects as context, and fan the task out to N agents. */
export function PromptComposer(props: {
  workspace: Workspace | null;
  agents: ReturnType<typeof createAgentStore>;
  onOpenSettings?: () => void;
}) {
  const [text, setText] = createSignal("");
  const [backend, setBackend] = createSignal<AgentBackend>("claude");
  const [difficulty, setDifficulty] = createSignal(3);
  const [agentCount, setAgentCount] = createSignal(1);
  const [perAgent, setPerAgent] = createSignal(false);
  const [prompts, setPrompts] = createSignal<string[]>([]);
  const [planMode, setPlanMode] = createSignal(false);
  // Session permission posture — one mode at a time (a select, not overlapping checkboxes, so
  // Bypass and Approvals can't silently unset each other). "ask" is interactive per-action
  // approval (B2, implies Rich); "bypass" skips all checks; "normal" leaves the CLI default.
  const [permMode, setPermMode] = createSignal<"normal" | "ask" | "bypass">("normal");
  const bypass = () => permMode() === "bypass";
  const approvals = () => permMode() === "ask";
  // Rich (structured card) view. Only offered for backends that can emit a structured stream.
  const [rich, setRich] = createSignal(false);
  // Pre-launch tool permission lists (B1), comma/space-separated. Only for capable backends.
  const [allowTools, setAllowTools] = createSignal("");
  const [denyTools, setDenyTools] = createSignal("");
  const [ultrathink, setUltrathink] = createSignal(false);
  const [isolate, setIsolate] = createSignal(false);
  const [runIn, setRunIn] = createSignal<string>("");
  const [history, setHistory] = createSignal<string[]>([]);
  const [error, setError] = createSignal<string | null>(null);
  const [recording, setRecording] = createSignal(false);
  const [transcribing, setTranscribing] = createSignal(false);
  // True when the mic is clicked but no transcribe command is configured — surfaces an
  // actionable "set it up in Settings" notice instead of recording audio that can't be transcribed.
  const [needsTranscribeSetup, setNeedsTranscribeSetup] = createSignal(false);
  const [captured, setCaptured] = createSignal<string | null>(null);
  // Structured annotations (annotated screenshot + text notes). Captured once, dispatched to
  // every agent in a launch — notes as prompt text (any backend), the image where supported.
  const [annotations, setAnnotations] = createSignal<Annotation[]>([]);
  const [showHandoff, setShowHandoff] = createSignal(false);
  // Maximize the task editor into a large full-window overlay (same bound text).
  const [maximized, setMaximized] = createSignal(false);
  const [analyzing, setAnalyzing] = createSignal(false);
  const [plan, setPlan] = createSignal<TaskPlan | null>(null);
  // True once the user sets the agent count by hand — analyze-on-launch then defers to them.
  const [countTouched, setCountTouched] = createSignal(false);
  // Backends offered in the picker: bundled + any disk-registered (`~/.autodev/backends`).
  const [backends, setBackends] = createSignal<BackendInfo[]>([]);
  // Prompt templates (`~/.autodev/templates/*.md`): typing `/name` expands to the body.
  const [templates, setTemplates] = createSignal<PromptTemplate[]>([]);
  // Does the currently-selected backend support the Rich (structured) view?
  const supportsRich = () => backends().find((b) => b.id === backend())?.structured ?? false;
  // ...and pre-launch tool allow/deny lists?
  const supportsToolPerms = () =>
    backends().find((b) => b.id === backend())?.toolPermissions ?? false;
  // ...and interactive per-action approval?
  const supportsApproval = () =>
    backends().find((b) => b.id === backend())?.interactiveApproval ?? false;
  // Split a comma/space-separated tool list into names.
  const parseTools = (s: string) => s.split(/[\s,]+/).map((t) => t.trim()).filter(Boolean);

  onMount(async () => {
    try {
      setHistory(await getPromptHistory());
    } catch {
      /* no history yet */
    }
    try {
      const list = await backendList();
      setBackends(list);
      // If the default backend isn't offered, fall back to the first available one.
      if (list.length && !list.some((b) => b.id === backend())) setBackend(list[0].id);
    } catch {
      /* keep the hardcoded default if the list can't be fetched */
    }
    try {
      setTemplates(await listTemplates());
    } catch {
      /* no templates configured */
    }
  });

  // All slash-commands: disk templates (P4) plus any registered by extensions (P5).
  const allCommands = createMemo(() => [...templates(), ...extensionCommands()]);
  // Templates matching the `/command` being typed (for the suggestion row).
  const templateSuggest = createMemo(() => templateMatches(text(), allCommands()));

  // Tab expands a `/name` slash-command: to the sole matching template while still typing the
  // name, or — once past the name — to its body with any trailing text kept.
  function onComposerKeyDown(e: KeyboardEvent) {
    if (e.key !== "Tab") return;
    const suggest = templateSuggest();
    if (suggest.length >= 1) {
      e.preventDefault();
      setText(suggest[0].body);
      return;
    }
    const expanded = expandTemplate(text(), allCommands());
    if (expanded !== null) {
      e.preventDefault();
      setText(expanded);
    }
  }

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

  // Applying an auto-split plan. Created after the difficulty effect above, and driven by a
  // setPlan() that always follows setDifficulty(), so on any flush this runs last: the concrete
  // unit count wins over the difficulty→agents heuristic. A parallel plan fills per-agent
  // prompts; a non-parallel verdict collapses to a single agent.
  createEffect(
    on(plan, (p) => {
      if (!p) return;
      if (p.parallel && p.units.length > 1) {
        setPerAgent(true);
        setAgentCount(p.units.length);
        setPrompts(p.units.map((u) => u.prompt));
      } else {
        setAgentCount(1);
      }
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
    setNeedsTranscribeSetup(false);
    if (recording()) {
      // Stop: the Rust core finalizes the capture and transcribes it, streaming progress.
      setRecording(false);
      setTranscribing(true);
      setVoiceStatus({ text: "Transcribing…", kind: "working" });
      let unlisten: (() => void) | null = null;
      try {
        // Surface the tool's live stderr in the footer — first-run model download can take a
        // while and otherwise looks like a hang.
        unlisten = await onTranscribeProgress((line) =>
          setVoiceStatus({ text: line, kind: "working" }),
        );
        const transcript = await recordStop();
        if (transcript) setText((t) => (t ? `${t} ${transcript}` : transcript));
      } catch (e) {
        setError(String(e));
      } finally {
        unlisten?.();
        setTranscribing(false);
        setVoiceStatus(null);
      }
    } else {
      // Check config before recording: capturing audio that can't be transcribed just wastes the
      // user's breath and surfaces a cryptic backend error. Guide them to Settings instead.
      const configured = await getSettings()
        .then((s) => !!s.transcribeCommand?.trim())
        .catch(() => false);
      if (!configured) {
        setNeedsTranscribeSetup(true);
        setError("Voice input isn't set up. Add a Transcribe command in Settings to enable the mic.");
        return;
      }
      try {
        await recordStart();
        setRecording(true);
        setVoiceStatus({ text: "Recording… (click the ◼ to transcribe)", kind: "recording" });
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

  async function onAnnotated(pngBase64: string, notes: string[]) {
    setCaptured(null);
    try {
      const path = await saveShot(pngBase64);
      setAnnotations((a) => [...a, { image: path, notes }]);
    } catch (e) {
      setError(String(e));
    }
  }

  // Ask a one-shot classifier whether this task parallelizes, and pre-fill the fan-out. Nothing
  // launches — the user reviews the proposed split (and per-agent prompts) before Launch. Returns
  // true when a plan was applied (so analyze-on-launch can pause for review instead of firing).
  async function autoSplit(): Promise<boolean> {
    setError(null);
    const ws = props.workspace;
    if (!ws || ws.projects.length === 0) {
      setError("Add a project directory to this workspace first.");
      return false;
    }
    if (!text().trim()) {
      setError("Describe the task first, then Auto-split.");
      return false;
    }
    const cwdProject = ws.projects.find((p) => p.name === runIn()) ?? ws.projects[0];
    const addDirs = mentions()
      .resolved.map((p) => p.path)
      .filter((path) => path !== cwdProject.path);
    setAnalyzing(true);
    try {
      const result = await analyzeTask({
        task: text().trim(),
        cwd: cwdProject.path,
        backend: backend(),
        projects: ws.projects.map((p) => p.name),
        addDirs,
      });
      if (!result) {
        setError("Couldn't analyze this task — set the agent count manually.");
        return false;
      }
      // Difficulty first (drives plan/ultrathink via its effect), then the plan (overrides count).
      setDifficulty(result.difficulty);
      setPlan(result);
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setAnalyzing(false);
    }
  }

  async function launch() {
    setError(null);
    const ws = props.workspace;
    if (!ws || ws.projects.length === 0) {
      setError("Add a project directory to this workspace first.");
      return;
    }
    // Rich view is one-shot (`claude -p`): with no prompt the CLI exits immediately with
    // "Input must be provided… when using --print". Block that rather than spawn a dead session.
    if (rich() && supportsRich() && !text().trim()) {
      setError("Rich view runs the agent once on your prompt — type a task before launching.");
      return;
    }

    // Analyze-on-launch (opt-in): the first Launch on a task that isn't already split and whose
    // count wasn't set by hand runs the classifier and pauses for review. A second Launch (now
    // with a plan applied) fans out. Read the setting fresh so a mid-session toggle takes effect.
    if (!plan() && !countTouched() && text().trim()) {
      const on = await getSettings()
        .then((s) => s.autoSplitOnLaunch ?? false)
        .catch(() => false);
      if (on && (await autoSplit())) return;
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
        const prompt = composeAgentPrompt(bases[i].trim(), annotations(), ultrathink(), backend());

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
            // Approval and bypass are mutually exclusive; approval implies a Rich session.
            bypassPermissions: bypass() && !(approvals() && supportsApproval()),
            rich: (rich() || (approvals() && supportsApproval())) && supportsRich(),
            interactiveApproval: approvals() && supportsApproval(),
            allowedTools: supportsToolPerms() ? parseTools(allowTools()) : undefined,
            disallowedTools: supportsToolPerms() ? parseTools(denyTools()) : undefined,
            addDirs,
            images: annotations().map((a) => a.image),
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
      setAnnotations([]);
      setPlan(null);
      setCountTouched(false);
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
          onKeyDown={onComposerKeyDown}
          placeholder="Describe the task to start. @mention a project, or /command to expand a template…"
          rows={3}
        />
        <button
          class="mic"
          classList={{ recording: recording() }}
          title={recording() ? "Stop and transcribe" : "Record voice"}
          onClick={toggleRecord}
          disabled={transcribing()}
        >
          {transcribing() ? "…" : recording() ? "◼" : "🎤"}
        </button>
        <button class="mic" title="Screenshot" onClick={takeScreenshot}>
          📷
        </button>
        <button class="mic" title="Browser handoff" onClick={() => setShowHandoff(true)}>
          🌐
        </button>
        <button class="mic" title="Maximize editor" onClick={() => setMaximized(true)}>
          ⛶
        </button>
      </div>

      <Show when={maximized()}>
        <div
          class="annotator-backdrop"
          onClick={(e) => e.target === e.currentTarget && setMaximized(false)}
        >
          <div class="composer-max">
            <div class="composer-max-head">
              <strong>New task</strong>
              <span class="muted">@mention a project, or /command to expand a template</span>
              <span class="spacer" />
              <button title="Minimize editor" onClick={() => setMaximized(false)}>
                ⤡ Minimize
              </button>
            </div>
            <textarea
              class="composer-max-text"
              value={text()}
              onInput={(e) => setText(e.currentTarget.value)}
              onKeyDown={(e) => {
                onComposerKeyDown(e);
                if (e.key === "Escape") setMaximized(false);
              }}
              placeholder="Describe the task to start…"
              autofocus
            />
          </div>
        </div>
      </Show>

      <Show when={templateSuggest().length}>
        <div class="mention-row">
          <For each={templateSuggest()}>
            {(t) => (
              <button
                class="chip template-suggest"
                title={t.body}
                onClick={() => setText(t.body)}
              >
                /{t.name}
              </button>
            )}
          </For>
          <span class="muted">Tab or click to expand</span>
        </div>
      </Show>

      <Show when={showHandoff()}>
        <BrowserHandoff onClose={() => setShowHandoff(false)} />
      </Show>


      <Show when={annotations().length}>
        <div class="mention-row">
          <For each={annotations()}>
            {(a, i) => (
              <span
                class="chip ok"
                title={a.notes.length ? `${a.image}\n${a.notes.join("\n")}` : a.image}
              >
                📎 shot{a.notes.length ? ` +${a.notes.length} note${a.notes.length > 1 ? "s" : ""}` : ""}{" "}
                <button
                  class="chip-x"
                  onClick={() => setAnnotations((arr) => arr.filter((_, j) => j !== i()))}
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
            <For each={backends()}>{(b) => <option value={b.id}>{b.label}</option>}</For>
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
            onInput={(e) => {
              setCountTouched(true);
              setAgentCount(Number(e.currentTarget.value));
            }}
          />
        </label>

        <button
          class="auto-split"
          title="Analyze the task and split it across independent agents"
          onClick={autoSplit}
          disabled={analyzing() || !text().trim()}
        >
          {analyzing() ? "Analyzing…" : "✨ Auto-split"}
        </button>
      </div>

      <Show when={plan()}>
        {(p) => (
          <div class="split-plan" classList={{ parallel: p().parallel }}>
            <span class="split-plan-head">
              {p().parallel
                ? `Split into ${p().units.length} independent units — review the per-agent prompts below, then Launch.`
                : `Best as a single agent (difficulty ${p().difficulty}).`}
            </span>
            <Show when={p().rationale}>
              <span class="muted">{p().rationale}</span>
            </Show>
            <button class="chip-x" title="Dismiss" onClick={() => setPlan(null)}>
              ×
            </button>
          </div>
        )}
      </Show>

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
        <label class="perm-mode" title="How tool calls are permitted this session">
          Permissions
          <select
            value={permMode()}
            onChange={(e) => {
              const m = e.currentTarget.value as "normal" | "ask" | "bypass";
              setPermMode(m);
              if (m === "ask") setRich(true); // interactive approval needs the card view
            }}
          >
            <option value="normal">Normal</option>
            <Show when={supportsApproval()}>
              <option value="ask">Ask each tool</option>
            </Show>
            <option value="bypass">Bypass (skip checks)</option>
          </select>
        </label>
        <Show when={supportsRich()}>
          <label title="Render this session as structured cards instead of a raw terminal (one-shot)"><input type="checkbox" checked={rich() || (approvals() && supportsApproval())} disabled={approvals() && supportsApproval()} onChange={(e) => setRich(e.currentTarget.checked)} /> Rich view</label>
        </Show>
        <label><input type="checkbox" checked={ultrathink()} onChange={(e) => setUltrathink(e.currentTarget.checked)} /> Ultrathink</label>
        <label><input type="checkbox" checked={isolate()} onChange={(e) => setIsolate(e.currentTarget.checked)} /> Isolate (worktree)</label>
        <label><input type="checkbox" checked={perAgent()} onChange={(e) => setPerAgent(e.currentTarget.checked)} /> Per-agent prompts</label>
        <span class="spacer" />
        <button class="primary" onClick={launch} disabled={analyzing()}>
          {analyzing() ? "Analyzing…" : `Launch ${agentCount()} agent${agentCount() > 1 ? "s" : ""}`}
        </button>
      </div>

      <Show when={supportsToolPerms()}>
        <details class="composer-tools">
          <summary>
            Tool permissions
            <Show when={allowTools().trim() || denyTools().trim()}>
              <span class="tools-badge">
                {[
                  parseTools(allowTools()).length && `allow ${parseTools(allowTools()).length}`,
                  parseTools(denyTools()).length && `block ${parseTools(denyTools()).length}`,
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
            </Show>
          </summary>
          <div class="composer-tools-body">
            <label>
              Auto-allow
              <input
                type="text"
                placeholder="e.g. Read, Grep, Glob (blank = backend default)"
                value={allowTools()}
                onInput={(e) => setAllowTools(e.currentTarget.value)}
              />
            </label>
            <label>
              Block
              <input
                type="text"
                placeholder="e.g. Bash, Write, Edit"
                value={denyTools()}
                onInput={(e) => setDenyTools(e.currentTarget.value)}
              />
            </label>
            <p class="muted tools-hint">
              Names are the agent's own tool names. Blocked tools are removed from the session;
              auto-allowed tools run without a prompt. Applies to Rich and terminal launches.
            </p>
          </div>
        </details>
      </Show>

      <Show when={perAgent() && promptsDiffer(selected()) && !isolate()}>
        <p class="warn">Agents share one working directory — enable Isolate to avoid collisions.</p>
      </Show>

      <Show when={error()}>
        {(e) => (
          <p class="error">
            {e()}
            <Show when={needsTranscribeSetup() && props.onOpenSettings}>
              {" "}
              <button class="link-btn" onClick={() => props.onOpenSettings?.()}>
                Open Settings
              </button>
            </Show>
          </p>
        )}
      </Show>

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
