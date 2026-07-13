import type { AgentOptions } from "./ipc";
import { skillsDir as fetchSkillsDir } from "./ipc";
import type { HookBus } from "./hooks";

/** Add `skillsPath` to an agent's context directories, unless it's already there. */
export function withSkillsDir(options: AgentOptions, skillsPath: string): AgentOptions {
  const dirs = options.addDirs ?? [];
  if (dirs.includes(skillsPath)) return options;
  return { ...options, addDirs: [...dirs, skillsPath] };
}

/** If a skills directory exists, register a spawn hook that adds it to every agent's context
 *  (P4). This is a real consumer of the P3 hook bus: skills reach agents through the same
 *  `--add-dir` seam as `@`-mentions, on every backend, with no per-launch wiring. */
export async function installSkillsHook(
  bus: HookBus,
  deps: { getSkillsDir?: () => Promise<string | null> } = {},
): Promise<void> {
  const getSkillsDir = deps.getSkillsDir ?? fetchSkillsDir;
  const path = await getSkillsDir();
  if (path) bus.onSpawn((o) => withSkillsDir(o, path));
}
