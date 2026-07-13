import { For, Show, createSignal, createEffect, on } from "solid-js";
import { gitWorktreeStatus, type Workspace } from "../lib/ipc";

/** One footer row: a project folder and, when it is a git repo, its branch + dirty flag. */
type Row = { name: string; path: string; branch: string | null; dirty: boolean };

/** Persistent bottom status bar. For the active workspace it lists each project folder and,
 *  when the folder is a git work tree, the checked-out branch (● marks uncommitted changes). */
export function StatusFooter(props: { workspace: Workspace | null }) {
  const [rows, setRows] = createSignal<Row[]>([]);

  // Refetch whenever the set of project paths changes (workspace switch, +dir, remove).
  createEffect(
    on(
      () => (props.workspace?.projects ?? []).map((p) => p.path).join("\n"),
      async () => {
        const projects = props.workspace?.projects ?? [];
        const next = await Promise.all(
          projects.map(async (p): Promise<Row> => {
            try {
              const s = await gitWorktreeStatus(p.path);
              return { name: p.name, path: p.path, branch: s.branch, dirty: s.dirty };
            } catch {
              // Not a git work tree (or git unavailable) — show the folder without a branch.
              return { name: p.name, path: p.path, branch: null, dirty: false };
            }
          }),
        );
        setRows(next);
      },
    ),
  );

  return (
    <footer class="app-footer">
      <Show
        when={props.workspace}
        fallback={<span class="muted">No workspace selected</span>}
      >
        {(ws) => (
          <>
            <span class="footer-ws">{ws().name}</span>
            <Show
              when={rows().length}
              fallback={<span class="muted">no project directories</span>}
            >
              <For each={rows()}>
                {(r) => (
                  <span class="footer-project" title={r.path}>
                    <span class="footer-name">{r.name}</span>
                    <Show
                      when={r.branch}
                      fallback={<span class="muted footer-nogit">not a git repo</span>}
                    >
                      <span class="footer-branch">
                        <span class="footer-git-icon" aria-hidden="true">
                          ⎇
                        </span>
                        <span class="footer-branch-name">{r.branch}</span>
                        <Show when={r.dirty}>
                          <span class="footer-dirty" title="uncommitted changes">
                            ●
                          </span>
                        </Show>
                      </span>
                    </Show>
                  </span>
                )}
              </For>
            </Show>
          </>
        )}
      </Show>
    </footer>
  );
}
