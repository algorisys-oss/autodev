import { describe, it, expect, vi } from "vitest";
import { createRoot } from "solid-js";
import { createWorkspaceStore } from "./workspace-store";
import type * as ipc from "./ipc";
import type { Workspace } from "./ipc";

function ws(id: string, projects: Workspace["projects"] = []): Workspace {
  return { id, name: id, projects };
}

/** A fake ipc module backed by an in-memory list. */
function fakeApi(initial: Workspace[] = []) {
  let list = [...initial];
  return {
    listWorkspaces: vi.fn(async () => list),
    createWorkspace: vi.fn(async (name: string) => {
      const w = ws(name);
      list = [...list, w];
      return w;
    }),
    deleteWorkspace: vi.fn(async (id: string) => {
      list = list.filter((w) => w.id !== id);
    }),
    addProject: vi.fn(async (id: string, path: string) => {
      const w = { ...list.find((x) => x.id === id)! };
      w.projects = [...w.projects, { name: path.split("/").pop()!, path }];
      list = list.map((x) => (x.id === id ? w : x));
      return w;
    }),
    removeProject: vi.fn(async (id: string, name: string) => {
      const w = { ...list.find((x) => x.id === id)! };
      w.projects = w.projects.filter((p) => p.name !== name);
      list = list.map((x) => (x.id === id ? w : x));
      return w;
    }),
  } as unknown as typeof ipc;
}

describe("workspace store", () => {
  it("refresh loads workspaces and auto-selects the first", async () => {
    await createRoot(async (dispose) => {
      const store = createWorkspaceStore(fakeApi([ws("a"), ws("b")]));
      await store.refresh();
      expect(store.state.workspaces.map((w) => w.id)).toEqual(["a", "b"]);
      expect(store.state.selectedId).toBe("a");
      dispose();
    });
  });

  it("create adds a workspace and selects it", async () => {
    await createRoot(async (dispose) => {
      const store = createWorkspaceStore(fakeApi());
      await store.create("api");
      expect(store.state.selectedId).toBe("api");
      expect(store.selected()?.id).toBe("api");
      dispose();
    });
  });

  it("addProject then removeProject updates the selected workspace", async () => {
    await createRoot(async (dispose) => {
      const store = createWorkspaceStore(fakeApi([ws("work")]));
      await store.refresh();
      await store.addProject("work", "/home/me/my-api");
      expect(store.selected()?.projects.map((p) => p.name)).toEqual(["my-api"]);
      await store.removeProject("work", "my-api");
      expect(store.selected()?.projects).toEqual([]);
      dispose();
    });
  });

  it("createFromFolder makes a workspace named after the folder and adds it as a project", async () => {
    await createRoot(async (dispose) => {
      const store = createWorkspaceStore(fakeApi());
      await store.createFromFolder("/home/me/lab/my-app");
      expect(store.selected()?.name).toBe("my-app");
      expect(store.selected()?.projects.map((p) => p.path)).toEqual(["/home/me/lab/my-app"]);
      expect(store.state.selectedId).toBe("my-app");
      dispose();
    });
  });

  it("createFromFolder handles a trailing slash and Windows separators", async () => {
    await createRoot(async (dispose) => {
      const store = createWorkspaceStore(fakeApi());
      await store.createFromFolder("/home/me/lab/api/");
      expect(store.selected()?.name).toBe("api");
      dispose();
    });
  });

  it("remove deletes and reselects", async () => {
    await createRoot(async (dispose) => {
      const store = createWorkspaceStore(fakeApi([ws("a"), ws("b")]));
      await store.refresh();
      await store.remove("a");
      expect(store.state.workspaces.map((w) => w.id)).toEqual(["b"]);
      expect(store.state.selectedId).toBe("b");
      dispose();
    });
  });

  it("captures errors from the core", async () => {
    await createRoot(async (dispose) => {
      const api = fakeApi();
      (api.createWorkspace as ReturnType<typeof vi.fn>).mockRejectedValueOnce("boom");
      const store = createWorkspaceStore(api);
      await store.create("x");
      expect(store.state.error).toContain("boom");
      dispose();
    });
  });
});
