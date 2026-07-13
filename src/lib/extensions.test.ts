import { describe, it, expect, beforeEach } from "vitest";
import {
  loadExtensions,
  extensionCommands,
  loadedExtensions,
  resetExtensionState,
  type AutoDevApi,
} from "./extensions";
import { createHookBus } from "./hooks";
import type { ExtensionFile, AgentOptions } from "./ipc";

const file = (name: string, source = ""): ExtensionFile => ({ name, source });
const opts = (): AgentOptions => ({ backend: "claude", cwd: "/p" });

describe("loadExtensions", () => {
  beforeEach(() => resetExtensionState());

  it("runs each extension against the api: hooks and commands register", async () => {
    const hooks = createHookBus();
    // Fake evaluator: interpret the "source" as a directive so we exercise the real api.
    const evaluate = async (source: string, api: AutoDevApi) => {
      if (source === "skills") api.hooks.onSpawn((o) => ({ ...o, addDirs: ["/skills"] }));
      if (source === "cmd") api.registerCommand("/standup", "Summarize standup");
    };
    const statuses = await loadExtensions(hooks, "9.9", {
      list: async () => [file("a", "skills"), file("b", "cmd")],
      evaluate,
    });

    expect(statuses).toEqual([
      { name: "a", ok: true },
      { name: "b", ok: true },
    ]);
    // The spawn hook the extension registered actually rewrites options.
    expect(hooks.applySpawn(opts()).addDirs).toEqual(["/skills"]);
    // The command it registered is exposed to the composer (leading slash stripped).
    expect(extensionCommands().map((c) => c.name)).toContain("standup");
  });

  it("isolates a failing extension: it is marked failed, others still load", async () => {
    const hooks = createHookBus();
    const evaluate = async (source: string, api: AutoDevApi) => {
      if (source === "boom") throw new Error("kaboom");
      if (source === "ok") api.registerCommand("good", "body");
    };
    const statuses = await loadExtensions(hooks, "1", {
      list: async () => [file("bad", "boom"), file("good", "ok")],
      evaluate,
    });

    expect(statuses[0].ok).toBe(false);
    expect(statuses[0].error).toContain("kaboom");
    expect(statuses[1].ok).toBe(true);
    expect(loadedExtensions()).toHaveLength(2);
    expect(extensionCommands().map((c) => c.name)).toContain("good");
  });

  it("passes the version and records nothing when there are no extensions", async () => {
    const hooks = createHookBus();
    let seenVersion = "";
    await loadExtensions(hooks, "3.1", {
      list: async () => [file("v", "ver")],
      evaluate: async (_s, api) => {
        seenVersion = api.version;
      },
    });
    expect(seenVersion).toBe("3.1");

    const none = await loadExtensions(hooks, "3.1", { list: async () => [] });
    expect(none).toEqual([]);
    expect(loadedExtensions()).toEqual([]);
  });
});
