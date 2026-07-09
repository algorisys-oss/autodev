import type { Project } from "./ipc";

/** Extract unique `@token` mentions from prompt text, in order of first appearance. */
export function parseMentions(text: string): string[] {
  const out: string[] = [];
  for (const m of text.matchAll(/@([A-Za-z0-9._-]+)/g)) {
    if (!out.includes(m[1])) out.push(m[1]);
  }
  return out;
}

/** Normalize so "Bridge Bench UI", "bridge-bench-ui", "bridgebenchui" all compare equal
 *  (matches the Rust resolver in workspace.rs). */
export function normalize(token: string): string {
  return token.toLowerCase().replace(/[^a-z0-9]/g, "");
}

export interface MentionResolution {
  /** Mentioned projects that matched, with their absolute paths. */
  resolved: Project[];
  /** Mention tokens that did not match any project. */
  unresolved: string[];
}

/** Resolve the `@`-mentions in `text` against a workspace's projects. */
export function resolveMentions(text: string, projects: Project[]): MentionResolution {
  const resolved: Project[] = [];
  const unresolved: string[] = [];
  for (const token of parseMentions(text)) {
    const match = projects.find((p) => normalize(p.name) === normalize(token));
    if (match) {
      if (!resolved.some((p) => p.path === match.path)) resolved.push(match);
    } else {
      unresolved.push(token);
    }
  }
  return { resolved, unresolved };
}
