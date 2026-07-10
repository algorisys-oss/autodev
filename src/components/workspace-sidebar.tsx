import { createSignal, For, Show } from "solid-js";
import { open } from "@tauri-apps/plugin-dialog";
import type { createWorkspaceStore } from "../lib/workspace-store";

/** Left sidebar: list workspaces, create them, and manage their project dirs. */
export function WorkspaceSidebar(props: {
  store: ReturnType<typeof createWorkspaceStore>;
}) {
  const { store } = props;
  const [newName, setNewName] = createSignal("");

  async function pickAndAddProject(workspaceId: string) {
    const picked = await open({ directory: true, multiple: false, title: "Add project directory" });
    if (typeof picked === "string") {
      await store.addProject(workspaceId, picked);
    }
  }

  async function openFolderAsWorkspace() {
    const picked = await open({ directory: true, multiple: false, title: "Open folder as workspace" });
    if (typeof picked === "string") {
      await store.createFromFolder(picked);
    }
  }

  async function submitNew(e: Event) {
    e.preventDefault();
    await store.create(newName());
    setNewName("");
  }

  return (
    <aside class="sidebar">
      <form class="new-workspace" onSubmit={submitNew}>
        <input
          value={newName()}
          onInput={(e) => setNewName(e.currentTarget.value)}
          placeholder="New workspace name…"
          aria-label="New workspace name"
        />
        <button type="submit">Add</button>
      </form>

      <button class="open-folder" onClick={openFolderAsWorkspace}>
        Open folder as workspace…
      </button>

      <Show when={store.state.error}>
        {(e) => <p class="error">{e()}</p>}
      </Show>

      <ul class="workspace-list">
        <For each={store.state.workspaces} fallback={<li class="muted">No workspaces yet.</li>}>
          {(ws) => (
            <li classList={{ selected: ws.id === store.state.selectedId }}>
              <div class="workspace-row">
                <button class="workspace-name" onClick={() => store.select(ws.id)}>
                  {ws.name}
                </button>
                <span class="spacer" />
                <button class="icon" title="Add project" onClick={() => pickAndAddProject(ws.id)}>
                  +dir
                </button>
                <button class="icon" title="Delete workspace" onClick={() => store.remove(ws.id)}>
                  ×
                </button>
              </div>
              <Show when={ws.id === store.state.selectedId && ws.projects.length}>
                <ul class="project-list">
                  <For each={ws.projects}>
                    {(p) => (
                      <li class="project-row" title={p.path}>
                        <span class="project-name">{p.name}</span>
                        <button
                          class="icon"
                          title="Remove project"
                          onClick={() => store.removeProject(ws.id, p.name)}
                        >
                          ×
                        </button>
                      </li>
                    )}
                  </For>
                </ul>
              </Show>
            </li>
          )}
        </For>
      </ul>
    </aside>
  );
}
