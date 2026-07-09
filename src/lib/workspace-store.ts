import { createStore } from "solid-js/store";
import * as ipc from "./ipc";
import type { Workspace } from "./ipc";

export interface WorkspaceState {
  workspaces: Workspace[];
  selectedId: string | null;
  error: string | null;
}

/** Reactive store for workspaces and the currently selected one. All disk work
 *  goes through the Rust core via `ipc`; this just holds UI state and orchestrates. */
export function createWorkspaceStore(api: typeof ipc = ipc) {
  const [state, setState] = createStore<WorkspaceState>({
    workspaces: [],
    selectedId: null,
    error: null,
  });

  const selected = () => state.workspaces.find((w) => w.id === state.selectedId) ?? null;

  function fail(e: unknown) {
    setState("error", String(e));
  }

  /** Replace one workspace in the list by id (after an add/remove-project op). */
  function upsert(ws: Workspace) {
    const idx = state.workspaces.findIndex((w) => w.id === ws.id);
    if (idx >= 0) setState("workspaces", idx, ws);
    else setState("workspaces", state.workspaces.length, ws);
  }

  async function refresh() {
    try {
      const list = await api.listWorkspaces();
      setState("workspaces", list);
      if ((!state.selectedId || !list.some((w) => w.id === state.selectedId)) && list.length) {
        setState("selectedId", list[0].id);
      }
      setState("error", null);
    } catch (e) {
      fail(e);
    }
  }

  async function create(name: string) {
    if (!name.trim()) return;
    try {
      const ws = await api.createWorkspace(name);
      upsert(ws);
      setState("selectedId", ws.id);
      setState("error", null);
    } catch (e) {
      fail(e);
    }
  }

  async function remove(id: string) {
    try {
      await api.deleteWorkspace(id);
      setState(
        "workspaces",
        state.workspaces.filter((w) => w.id !== id),
      );
      if (state.selectedId === id) {
        setState("selectedId", state.workspaces[0]?.id ?? null);
      }
      setState("error", null);
    } catch (e) {
      fail(e);
    }
  }

  async function addProject(workspaceId: string, path: string) {
    try {
      upsert(await api.addProject(workspaceId, path));
      setState("error", null);
    } catch (e) {
      fail(e);
    }
  }

  async function removeProject(workspaceId: string, projectName: string) {
    try {
      upsert(await api.removeProject(workspaceId, projectName));
      setState("error", null);
    } catch (e) {
      fail(e);
    }
  }

  function select(id: string) {
    setState("selectedId", id);
  }

  return {
    state,
    selected,
    refresh,
    create,
    remove,
    addProject,
    removeProject,
    select,
  };
}
