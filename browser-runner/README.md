# browser-runner — a Playwright `browserCommand` for AutoDev

A minimal, real runner for AutoDev's [Browser handoff](../README.md#browser-handoff). AutoDev
writes the structured handoff to a file and runs your configured `browserCommand` on it; this
script reads that file, opens a real Chromium at the handoff's **Starting point** URL, screenshots
the page, and prints a report that AutoDev shows back in the modal.

It is deliberately **self-contained** (its own `package.json`) so it does not add Playwright to
the AutoDev app's dependencies.

## What it does (and doesn't)

- **Does:** parse the handoff, launch a real browser, navigate to the starting URL, capture a
  screenshot next to the handoff file, and report the page title + screenshot path.
- **Doesn't:** autonomously *complete* the task — that needs an LLM-driven browser agent. This is
  the launcher/scaffold: it gets you (or a future agent) to the right page. Run headed to finish
  by hand.

## Setup

```bash
cd browser-runner
npm install
npx playwright install chromium   # one-time browser download
```

## Wire it into AutoDev

In **Settings (⚙)** → `browserCommand`, or in `~/.autodev/settings.json`, set (absolute path):

```
node /ABS/PATH/TO/browser-runner/browser-runner.mjs {file}
```

`{file}` is replaced by AutoDev with the path to the handoff. To open a **visible** window (so you
can drive it yourself), prefix with `HEADLESS=0`:

```
HEADLESS=0 node /ABS/PATH/TO/browser-runner/browser-runner.mjs {file}
```

## Try it standalone

```bash
printf '## Goal\nTest\n## Starting point\nhttps://example.com\n' > /tmp/handoff.txt
node browser-runner.mjs /tmp/handoff.txt
# -> Opened https://example.com / Page title: Example Domain / Screenshot: /tmp/handoff-screenshot.png
```

## Making it autonomous

To go past the scaffold, replace the "navigate + screenshot" block with an agent loop: feed the
handoff text to an LLM, let it choose Playwright actions (`click`, `fill`, `goto`), and iterate
until the goal's `## Report back` is satisfied. Keep the same CLI contract (`<handoff-file>` in,
report on stdout) so AutoDev's wiring is unchanged.
