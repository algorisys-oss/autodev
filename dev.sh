#!/usr/bin/env bash
# AutoDev developer script. One entry point for setup, running, testing, and linting.
#
# Usage:
#   ./dev.sh setup     Install all dependencies (npm + cargo fetch)
#   ./dev.sh dev        Run the app in development (hot reload)
#   ./dev.sh headless   Drive the orchestrator over JSONL (stdin/stdout, no GUI)
#   ./dev.sh build      Produce a production build + bundle
#   ./dev.sh run        Launch the built release binary (snap-env scrubbed)
#   ./dev.sh test       Run all tests (frontend Vitest + Rust cargo test)
#   ./dev.sh lint       Lint + typecheck everything (eslint, tsc, clippy, fmt)
#   ./dev.sh verify     Everything CI runs: lint + test + build
#   ./dev.sh release X.Y.Z   Bump version, tag vX.Y.Z, push (CI builds the GitHub release)
#   ./dev.sh help       Show this help
set -euo pipefail

cd "$(dirname "$0")"

# A native GTK/WebKit binary launched from inside a snap-confined shell (e.g. the
# snap build of VSCode) picks up snap's core20 libraries and GTK module paths,
# which crashes it with an "undefined symbol __libc_pthread_init" linker error.
# Strip those so the app links against system libraries. No-op outside snap.
scrub_snap_env() {
  unset LD_LIBRARY_PATH GTK_EXE_PREFIX GTK_PATH GDK_PIXBUF_MODULE_FILE \
    GSETTINGS_SCHEMA_DIR GIO_MODULE_DIR LOCPATH 2>/dev/null || true
}

usage() { sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; }

cmd="${1:-dev}"
case "$cmd" in
setup)
  npm install
  (cd src-tauri && cargo fetch)
  echo "Setup complete. Run ./dev.sh dev to start."
  ;;
dev)
  scrub_snap_env
  exec npm run tauri dev
  ;;
headless)
  # Drive the orchestrator over JSONL (P6), no GUI. Pipe `{"cmd":…}` lines in, read
  # `{"event":…}` lines out — e.g.  echo '{"cmd":"list"}' | ./dev.sh headless
  (cd src-tauri && cargo build --quiet --bin autodev-headless)
  exec src-tauri/target/debug/autodev-headless
  ;;
build)
  scrub_snap_env
  npm run build
  npm run tauri build
  ;;
run)
  # Launch the already-built release binary (with the snap-env scrub). Build it first if missing.
  bin="src-tauri/target/release/autodev"
  if [ ! -x "$bin" ]; then
    echo "No release binary yet — run ./dev.sh build first." >&2
    exit 1
  fi
  scrub_snap_env
  exec "$bin"
  ;;
test)
  npm run test
  (cd src-tauri && cargo test)
  ;;
lint)
  npm run lint
  (cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo fmt --check)
  ;;
verify)
  npm run lint
  npm run test
  npm run build
  (cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
  echo "verify: all checks passed"
  ;;
release)
  # Cut a versioned release: bump the version in both manifests, commit, tag, and push.
  # Pushing the tag triggers .github/workflows/release.yml, which builds every platform
  # and uploads the installers to a draft GitHub release.
  ver="${2:-}"
  if ! printf '%s' "$ver" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "usage: ./dev.sh release X.Y.Z   (semver, no leading v)" >&2
    exit 1
  fi
  if [ -n "$(git status --porcelain)" ]; then
    echo "working tree is dirty; commit or stash first" >&2
    exit 1
  fi
  # Update only the version line in each manifest, preserving formatting. Bump all of them so
  # they stay in sync — including Cargo.toml, since the in-app version comes from
  # env!("CARGO_PKG_VERSION").
  sed -i -E "0,/\"version\": *\"[^\"]+\"/s//\"version\": \"$ver\"/" package.json
  sed -i -E "0,/\"version\": *\"[^\"]+\"/s//\"version\": \"$ver\"/" src-tauri/tauri.conf.json
  sed -i -E "0,/^version = \"[^\"]+\"/s//version = \"$ver\"/" src-tauri/Cargo.toml
  sed -i "/^name = \"autodev\"\$/{n;s/^version = \".*\"/version = \"$ver\"/}" src-tauri/Cargo.lock
  git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
  git commit -m "Release v$ver"
  git tag "v$ver"
  git push origin HEAD "v$ver"
  echo "Pushed v$ver. The Release workflow builds every OS and publishes the release at"
  echo "https://github.com/algorisys-oss/autodev/releases (a few minutes)."
  ;;
help | -h | --help)
  usage
  ;;
*)
  echo "unknown command: $cmd" >&2
  usage
  exit 1
  ;;
esac
