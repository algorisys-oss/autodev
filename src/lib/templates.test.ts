import { describe, it, expect } from "vitest";
import { expandTemplate, templateMatches } from "./templates";
import type { PromptTemplate } from "./ipc";

const tpls: PromptTemplate[] = [
  { name: "refactor", body: "Refactor this for clarity." },
  { name: "review", body: "Review this diff." },
];

describe("expandTemplate", () => {
  it("expands a bare slash-command to its body", () => {
    expect(expandTemplate("/refactor", tpls)).toBe("Refactor this for clarity.");
  });

  it("keeps text typed after the command", () => {
    expect(expandTemplate("/refactor the parser", tpls)).toBe(
      "Refactor this for clarity. the parser",
    );
  });

  it("returns null for an unknown command or non-command text", () => {
    expect(expandTemplate("/nope", tpls)).toBeNull();
    expect(expandTemplate("just a task", tpls)).toBeNull();
    expect(expandTemplate("not /refactor here", tpls)).toBeNull();
  });
});

describe("templateMatches", () => {
  it("prefix-matches while typing the command name", () => {
    expect(templateMatches("/re", tpls).map((t) => t.name)).toEqual(["refactor", "review"]);
    expect(templateMatches("/ref", tpls).map((t) => t.name)).toEqual(["refactor"]);
    expect(templateMatches("/", tpls)).toHaveLength(2);
  });

  it("stops suggesting once past the command name or when not a command", () => {
    expect(templateMatches("/refactor x", tpls)).toEqual([]);
    expect(templateMatches("hello", tpls)).toEqual([]);
  });
});
