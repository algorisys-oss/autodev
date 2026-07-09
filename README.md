# AutoDev

A desktop app for running and managing many terminal coding agents (Claude Code, Codex)
in parallel across multiple project workspaces. It wraps the agents you already use with
a workspace model, live status, git-worktree isolation, voice and screenshot input, and
an optional autonomous build loop.

Built with Tauri (Rust core) and SolidJS. See [`PLAN.md`](PLAN.md) for the full roadmap
and [`LOOPS.md`](LOOPS.md) for the engineering method the project follows.

> Status: early. Phase 0 (foundation) is done. Phases 1–9 are in progress — see
> [`handoff.md`](handoff.md) for exactly what works today.

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
directly from a snap shell.

## Usage

Today (Phase 0) the app opens a window that confirms the Rust core and UI are talking and
lets you cycle the theme (persisted to `~/.autodev/settings.json`). As phases land, this
section grows into the real workflow:

1. Open a **workspace** pointed at a folder that holds your projects.
2. Add the project directories you work in (API, app, UI, …).
3. Compose a prompt, `@`-mention the projects it needs for context, pick a difficulty,
   and launch one or more agents.
4. Watch each agent's status and terminal, isolate risky ones in a git worktree, and feed
   them voice or annotated-screenshot context.

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
