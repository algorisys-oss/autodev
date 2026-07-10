#!/usr/bin/env bash
# AutoDev multi-agent demo — runnable illustration of the real workflow.
#
# Three agents build a tiny calculator library in parallel, each isolated on its own
# `git worktree`/branch, then their work is merged back. This is exactly what AutoDev does
# when you type a task in the composer, set the agent count to 3, tick "Isolate (worktree)",
# launch, and later hit "Merge" on each agent — reproduced here with plain git + bash so it
# runs anywhere with no GUI, no CLI auth, and nothing left behind (it works in a temp dir).
#
# The desktop app drives real coding agents (Claude Code / Codex / Antigravity) instead of the
# canned steps below, and shows each one's live terminal and status — but the orchestration
# shape (fan out → isolate per worktree → run in parallel → merge) is identical.
set -euo pipefail

c1=$'\e[36m'; c2=$'\e[35m'; c3=$'\e[32m'; bold=$'\e[1m'; dim=$'\e[2m'; rst=$'\e[0m'

root="$(mktemp -d)"
repo="$root/calc"
trap 'rm -rf "$root"' EXIT

say()  { printf '\n%s%s%s\n' "$bold" "$1" "$rst"; }
note() { printf '%s%s%s\n' "$dim" "$1" "$rst"; }
agent_say() { printf '%s[agent %s]%s %s\n' "$2" "$1" "$rst" "$3"; }

say "▶ AutoDev multi-agent demo — 3 agents build a calc library in parallel"
note "project repo: $repo"

# The "project": a fresh git repo the agents will work in.
mkdir -p "$repo"; cd "$repo"
git init -q -b main
git config user.email demo@autodev.local
git config user.name  "AutoDev Demo"
printf '# calc\n\nA tiny calculator, built by three AutoDev agents in parallel.\n' > README.md
git add -A; git commit -qm "init project"

# Each agent gets its OWN git worktree on its OWN branch, so parallel edits never collide —
# this is AutoDev's "Isolate (worktree)". Create them up front (serialized), like the app does.
say "▶ Isolate: give each agent its own worktree + branch"
for id in 1 2 3; do
  git worktree add -q -b "autodev/agent-$id" "$root/wt-$id" main
  printf '  created worktree wt-%s on branch autodev/agent-%s\n' "$id" "$id"
done

# One agent = one task, run concurrently. Each works only inside its own worktree.
run_agent() {
  local id="$1" color="$2" file="$3" content="$4" wt="$root/wt-$1"
  agent_say "$id" "$color" "spawned — task: create $file"
  sleep "0.$(( id * 3 ))"
  printf '%s' "$content" > "$wt/$file"
  agent_say "$id" "$color" "wrote $file"
  git -C "$wt" add -A
  git -C "$wt" commit -qm "agent $id: add $file"
  agent_say "$id" "$color" "committed to autodev/agent-$id ✓"
}

say "▶ Fan out: 3 agents work in parallel (interleaved output = real concurrency)"
run_agent 1 "$c1" add.js 'export const add = (a, b) => a + b;
' &
run_agent 2 "$c2" sub.js 'export const sub = (a, b) => a - b;
' &
run_agent 3 "$c3" calc.test.js "import assert from 'node:assert';
import { add } from './add.js';
import { sub } from './sub.js';
assert.equal(add(2, 3), 5);
assert.equal(sub(5, 2), 3);
console.log('all calc tests passed');
" &
wait

# Fold each agent's branch back into main — AutoDev's "Merge" (no-fast-forward, like the app).
say "▶ Merge back: fold every agent's branch into main"
for id in 1 2 3; do
  git merge -q --no-ff "autodev/agent-$id" -m "Merge agent $id"
  printf '  merged autodev/agent-%s\n' "$id"
  git worktree remove --force "$root/wt-$id"
done

say "▶ Result: one assembled project from three parallel agents"
git ls-files | sed 's/^/  /'

say "▶ Run the agents' tests"
if command -v node >/dev/null 2>&1; then
  (cd "$repo" && node calc.test.js | sed 's/^/  /')
else
  note "  (node not installed — skipping test run)"
fi

say "✓ Done: 3 agents · 3 isolated worktrees · merged into one project"
