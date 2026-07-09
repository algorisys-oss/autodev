#!/usr/bin/env bash
# AutoDev developer script. One entry point for setup, running, testing, and linting.
#
# Usage:
#   ./dev.sh setup     Install all dependencies (npm + cargo fetch)
#   ./dev.sh dev        Run the app in development (hot reload)
#   ./dev.sh build      Produce a production build + bundle
#   ./dev.sh test       Run all tests (frontend Vitest + Rust cargo test)
#   ./dev.sh lint       Lint + typecheck everything (eslint, tsc, clippy, fmt)
#   ./dev.sh verify     Everything CI runs: lint + test + build
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

usage() { sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'; }

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
build)
  scrub_snap_env
  npm run build
  npm run tauri build
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
help | -h | --help)
  usage
  ;;
*)
  echo "unknown command: $cmd" >&2
  usage
  exit 1
  ;;
esac
