# AutoDev

A desktop app for running and managing many terminal coding agents (Claude Code, Codex)
in parallel across multiple project workspaces. It wraps the agents you already use with
a workspace model, live status, git-worktree isolation, voice and screenshot input, and
an optional autonomous build loop.

Built with Tauri (Rust core) and SolidJS. See [`PLAN.md`](PLAN.md) for the full roadmap
and [`LOOPS.md`](LOOPS.md) for the engineering method the project follows.

> Status: all planned phases (0–9) are implemented — workspaces, multi-agent
> orchestration, prompt composer, git-worktree isolation, voice input, screenshot +
> annotate, browser handoff, and the autonomous Planner/Generator/Evaluator loop. See
> [`handoff.md`](handoff.md) for exactly what works today and the known gaps.

## Prerequisites

- **Rust** (stable) and **Cargo** — https://rustup.rs
- **Node.js** 20+ and **npm**
- **Linux system libraries** for Tauri (Debian/Ubuntu names):
  ```
  sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
    libayatana-appindicator3-dev build-essential
  ```
  On macOS install Xcode command line tools; on Windows install the WebView2 runtime and
  the MSVC build tools. See https://tauri.app/start/prerequisites/ for the current list.
- To actually drive agents you need their CLIs on your `PATH`: `claude` (Claude Code)
  and/or `codex`.

## Quick start

```bash
git clone https://github.com/algorisys-oss/autodev.git
cd autodev
./dev.sh setup     # install npm + cargo dependencies
./dev.sh dev       # launch the app with hot reload
```

The first `dev` run compiles the Rust core, so it takes a few minutes. After that,
frontend changes hot-reload instantly and Rust changes trigger a quick rebuild.

## The `dev.sh` script

One entry point for everything:

| Command | What it does |
|---|---|
| `./dev.sh setup` | Install all dependencies (npm + `cargo fetch`) |
| `./dev.sh dev` | Run the app in development with hot reload |
| `./dev.sh build` | Produce a production build and platform bundle |
| `./dev.sh test` | Run all tests: Vitest (frontend) + `cargo test` (core) |
| `./dev.sh lint` | Lint and typecheck: eslint, tsc, clippy, rustfmt |
| `./dev.sh verify` | Everything CI runs: lint + test + build |
| `./dev.sh help` | Show usage |

### Snap / VSCode note (Linux)

If you launch from a snap-packaged terminal or the snap build of VSCode, a native
GTK/WebKit binary can crash with `undefined symbol __libc_pthread_init` because snap
injects its own libraries into child processes. `dev.sh` strips those before launching,
so **always start the app with `./dev.sh dev`** rather than calling `npm run tauri dev`
directly from a snap shell. The same applies to running a build you produced locally —
launch it from a non-snap shell, or `unset LD_LIBRARY_PATH GTK_PATH GTK_EXE_PREFIX` first.

## Building a standalone executable

To produce a release build and self-contained installers for distribution:

```bash
./dev.sh build
```

This bundles the frontend (`vite build`), compiles the Rust core in release mode, and runs
`tauri build` (with the snap-env scrub, same as `dev`). It writes:

- **Standalone binary** — `src-tauri/target/release/autodev`. A single native executable; run
  it directly. Everything (frontend assets, Rust core) is embedded — there is no separate
  runtime to ship, though the host still needs the WebKit/GTK system libraries from
  [Prerequisites](#prerequisites).
- **Installers / portable bundles** — `src-tauri/target/release/bundle/`. The format matches
  the OS you build on (`bundle.targets` is `"all"`):
  - **Linux** — `appimage/AutoDev_0.1.0_amd64.AppImage` (a portable, double-clickable single
    file — the easiest thing to hand someone), plus `deb/` and `rpm/` packages.
  - **macOS** — `dmg/AutoDev_0.1.0_<arch>.dmg` and `macos/AutoDev.app`.
  - **Windows** — `msi/` (WiX) and `nsis/` (`.exe`) installers.

Notes:

- **Build on each target OS.** Tauri does not cross-compile between Linux/macOS/Windows in
  one step; run `./dev.sh build` on each platform you want to ship for.
- **Version** comes from `src-tauri/tauri.conf.json` (`version`); bump it there before a
  release so bundle filenames and the in-app version match.
- **Code signing / notarization** (macOS `.app`/`.dmg`, Windows installers) is not configured
  here; add signing identities to `tauri.conf.json` when you need distributable, unflagged
  binaries. Unsigned builds run fine locally and for internal sharing.
- The app still shells out to the agent CLIs at runtime — whoever runs the bundle needs
  `claude` and/or `codex` on their `PATH`.

## Usage

1. Open a **workspace** pointed at a folder that holds your projects, and add the project
   directories you work in (API, app, UI, …).
2. Compose a prompt, `@`-mention the projects it needs for context, pick a difficulty, and
   launch one or more agents (Claude Code or Codex).
3. Watch each agent's status dot and terminal — `running`, `idle`, `waiting` (blocked on a
   prompt), `exited`, or `error` — isolate risky ones in a git worktree, and feed them voice
   or annotated-screenshot context.
4. Open **Settings** (⚙ in the header) to configure the pluggable voice, screenshot, and
   browser-handoff commands.
5. Use the **Loops** tab to run an autonomous Planner → Generator → Evaluator loop against a
   project; tick **Auto-run** for a fully hands-off pass.

Configuration and state live at `~/.autodev/` (settings, prompt history, per-agent logs,
loop state).

## Project layout

```
autodev/
  src/                 SolidJS frontend
    lib/ipc.ts         typed wrappers over Tauri commands (the shared contract)
    App.tsx            app shell
  src-tauri/           Rust core
    src/
      lib.rs           Tauri builder, command registration
      commands.rs      #[tauri::command] handlers
      state.rs         on-disk state (~/.autodev)
      error.rs         command-boundary error type
    tauri.conf.json    app config
  dev.sh               developer entry point
  PLAN.md              build roadmap
  LOOPS.md             engineering method
  CLAUDE.md            guidance for AI agents working in this repo
```

State lives at `~/.autodev/` on disk, outside the repo.

## Contributing

Read [`CLAUDE.md`](CLAUDE.md) and [`LOOPS.md`](LOOPS.md) first — they define how work is
done here (scope lock, tests first, verify before claiming done). Run `./dev.sh verify`
before opening a PR.

## License

[GNU AGPL v3](LICENSE).
