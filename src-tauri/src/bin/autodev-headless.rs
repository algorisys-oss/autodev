//! Headless entry point (P6): drives the orchestrator over JSONL on stdin/stdout, no GUI.
//! See `autodev_lib::headless`. Pipe `{"cmd":…}` lines in, read `{"event":…}` lines out.

fn main() {
    autodev_lib::headless::run();
}
