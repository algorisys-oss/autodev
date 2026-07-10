# Recording a real GUI demo (headless, on a virtual display)

This documents exactly how [`demo/autodev-multi-agent-demo.mp4`](../demo/autodev-multi-agent-demo.mp4)
was produced: a real screen recording of the AutoDev desktop app running a 3-agent task, captured
on a **virtual display** so it never touches your real desktop, driven programmatically, with
real `claude` agents.

## Why a virtual display

AutoDev is a native (Tauri/WebKitGTK) desktop app. Recording the real screen would hijack your
mouse/keyboard and capture whatever is visible. Instead we run the app on an off-screen X server
(`Xvfb`), grab that display with `ffmpeg`, and drive the UI with `xdotool`. Nothing appears on
your monitor; your session is untouched.

## Prerequisites

```bash
sudo apt-get install -y xvfb    # virtual X server (the one non-scriptable install)
# already present on most dev boxes:  ffmpeg, xdotool
```

Also: a release build of the app, and the agent CLIs (`claude` / `codex` / `agy`) authenticated
for the same user.

## Steps

### 1. Build a runnable binary

The binary must embed the frontend. Use `tauri build` (which compiles with the `custom-protocol`
feature) — **not** plain `cargo build --release`, which leaves the app pointing at the dev server
and you'll see *"Could not connect to localhost"*.

```bash
npx tauri build --no-bundle      # ~1-2 min; skips the slow installer step, just the binary
# -> src-tauri/target/release/autodev
```

### 2. Start the virtual display

```bash
Xvfb :99 -screen 0 1600x1000x24 -nolisten tcp &
```

### 3. Launch the app on it

WebKitGTK needs software rendering on a headless display, and the GTK **x11** backend so the
window is capturable/drivable. Keep the repo's snap-env scrub (unset the `LD_LIBRARY_PATH` /
`GTK_*` vars — see `dev.sh`).

```bash
env -u LD_LIBRARY_PATH -u GTK_EXE_PREFIX -u GTK_PATH -u GDK_PIXBUF_MODULE_FILE \
    -u GSETTINGS_SCHEMA_DIR -u GIO_MODULE_DIR -u LOCPATH \
  DISPLAY=:99 GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 \
  WEBKIT_DISABLE_COMPOSITING_MODE=1 WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  src-tauri/target/release/autodev &
```

Tip: to skip driving the native folder-picker, pre-seed a workspace before launch:

```bash
cat > ~/.autodev/workspaces.json <<'JSON'
{ "workspaces": [ { "id": "demo", "name": "todo-app",
  "projects": [ { "name": "demo-project", "path": "/tmp/demo-project" } ] } ] }
JSON
# and make /tmp/demo-project a git repo so worktree isolation works
```

### 4. Record the display

```bash
ffmpeg -y -f x11grab -video_size 1280x800 -framerate 15 -i :99+0,0 \
  -c:v libx264 -preset ultrafast -pix_fmt yuv420p -movflags +faststart out.mp4 &
```

### 5. Drive the UI with xdotool

Find the window, then move/click/type at coordinates. "See" the state at any point by grabbing a
single frame (`ffmpeg ... -frames:v 1 frame.png`) and inspecting it — that feedback loop is how
you locate buttons without a human watching.

```bash
export DISPLAY=:99
xdotool mousemove 700 250 click 1                 # focus the task box
xdotool type --delay 45 "Create index.html, style.css and app.js for a to-do app."
xdotool mousemove 1211 336 click 1; xdotool key Up Up   # Agents: 1 -> 3
xdotool mousemove 620 375 click 1                 # tick "Isolate (worktree)"
xdotool mousemove 1193 376 click 1                # Launch 3 agents
```

### 6. Handle Claude Code's onboarding

Each agent runs `claude` in a **fresh worktree**, so Claude Code shows onboarding gates that need
a keypress in each agent's terminal (focus the agent card, click into its terminal, press Enter):

- **"Trust this folder?"** — always appears for a new directory. Enter accepts.
- **Bypass-permissions warning** — appears only if you ticked *Bypass permissions*; its default
  is the safe *No*, so Enter **exits** the agent (you'll see the app's `error` status). For an
  unattended build, leave Bypass **off** and rely on your global Claude `defaultMode` /
  `skipAutoPermissionPrompt` in `~/.claude/settings.json` so edits auto-apply.

### 7. Stop, encode, clean up

```bash
pkill -f x11grab                                   # stop recording
# speed up dead time + compress:
ffmpeg -i out.mp4 -filter:v "setpts=PTS/1.75" -c:v libx264 -crf 28 -preset medium \
  -pix_fmt yuv420p -movflags +faststart -an demo/autodev-multi-agent-demo.mp4
pkill -x autodev; pkill -x Xvfb                    # tear down
# remove temp worktrees/project, restore ~/.autodev/workspaces.json
```

## Gotchas learned the hard way

- `cargo build --release` ≠ `tauri build`. Only the latter embeds assets (custom-protocol);
  otherwise the window shows *"Could not connect to localhost"*.
- WebKitGTK renders a blank/black surface on a headless display without
  `LIBGL_ALWAYS_SOFTWARE=1` + `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
- `pgrep -f "release/autodev"` also matches *your own command line* (it contains that string).
  Use `ps -C autodev` to check whether the app is actually running.
- Real agents hit real onboarding prompts in fresh worktrees; budget for the trust-dialog keypress
  per agent, and prefer non-bypass mode for a hands-off build.
