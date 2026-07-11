import type { AgentBackend } from "./ipc";

/** Choose the effective (pre-suffix) prompt text for each of `count` agents. In per-agent
 *  mode a non-blank override wins; a blank override — or single-prompt mode — falls back to
 *  the shared base. Always returns exactly `count` strings. */
export function selectPrompts(
  base: string,
  overrides: string[],
  count: number,
  perAgent: boolean,
): string[] {
  return Array.from({ length: count }, (_, i) => {
    if (!perAgent) return base;
    const override = overrides[i];
    return override && override.trim() ? override : base;
  });
}

/** Append the "ultrathink" hint for Claude; other backends return the prompt unchanged. */
export function withUltrathink(
  prompt: string,
  ultrathink: boolean,
  backend: AgentBackend,
): string {
  if (!ultrathink || backend !== "claude") return prompt;
  return prompt ? `${prompt} ultrathink` : "ultrathink";
}

/** True when the selected prompts are not all identical — used to warn/auto-isolate. */
export function promptsDiffer(selected: string[]): boolean {
  return selected.some((p) => p !== selected[0]);
}
