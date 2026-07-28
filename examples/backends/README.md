# Example backend specs

Drop any of these into `~/.autodev/backends/` (create the folder if needed) and restart
AutoDev — the backend appears in the New-task **Backend** picker. No code changes; this is the
P1 declarative-backend mechanism.

## `pi.json` — [Pi](https://github.com/earendil-works/pi) as a backend

Verified against `pi` v0.80.6 (`@earendil-works/pi-coding-agent`).

1. Install Pi so the `pi` binary is on your PATH:
   ```
   npm i -g @earendil-works/pi-coding-agent
   ```
2. Log Pi into a provider once (it needs an API key / OAuth): run `pi`, then `/login`.
3. Copy `pi.json` to `~/.autodev/backends/pi.json` and restart AutoDev.
4. Pick **Pi** as the backend and launch a task.

**Flag mapping** (what AutoDev's toggles become): the model dropdown → `--model`; the
bypass/yolo toggle → `--approve` (Pi has no per-action permission system — this just pre-trusts
project-local files so an unattended run doesn't stall); print/one-shot → `-p`; the prompt is
passed positionally.

**Current limitations** (this is Pi-as-an-interactive-cell, not the deeper RPC embedding):
- `@`-mentioned *other* project dirs aren't forwarded — Pi has no `--add-dir`. The agent still
  runs in the selected project directory and auto-discovers `AGENTS.md`/`CLAUDE.md` there.
- Plan mode isn't mapped (Pi's `--plan` comes from an optional extension, not core).
- Attached screenshots aren't forwarded (Pi takes images via `@image.png`, which the current
  spec format can't emit).
- Pi's own extensions (e.g. pi-annotate) run *inside* the Pi cell; AutoDev doesn't yet drive
  Pi's RPC mode to surface them in AutoDev's UI (that's the still-planned P7).

## Telling AutoDev when your backend is busy or waiting

AutoDev reads each agent's state — **working**, **waiting on you**, or **idle** — from what the
agent draws in its terminal. The signatures it matches are per-backend, in an optional
`detection` block:

```json
{
  "id": "opencode",
  "program": "opencode",
  "detection": {
    "busy": ["esc interrupt"],
    "blocked": ["allow this command?"],
    "menuCursor": true
  }
}
```

- **`busy`** — text the agent shows *while it is mid-turn*, usually its interrupt hint. This is
  what keeps a long silent think reading as working instead of decaying to idle, so it is the
  one worth getting right.
- **`blocked`** — text it shows when it is parked on you: an approval, permission, or y/n prompt.
- **`menuCursor`** — also treat a selection cursor on a numbered option (`❯ 1. Yes`) as parked.

Both lists are case-insensitive substrings matched against the bottom of the agent's screen —
not regexes, and not the raw byte stream, so a prompt the agent has already answered and
repainted away does not keep reading as waiting.

Signatures are never shared between backends: a spec with no `detection` block still shows
working and idle from output activity, but is never reported as waiting, because guessing with
another agent's prompt shapes produces a card that says "needs you" when nothing does. Find your
agent's strings by running it and looking at the footer it draws while it works.
