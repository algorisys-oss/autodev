import { createSignal } from "solid-js";
import { listExtensions as fetchExtensions, type ExtensionFile, type PromptTemplate } from "./ipc";
import type { HookBus } from "./hooks";

/** Executable extensions (P5).
 *
 *  Each `~/.autodev/extensions/*.js` file is run as an ES module; its default export is called
 *  with an `autodev` API through which it registers lifecycle hooks (the P3 bus) and composer
 *  slash-commands. Extensions run with the app's full trust — they are the user's own files, so
 *  they are surfaced (a status list), not sandboxed. Extensions must be self-contained single
 *  files: bare `import` of other modules won't resolve.
 */

/** Composer commands contributed by extensions — reactive, so the composer merges them live. */
const [extensionCommands, setExtensionCommands] = createSignal<PromptTemplate[]>([]);
export { extensionCommands };

/** What each extension did when loaded, for surfacing in the UI. */
export interface ExtensionStatus {
  name: string;
  ok: boolean;
  error?: string;
}
const [loadedExtensions, setLoadedExtensions] = createSignal<ExtensionStatus[]>([]);
export { loadedExtensions };

/** The API handed to an extension's default export. */
export interface AutoDevApi {
  /** The agent-lifecycle hook bus (onSpawn/onOutput/onIdle/onWaiting/onExit). */
  hooks: HookBus;
  /** Register a composer slash-command that expands to `body` (leading `/` optional). */
  registerCommand: (name: string, body: string) => void;
  /** The running app version. */
  version: string;
}

/** Clear registered commands and statuses. For tests and re-loads. */
export function resetExtensionState(): void {
  setExtensionCommands([]);
  setLoadedExtensions([]);
}

/** Run one extension module's source, calling its default export with the api. A blob module
 *  URL is used so `export default` works. Separated out so tests can inject a fake evaluator
 *  instead of exercising the browser's module loader. */
async function evaluateModule(source: string, api: AutoDevApi): Promise<void> {
  const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
  try {
    const mod = await import(/* @vite-ignore */ url);
    if (typeof mod.default !== "function") {
      throw new Error("extension must `export default` a function");
    }
    await mod.default(api);
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** Load and run every extension, wiring each to `hooks`. Records a status per extension and
 *  returns them. A throwing extension is isolated — it fails alone; the rest still load. */
export async function loadExtensions(
  hooks: HookBus,
  version: string,
  deps: {
    list?: () => Promise<ExtensionFile[]>;
    evaluate?: (source: string, api: AutoDevApi) => Promise<void>;
  } = {},
): Promise<ExtensionStatus[]> {
  const list = deps.list ?? fetchExtensions;
  const evaluate = deps.evaluate ?? evaluateModule;

  let files: ExtensionFile[] = [];
  try {
    files = await list();
  } catch {
    files = [];
  }

  const statuses: ExtensionStatus[] = [];
  for (const file of files) {
    const api: AutoDevApi = {
      hooks,
      version,
      registerCommand: (name, body) => {
        const clean = name.replace(/^\//, "").trim();
        if (!clean) return;
        setExtensionCommands((cmds) => [...cmds.filter((c) => c.name !== clean), { name: clean, body }]);
      },
    };
    try {
      await evaluate(file.source, api);
      statuses.push({ name: file.name, ok: true });
    } catch (e) {
      statuses.push({ name: file.name, ok: false, error: String(e) });
    }
  }
  setLoadedExtensions(statuses);
  return statuses;
}
