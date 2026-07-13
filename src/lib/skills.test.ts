import { describe, it, expect } from "vitest";
import { withSkillsDir, installSkillsHook } from "./skills";
import { createHookBus } from "./hooks";
import type { AgentOptions } from "./ipc";

const opts = (over: Partial<AgentOptions> = {}): AgentOptions => ({
  backend: "claude",
  cwd: "/p",
  ...over,
});

describe("withSkillsDir", () => {
  it("appends the skills dir to addDirs", () => {
    expect(withSkillsDir(opts(), "/skills").addDirs).toEqual(["/skills"]);
    expect(withSkillsDir(opts({ addDirs: ["/a"] }), "/skills").addDirs).toEqual(["/a", "/skills"]);
  });

  it("does not add a duplicate", () => {
    const o = opts({ addDirs: ["/skills"] });
    expect(withSkillsDir(o, "/skills")).toBe(o);
  });
});

describe("installSkillsHook", () => {
  it("registers a spawn hook that injects the skills dir when one exists", async () => {
    const bus = createHookBus();
    await installSkillsHook(bus, { getSkillsDir: async () => "/home/u/.autodev/skills" });
    expect(bus.applySpawn(opts()).addDirs).toEqual(["/home/u/.autodev/skills"]);
  });

  it("registers nothing when there is no skills dir", async () => {
    const bus = createHookBus();
    await installSkillsHook(bus, { getSkillsDir: async () => null });
    expect(bus.applySpawn(opts()).addDirs).toBeUndefined();
  });
});
