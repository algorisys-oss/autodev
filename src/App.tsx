import { createSignal, onMount, Show, For } from "solid-js";
import { appInfo, type AppInfo } from "./lib/ipc";
import { createWorkspaceStore } from "./lib/workspace-store";
import { WorkspaceSidebar } from "./components/workspace-sidebar";
import "./App.css";

function App() {
  const [info, setInfo] = createSignal<AppInfo | null>(null);
  const store = createWorkspaceStore();

  onMount(async () => {
    try {
      setInfo(await appInfo());
    } catch {
      /* header just shows "connecting…" if the core is unreachable */
    }
    await store.refresh();
  });

  const selected = () => store.selected();

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
                          <span class="project-name">{p.name}</span>
                          <code class="project-path">{p.path}</code>
                        </li>
                      )}
                    </For>
                  </ul>
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
