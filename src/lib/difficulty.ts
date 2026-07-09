export interface DifficultySuggestion {
  /** How many agents to fan the task out to. */
  agents: number;
  /** Whether to launch in plan mode. */
  planMode: boolean;
  /** Whether to add the "ultrathink" hint (Claude). */
  ultrathink: boolean;
}

// Agents per difficulty level 1..10, from the workflow in the reference video:
// trivial edits = 1 agent, hard multi-part tasks = several sub-agents.
const AGENTS_BY_LEVEL = [1, 1, 1, 2, 2, 3, 3, 4, 5, 6];

/** Map a 1–10 difficulty to a suggested agent count and modes. Values below 1 or above
 *  10 are clamped. This is a suggestion; the user can override each field. */
export function suggestForDifficulty(difficulty: number): DifficultySuggestion {
  const level = Math.max(1, Math.min(10, Math.round(difficulty)));
  return {
    agents: AGENTS_BY_LEVEL[level - 1],
    planMode: level >= 5,
    ultrathink: level >= 8,
  };
}
