import { createSignal, onMount, onCleanup, Show, For } from "solid-js";
import { appInfo, type AppInfo, type AgentBackend, type Project } from "./lib/ipc";
import { createWorkspaceStore } from "./lib/workspace-store";
import { createAgentStore } from "./lib/agent-store";
import { WorkspaceSidebar } from "./components/workspace-sidebar";
import { AgentGrid } from "./components/agent-grid";
import { TerminalPane } from "./components/terminal-pane";
import "./App.css";

function App() {
  const [info, setInfo] = createSignal<AppInfo | null>(null);
  const [launchError, setLaunchError] = createSignal<string | null>(null);
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

  async function launch(project: Project, backend: AgentBackend) {
    setLaunchError(null);
    try {
      await agents.spawn({ backend, cwd: project.path }, project.name);
    } catch (e) {
      setLaunchError(String(e));
    }
  }

  return (
    <div class="app">
      <header class="app-header">
        <h1>AutoDev</h1>
        <Show when={info()} fallback={<span class="muted">connecting…</span>}>
          {(i) => <span class="muted">v{i().version}</span>}
        </Show>
      </header>

      <div class="app-body">
        <WorkspaceSidebar store={workspaces} />

        <main class="main-panel">
          <Show
            when={selected()}
            fallback={<p class="muted">Create a workspace and add project directories to begin.</p>}
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
                          <div class="project-line">
                            <span class="project-name">{p.name}</span>
                            <code class="project-path">{p.path}</code>
                            <button class="launch" onClick={() => launch(p, "claude")}>
                              ▶ Claude
                            </button>
                            <button class="launch" onClick={() => launch(p, "codex")}>
                              ▶ Codex
                            </button>
                          </div>
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>
                <Show when={launchError()}>{(e) => <p class="error">{e()}</p>}</Show>
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
                  <Show when={a.status !== "exited"}>
                    <button onClick={() => agents.kill(a.id)}>Kill</button>
                  </Show>
                </div>
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
