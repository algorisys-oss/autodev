import { openExternal } from "../lib/ipc";
import logoUrl from "../assets/logo.svg";

/** About AutoDev — a small modal reached from Help → About AutoDev. Self-contained; the
 *  version is passed in from the running core. */
export function AboutPanel(props: { version: string; onClose: () => void }) {
  const open = (url: string) => (e: MouseEvent) => {
    e.preventDefault();
    void openExternal(url).catch(() => {});
  };
  return (
    <div
      class="annotator-backdrop"
      onClick={(e) => e.target === e.currentTarget && props.onClose()}
    >
      <div class="handoff-modal about-modal">
        <div class="handoff-head">
          <strong>About AutoDev</strong>
          <span class="spacer" />
          <button onClick={props.onClose}>Close</button>
        </div>

        <div class="about-body">
          <div class="about-hero">
            <img class="about-logo" src={logoUrl} alt="" width="48" height="48" />
            <div>
              <div class="about-name">
                AutoDev <span class="about-version">v{props.version || "—"}</span>
              </div>
              <div class="muted">Run and manage many terminal coding agents in parallel.</div>
            </div>
          </div>

          <p>
            AutoDev is a desktop environment that runs and supervises many terminal coding agents —
            Claude Code, Codex, Antigravity, Pi, and any CLI you add — in parallel across your
            project workspaces. It wraps the agents you already use with a workspace model, live
            status, git-worktree isolation, voice and screenshot input, cross-agent annotation, an
            autonomous build loop, and a file-based extension surface.
          </p>

          <dl class="about-facts">
            <div>
              <dt>Version</dt>
              <dd>{props.version || "—"}</dd>
            </div>
            <div>
              <dt>Built with</dt>
              <dd>Tauri (Rust core) + SolidJS</dd>
            </div>
            <div>
              <dt>License</dt>
              <dd>GNU AGPL v3.0</dd>
            </div>
            <div>
              <dt>Source</dt>
              <dd>
                <a href="https://github.com/algorisys-oss/autodev" class="about-link" onClick={open("https://github.com/algorisys-oss/autodev")}>
                  github.com/algorisys-oss/autodev
                </a>
              </dd>
            </div>
          </dl>

          <p class="about-foot muted">
            Extend it with your own backends, prompt templates, skills, and JS extensions under
            <code> ~/.autodev/</code>. See <strong>Help → Documentation</strong> for the full guide.
          </p>

          <div class="about-madeby">
            Made by the{" "}
            <a href="https://www.algorisys.com" class="about-link" onClick={open("https://www.algorisys.com")}>
              Algorisys Open Source Team
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}
