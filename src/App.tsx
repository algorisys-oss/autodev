import { createSignal, onMount, onCleanup, Show, For } from "solid-js";
import { appInfo, gitMergeWorktree, gitRemoveWorktree, type AppInfo } from "./lib/ipc";
import { createWorkspaceStore } from "./lib/workspace-store";
import { createAgentStore, isTerminal } from "./lib/agent-store";
import { WorkspaceSidebar } from "./components/workspace-sidebar";
import { AgentGrid } from "./components/agent-grid";
import { TerminalPane } from "./components/terminal-pane";
import { PromptComposer } from "./components/prompt-composer";
import { LoopPanel } from "./components/loop-panel";
import { SettingsPanel } from "./components/settings-panel";
import "./App.css";

function App() {
  const [info, setInfo] = createSignal<AppInfo | null>(null);
  const [view, setView] = createSignal<"workspace" | "loops">("workspace");
  const [showSettings, setShowSettings] = createSignal(false);
  const workspaces = createWorkspaceStore();
  const agents = createAgentStore();

  onMount(async () => {
    try {
      setInfo(await appInfo());
    } catch {
      /* header shows "connecting…" if the core is unreachable */
    }
    await workspaces.refresh();
    agents.start();
  });
  onCleanup(() => agents.dispose());

  const selected = () => workspaces.selected();
  const [wtMsg, setWtMsg] = createSignal<string | null>(null);

  async function mergeWorktree(repo: string, branch: string) {
    setWtMsg("merging…");
    try {
      await gitMergeWorktree(repo, branch);
      setWtMsg(`merged ${branch}`);
    } catch (e) {
      setWtMsg(String(e));
    }
  }

  async function removeWorktree(repo: string, path: string) {
    setWtMsg("removing…");
    try {
      await gitRemoveWorktree(repo, path, false);
      setWtMsg("worktree removed");
    } catch (e) {
      setWtMsg(String(e));
    }
  }

  return (
    <div class="app">
      <header class="app-header">
        <h1>AutoDev</h1>
        <Show when={info()} fallback={<span class="muted">connecting…</span>}>
          {(i) => <span class="muted">v{i().version}</span>}
        </Show>
        <span class="spacer" />
        <nav class="view-tabs">
          <button classList={{ active: view() === "workspace" }} onClick={() => setView("workspace")}>
            Workspace
          </button>
          <button classList={{ active: view() === "loops" }} onClick={() => setView("loops")}>
            Loops
          </button>
          <button class="icon settings-btn" title="Settings" onClick={() => setShowSettings(true)}>
            ⚙
          </button>
        </nav>
      </header>

      <Show when={showSettings()}>
        <SettingsPanel onClose={() => setShowSettings(false)} />
      </Show>

      <div class="app-body">
        <WorkspaceSidebar store={workspaces} />

        <main class="main-panel">
          <Show when={view() === "loops"}>
            <LoopPanel agents={agents} defaultProjectDir={selected()?.projects[0]?.path ?? null} />
          </Show>

          <Show when={view() === "workspace" && selected()}
            fallback={
              <Show when={view() === "workspace"}>
                <p class="muted">Create a workspace and add project directories to begin.</p>
              </Show>
            }
          >
            {(ws) => (
              <div class="workspace-detail">
                <h2>{ws().name}</h2>
                <Show
                  when={ws().projects.length}
                  fallback={<p class="muted">No projects yet. Use “+dir” in the sidebar.</p>}
                >
                  <ul class="detail-projects">
                    <For each={ws().projects}>
                      {(p) => (
                        <li>
                          <span class="project-name">{p.name}</span>
                          <code class="project-path">{p.path}</code>
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>

                <PromptComposer workspace={ws()} agents={agents} />
              </div>
            )}
          </Show>

          <AgentGrid store={agents} />

          <Show when={agents.focused()} keyed>
            {(a) => (
              <section class="agent-session">
                <div class="agent-bar">
                  <span class="agent-title">
                    {a.label} · <span class="muted">{a.id} · {a.backend}</span>
                  </span>
                  <span class="spacer" />
                  <Show when={!isTerminal(a.status)}>
                    <button onClick={() => agents.kill(a.id)}>Kill</button>
                  </Show>
                </div>
                <Show when={a.worktree}>
                  {(wt) => (
                    <div class="worktree-bar">
                      <span class="muted">worktree · {wt().branch}</span>
                      <span class="spacer" />
                      <button onClick={() => mergeWorktree(wt().repo, wt().branch)}>Merge</button>
                      <button onClick={() => removeWorktree(wt().repo, wt().path)}>Remove</button>
                      <Show when={wtMsg()}>{(m) => <span class="wt-msg">{m()}</span>}</Show>
                    </div>
                  )}
                </Show>
                <TerminalPane agentId={a.id} store={agents} />
              </section>
            )}
          </Show>
        </main>
      </div>
    </div>
  );
}

export default App;
