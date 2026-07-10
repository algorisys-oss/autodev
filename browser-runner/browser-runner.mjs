#!/usr/bin/env node
// Minimal real `browserCommand` runner for AutoDev's Browser Handoff.
//
//   node browser-runner.mjs <handoff-file>
//
// Reads AutoDev's structured handoff, opens a real Chromium at its "## Starting point" URL,
// captures a screenshot, and prints a report back (which AutoDev shows in the modal).
//
// This is a launcher/scaffold: it navigates to the right page and proves it with a screenshot.
// It does NOT autonomously complete the task — that needs an LLM-driven browser agent. Run with
// HEADLESS=0 to open a visible window and finish the task by hand.
import { readFileSync } from "node:fs";
import { chromium } from "playwright";

const file = process.argv[2];
if (!file) {
  console.error("usage: browser-runner.mjs <handoff-file>");
  process.exit(2);
}

const handoff = readFileSync(file, "utf8");
// Pull the starting URL out of the `## Starting point` section of the handoff.
const match = handoff.match(/##\s*Starting point\s*\n\s*(\S+)/i);
const url = match && /^https?:\/\//i.test(match[1]) ? match[1] : null;
if (!url) {
  console.log("No starting URL found in the handoff — open the relevant site manually.");
  process.exit(0);
}

const headless = process.env.HEADLESS !== "0"; // HEADLESS=0 to watch / drive it yourself
const browser = await chromium.launch({ headless });
try {
  const page = await browser.newPage();
  await page.goto(url, { waitUntil: "domcontentloaded", timeout: 30000 });
  const title = await page.title();
  const shot = file.replace(/\.[^./]*$/, "") + "-screenshot.png";
  await page.screenshot({ path: shot });
  console.log(`Opened ${url}`);
  console.log(`Page title: ${title}`);
  console.log(`Screenshot: ${shot}`);
  console.log(
    "Scaffold: navigated + captured. Finish the task interactively (HEADLESS=0), " +
      "or wire an LLM-driven agent here for full autonomy.",
  );
} finally {
  await browser.close();
}
