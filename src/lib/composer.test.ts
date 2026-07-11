import { describe, it, expect } from "vitest";
import { suggestForDifficulty } from "./difficulty";
import { parseMentions, resolveMentions } from "./mentions";
import { selectPrompts, withUltrathink, promptsDiffer } from "./agent-prompts";
import type { Project } from "./ipc";

describe("suggestForDifficulty", () => {
  it("trivial = 1 agent, no plan, no ultrathink", () => {
    expect(suggestForDifficulty(1)).toEqual({ agents: 1, planMode: false, ultrathink: false });
  });
  it("mid = plan mode kicks in", () => {
    expect(suggestForDifficulty(5)).toEqual({ agents: 2, planMode: true, ultrathink: false });
  });
  it("hard = many agents + plan + ultrathink", () => {
    expect(suggestForDifficulty(10)).toEqual({ agents: 6, planMode: true, ultrathink: true });
  });
  it("clamps out-of-range input", () => {
    expect(suggestForDifficulty(0).agents).toBe(1);
    expect(suggestForDifficulty(99).agents).toBe(6);
  });
});

describe("parseMentions", () => {
  it("extracts unique tokens in order", () => {
    expect(parseMentions("wire @api into @ui and test @api")).toEqual(["api", "ui"]);
  });
  it("returns empty when none", () => {
    expect(parseMentions("no mentions here")).toEqual([]);
  });
});

describe("resolveMentions", () => {
  const projects: Project[] = [
    { name: "bridge-bench-ui", path: "/p/ui" },
    { name: "bridge-bench-api", path: "/p/api" },
  ];
  it("matches fuzzily and reports unresolved", () => {
    const r = resolveMentions("build @BridgeBenchUI using @nope", projects);
    expect(r.resolved.map((p) => p.path)).toEqual(["/p/ui"]);
    expect(r.unresolved).toEqual(["nope"]);
  });
  it("dedupes resolved projects", () => {
    const r = resolveMentions("@bridge-bench-api @bridgebenchapi", projects);
    expect(r.resolved.map((p) => p.path)).toEqual(["/p/api"]);
  });
});

describe("selectPrompts", () => {
  it("single-prompt mode gives every agent the base", () => {
    expect(selectPrompts("do the thing", [], 3, false)).toEqual([
      "do the thing",
      "do the thing",
      "do the thing",
    ]);
  });
  it("per-agent mode uses distinct overrides", () => {
    expect(selectPrompts("base", ["fix nav", "add route"], 2, true)).toEqual([
      "fix nav",
      "add route",
    ]);
  });
  it("a blank override falls back to the base", () => {
    expect(selectPrompts("base", ["fix nav", "  "], 2, true)).toEqual(["fix nav", "base"]);
  });
  it("overrides shorter than count fall back to the base", () => {
    expect(selectPrompts("base", ["only one"], 3, true)).toEqual(["only one", "base", "base"]);
  });
  it("ignores overrides entirely when per-agent is off", () => {
    expect(selectPrompts("base", ["ignored"], 2, false)).toEqual(["base", "base"]);
  });
});

describe("withUltrathink", () => {
  it("appends the hint for claude", () => {
    expect(withUltrathink("do it", true, "claude")).toBe("do it ultrathink");
  });
  it("uses the bare hint when the prompt is empty", () => {
    expect(withUltrathink("", true, "claude")).toBe("ultrathink");
  });
  it("does nothing when ultrathink is off", () => {
    expect(withUltrathink("do it", false, "claude")).toBe("do it");
  });
  it("does nothing for non-claude backends", () => {
    expect(withUltrathink("do it", true, "codex")).toBe("do it");
    expect(withUltrathink("do it", true, "antigravity")).toBe("do it");
  });
});

describe("promptsDiffer", () => {
  it("is false when all prompts are identical", () => {
    expect(promptsDiffer(["a", "a", "a"])).toBe(false);
  });
  it("is true when any prompt differs", () => {
    expect(promptsDiffer(["a", "b", "a"])).toBe(true);
  });
  it("is false for zero or one prompt", () => {
    expect(promptsDiffer([])).toBe(false);
    expect(promptsDiffer(["a"])).toBe(false);
  });
});
