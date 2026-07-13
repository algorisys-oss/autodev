import { For, type JSX } from "solid-js";

/** A documentation section: `id` anchors the table-of-contents jump, `title` labels it, and
 *  `body` is the rendered content. Kept as data so the ToC and the content never drift. */
interface Section {
  id: string;
  title: string;
  body: () => JSX.Element;
}

const SECTIONS: Section[] = [
  {
    id: "overview",
    title: "What is AutoDev",
    body: () => (
      <>
        <p>
          AutoDev runs and supervises many terminal coding agents — Claude Code, Codex,
          Antigravity, and any you add yourself — <em>in parallel</em> across your project
          directories. You describe a task once; AutoDev launches one or more agents to do it,
          each in a real terminal you can watch and type into.
        </p>
        <p>
          It is an <strong>orchestrator</strong>, not a chatbot: it drives the agent CLIs you
          already use, and adds the tooling around them — parallel fan-out, git worktree
          isolation, voice, screenshots, a browser handoff, an autonomous loop, and file-based
          extensibility.
        </p>
        <p class="help-callout">
          Everything AutoDev stores lives under <code>~/.autodev/</code> on your machine. Nothing
          is sent anywhere except to the agent CLIs you launch.
        </p>
      </>
    ),
  },
  {
    id: "workspaces",
    title: "Workspaces & projects",
    body: () => (
      <>
        <p>
          A <strong>workspace</strong> is a named set of project directories you work on together
          (e.g. an API, a web app, a shared UI library). Keeping them side by side lets any agent
          reference any of them for context.
        </p>
        <ul>
          <li>Create a workspace from the left sidebar, then use <strong>“+dir”</strong> to add project directories to it.</li>
          <li>Your workspaces and projects are saved to disk and restored when you reopen the app.</li>
          <li>
            In a task, type <code>@ProjectName</code> to attach that project as extra context — the
            agent is given the directory to read from. Matching is fuzzy; a resolved mention shows
            as a green chip, an unmatched one as a red <code>@name?</code>.
          </li>
        </ul>
      </>
    ),
  },
  {
    id: "new-task",
    title: "Starting a task",
    body: () => (
      <>
        <p>
          The <strong>New task</strong> box launches a fresh agent for each run. (To continue an
          agent that is already running, type in <em>its</em> terminal instead — not here.)
        </p>
        <ul>
          <li><strong>Describe the task</strong> in plain language. <code>@mention</code> projects to add context.</li>
          <li><strong>Backend</strong> — which agent CLI to launch. The list includes any you have added (see “Extending AutoDev”).</li>
          <li><strong>Run in</strong> — which project directory the agent works in.</li>
          <li>
            <strong>Difficulty (1–10)</strong> — a slider that suggests how many agents to launch
            and whether to plan first. Low = one agent, no plan; high = several agents, plan mode,
            ultrathink. You can override any suggestion.
          </li>
          <li>
            <strong>Modes</strong> — <strong>Plan</strong> (agent proposes a plan before acting),
            <strong> Bypass</strong> (yolo: skip permission prompts — powerful, never the silent
            default), <strong>Ultrathink</strong> (deeper reasoning, Claude), and
            <strong> Isolate</strong> (run in a git worktree — see below).
          </li>
          <li>
            <strong>Per-agent prompts</strong> — give each agent its own instructions instead of a
            shared one. Turning this on auto-enables Isolate, since divergent tasks in one directory
            would collide.
          </li>
        </ul>
        <p>The three tools beside the box:</p>
        <ul>
          <li>🎤 <strong>Voice</strong> — record a spoken prompt; it is transcribed into the box.</li>
          <li>📷 <strong>Screenshot</strong> — capture the screen, annotate it, and attach it as visual context (cuts hallucinations).</li>
          <li>🌐 <strong>Browser handoff</strong> — generate a prompt for a browser AI to execute.</li>
        </ul>
        <p>
          When ready, click <strong>Launch</strong>. Each agent starts in its own terminal and
          appears in the grid.
        </p>
      </>
    ),
  },
  {
    id: "auto-split",
    title: "Auto-split",
    body: () => (
      <>
        <p>
          The <strong>✨ Auto-split</strong> button asks a quick, read-only classifier whether your
          task naturally breaks into independent pieces that can run in parallel, and pre-fills the
          fan-out for you — one agent per piece, each with its own sub-prompt.
        </p>
        <ul>
          <li>It may inspect the working directory to enumerate real work items (e.g. “convert every video in ./media” → one unit per file).</li>
          <li>A cohesive task (one bug fix) collapses to a single agent instead.</li>
          <li><strong>Nothing launches automatically</strong> — you review the proposed split, adjust, then Launch.</li>
          <li>
            In <strong>Settings</strong> you can enable <em>Auto-split on Launch</em>: the first
            Launch analyzes and pauses for review (unless the task is already split or you set the
            count by hand).
          </li>
        </ul>
      </>
    ),
  },
  {
    id: "agents",
    title: "Agents & terminals",
    body: () => (
      <>
        <p>Every running agent is a card in the grid. The colored dot shows its status:</p>
        <ul>
          <li><span class="help-dot running" /> <strong>running</strong> — actively working.</li>
          <li><span class="help-dot idle" /> <strong>idle</strong> — quiet, not waiting on you.</li>
          <li><span class="help-dot waiting" /> <strong>waiting</strong> — parked on a prompt (a y/n, a menu) and needs your input.</li>
          <li><span class="help-dot exited" /> <strong>exited</strong> — finished cleanly.</li>
          <li><span class="help-dot error" /> <strong>error</strong> — exited non-zero (crashed/failed).</li>
        </ul>
        <ul>
          <li>Click a card to focus its terminal below. Type there to reply to that agent.</li>
          <li><strong>Open in editor</strong> opens the agent’s working directory (or worktree) in your configured editor.</li>
          <li><strong>Kill</strong> stops one agent; <strong>Kill all</strong> stops every agent. Closing the app kills every agent — no orphaned processes.</li>
          <li>Press <strong>×</strong> on a finished card to remove it from the grid.</li>
        </ul>
      </>
    ),
  },
  {
    id: "worktrees",
    title: "Git worktree isolation",
    body: () => (
      <>
        <p>
          When several agents might edit the same repository, turn on <strong>Isolate</strong>.
          Each agent then runs in its own <code>git worktree</code> — a separate checkout on its own
          branch — so their changes never collide.
        </p>
        <ul>
          <li>The agent’s bar shows its worktree branch.</li>
          <li><strong>Merge</strong> merges the agent’s branch back into your target branch (it refuses if the target is dirty).</li>
          <li><strong>Remove</strong> prunes the worktree when you’re done.</li>
        </ul>
        <p>Skip isolation when agents work on different projects or clearly won’t collide.</p>
      </>
    ),
  },
  {
    id: "loops",
    title: "Autonomous loops",
    body: () => (
      <>
        <p>
          The <strong>Loops</strong> tab runs a task through an autonomous
          Planner → Generator → Evaluator cycle until it passes or hits the iteration cap.
        </p>
        <ul>
          <li><strong>Features</strong> — the list of things to build; <strong>Criteria</strong> — how “done” is judged.</li>
          <li><strong>Verify command</strong> — a shell command whose exit 0 means the tests pass. A failing verify blocks a “pass” even if the evaluator is happy — it is ground truth.</li>
          <li>Each role runs as a one-shot agent; when it exits, AutoDev parses its output and advances the loop. If a parse fails, the phase stays put and you continue with the manual controls.</li>
          <li><strong>Auto-run</strong> (off by default) chains the next role automatically for a hands-off run.</li>
          <li><strong>Auto-onboard</strong> (off by default) auto-accepts an agent’s “trust this folder?” prompt so an unattended run doesn’t stall.</li>
        </ul>
      </>
    ),
  },
  {
    id: "settings",
    title: "Settings",
    body: () => (
      <>
        <p>Open Settings with the ⚙ button in the header.</p>
        <ul>
          <li><strong>Theme</strong> and <strong>Default effort</strong> (high / extra-high).</li>
          <li>
            <strong>Pluggable commands</strong> — the shell commands AutoDev uses for voice
            transcription, screenshots, and the browser handoff. Use <code>{"{file}"}</code> where
            the tool should substitute the working file path; leave blank to disable that tool.
          </li>
          <li><strong>Editor</strong> — the command for “Open in editor” (e.g. <code>code</code>, <code>cursor</code>, <code>subl</code>). The path is appended.</li>
          <li><strong>Auto-split on Launch</strong> — analyze for a parallel split before the first fan-out.</li>
        </ul>
      </>
    ),
  },
  {
    id: "extending",
    title: "Extending AutoDev",
    body: () => (
      <>
        <p>
          AutoDev is extended with plain files under <code>~/.autodev/</code> — no rebuild, no code.
          Add the file, restart the app, and it’s picked up.
        </p>

        <h4>Add a backend (a new agent CLI)</h4>
        <p>
          Drop a JSON file in <code>~/.autodev/backends/</code> describing how to launch the CLI.
          It then appears in the Backend picker. A file whose <code>id</code> matches a built-in
          one overrides it (handy for retuning flags). Example
          <code> ~/.autodev/backends/opencode.json</code>:
        </p>
        <pre class="help-code">{`{
  "id": "opencode",
  "label": "OpenCode",
  "program": "opencode",
  "models": ["big", "small"],
  "bypassFlag": ["--yolo"],
  "modelFlag": "--model",
  "addDirFlag": "--add-dir",
  "images": { "mode": "flag", "flag": "-i" },
  "prompt": { "mode": "positional" }
}`}</pre>
        <p class="help-muted">
          Fields: <code>program</code> (the executable); <code>printFlag</code>/<code>planFlag</code>/
          <code>bypassFlag</code> (args emitted for those modes); <code>modelFlag</code>,
          <code>addDirFlag</code> (flag before each value); <code>addCwdToDirs</code> (add the working
          dir first); <code>images</code> (<code>flag</code> per image, or <code>appendToPrompt</code>
          with a <code>{"{path}"}</code> template); <code>prompt</code> (<code>positional</code> or
          <code>flag</code>). Only <code>id</code> and <code>program</code> are required.
        </p>
        <p class="help-muted">
          A ready-made <strong>Pi</strong> backend ships in <code>examples/backends/pi.json</code>.
          Install Pi (<code>npm i -g @earendil-works/pi-coding-agent</code>), log it in once with{" "}
          <code>pi</code> → <code>/login</code>, copy that file to
          <code> ~/.autodev/backends/pi.json</code>, and pick <strong>Pi</strong> as the backend.
        </p>

        <h4>Prompt templates</h4>
        <p>
          Put a Markdown file in <code>~/.autodev/templates/</code> — the filename is its name. In
          the task box, type <code>/name</code> and a suggestion appears; click it or press
          <strong> Tab</strong> to expand the template into the box. Any text you typed after
          <code> /name</code> is kept. Example: <code>~/.autodev/templates/refactor.md</code> →
          type <code>/refactor the parser</code>.
        </p>

        <h4>Skills</h4>
        <p>
          Put reference files (house style, conventions, snippets) in
          <code> ~/.autodev/skills/</code>. When that folder has any content, it is automatically
          added to <em>every</em> agent’s context on every backend — so all your agents follow the
          same guidance without you attaching it each time.
        </p>

        <h4>Extensions (advanced — runs code)</h4>
        <p>
          For behavior that data files can’t express, drop a self-contained JavaScript module in
          <code> ~/.autodev/extensions/</code>. Each file’s default export receives an
          <code> autodev</code> API to register lifecycle hooks and composer commands. Extensions
          load at startup — restart to pick up changes, and see which loaded (and any errors) in
          Settings.
        </p>
        <p class="help-callout">
          ⚠ Extensions run with the app’s full access (they can do anything AutoDev can). Only add
          files you trust — they are code, like a shell script in your home dir. There is no
          sandbox.
        </p>
        <pre class="help-code">{`// ~/.autodev/extensions/team.js
export default (autodev) => {
  // Add a team skills dir to every agent's context:
  autodev.hooks.onSpawn((o) => ({
    ...o, addDirs: [...(o.addDirs ?? []), '/team/skills'],
  }));

  // React when any agent exits:
  autodev.hooks.onExit((id, code) => console.log(id, 'exited', code));

  // Add a /standup slash-command to the composer:
  autodev.registerCommand('standup', 'Summarize what changed today and what is next.');
};`}</pre>
        <p class="help-muted">
          Hooks: <code>onSpawn</code> (rewrite launch options before an agent starts),
          <code> onOutput</code>, <code>onIdle</code>, <code>onWaiting</code>, <code>onExit</code>.
          Extensions must be a single self-contained file — bare <code>import</code> of other
          modules won’t resolve.
        </p>
      </>
    ),
  },
  {
    id: "data",
    title: "Where your data lives",
    body: () => (
      <>
        <p>Everything is under <code>~/.autodev/</code>:</p>
        <pre class="help-code">{`~/.autodev/
├─ settings.json     app settings
├─ workspaces.json   your workspaces & projects
├─ prompts.json      task history
├─ backends/         *.json  — custom agent backends
├─ templates/        *.md    — prompt templates (/name)
├─ skills/           files added to every agent's context
├─ logs/             per-agent output logs
└─ loops/            autonomous-loop state`}</pre>
      </>
    ),
  },
  {
    id: "tips",
    title: "Tips & troubleshooting",
    body: () => (
      <>
        <ul>
          <li><strong>An agent is stuck on “waiting.”</strong> It’s parked on a prompt — focus its terminal and answer it. For unattended loop runs, enable Auto-onboard.</li>
          <li><strong>A new backend/template/skill isn’t showing up.</strong> Make sure the file is in the right folder and restart the app; the folders are read at startup.</li>
          <li><strong>Bypass/yolo mode</strong> lets an agent act without asking permission. Use it deliberately — it’s never the silent default.</li>
          <li><strong>“Open in editor” does nothing.</strong> Set your editor command in Settings (default is <code>code</code>).</li>
          <li><strong>Nothing launches.</strong> Add a project directory to the workspace first, and check the backend’s CLI is installed and on your PATH.</li>
        </ul>
      </>
    ),
  },
];

/** In-app documentation. A self-contained Help panel — a table of contents on the left, the
 *  full guide on the right — so end users never need to leave the app for help. */
export function HelpPanel(props: { onClose: () => void }) {
  const jump = (id: string) => document.getElementById(`help-${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" });

  return (
    <div
      class="annotator-backdrop"
      onClick={(e) => e.target === e.currentTarget && props.onClose()}
    >
      <div class="handoff-modal help-modal">
        <div class="handoff-head">
          <strong>AutoDev — Help &amp; documentation</strong>
          <span class="spacer" />
          <button onClick={props.onClose}>Close</button>
        </div>

        <div class="help-body">
          <nav class="help-toc">
            <For each={SECTIONS}>
              {(s) => (
                <button class="help-toc-item" onClick={() => jump(s.id)}>
                  {s.title}
                </button>
              )}
            </For>
          </nav>

          <div class="help-content">
            <For each={SECTIONS}>
              {(s) => (
                <section id={`help-${s.id}`} class="help-section">
                  <h3>{s.title}</h3>
                  {s.body()}
                </section>
              )}
            </For>
          </div>
        </div>
      </div>
    </div>
  );
}
