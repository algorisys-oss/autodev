import { describe, it, expect } from "vitest";
import { suggestForDifficulty } from "./difficulty";
import { parseMentions, resolveMentions } from "./mentions";
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
