import { createSignal, onMount, Show, For } from "solid-js";
import { appInfo, agentSpawn, agentKill, type AppInfo, type Project } from "./lib/ipc";
import { createWorkspaceStore } from "./lib/workspace-store";
import { WorkspaceSidebar } from "./components/workspace-sidebar";
import { TerminalPane } from "./components/terminal-pane";
import "./App.css";

interface ActiveAgent {
  id: string;
  label: string;
  running: boolean;
}

function App() {
  const [info, setInfo] = createSignal<AppInfo | null>(null);
  const [agent, setAgent] = createSignal<ActiveAgent | null>(null);
  const [launchError, setLaunchError] = createSignal<string | null>(null);
  const store = createWorkspaceStore();

  onMount(async () => {
    try {
      setInfo(await appInfo());
    } catch {
      /* header shows "connecting…" if the core is unreachable */
    }
    await store.refresh();
  });

  const selected = () => store.selected();

  async function launch(project: Project) {
    setLaunchError(null);
    try {
      const id = await agentSpawn({ backend: "claude", cwd: project.path });
      setAgent({ id, label: project.name, running: true });
    } catch (e) {
      setLaunchError(String(e));
    }
  }

  async function kill() {
    const a = agent();
    if (!a) return;
    try {
      await agentKill(a.id);
    } catch {
      /* already gone */
    }
    setAgent(null);
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
        <WorkspaceSidebar store={store} />

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
                            <button class="launch" onClick={() => launch(p)}>
                              ▶ Claude
                            </button>
                          </div>
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>

                <Show when={launchError()}>{(e) => <p class="error">{e()}</p>}</Show>

                <Show when={agent()}>
                  {(a) => (
                    <section class="agent-session">
                      <div class="agent-bar">
                        <span class="agent-title">
                          {a().label} · <span class="muted">{a().id}</span>
                        </span>
                        <span class="spacer" />
                        <button onClick={kill}>Kill</button>
                      </div>
                      <TerminalPane
                        agentId={a().id}
                        onExit={() => setAgent({ ...a(), running: false })}
                      />
                    </section>
                  )}
                </Show>
              </div>
            )}
          </Show>
        </main>
      </div>
    </div>
  );
}

export default App;
