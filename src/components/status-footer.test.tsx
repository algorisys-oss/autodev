import { describe, it, expect, vi, afterEach } from "vitest";
import { render } from "@solidjs/testing-library";

const gitWorktreeStatus = vi.fn();
vi.mock("../lib/ipc", () => ({
  gitWorktreeStatus: (p: string) => gitWorktreeStatus(p),
}));

import { StatusFooter } from "./status-footer";
import { setVoiceStatus } from "../lib/status";

const ws = (projects: { name: string; path: string }[]) => ({
  id: "w1",
  name: "yappy",
  projects,
});

describe("StatusFooter", () => {
  it("shows the git branch and a dirty marker for a project repo", async () => {
    gitWorktreeStatus.mockResolvedValue({ branch: "dev", dirty: true });
    const { findByText, getByTitle } = render(() => (
      <StatusFooter workspace={ws([{ name: "yappy", path: "/home/x/yappy" }])} />
    ));
    expect(await findByText("dev")).toBeTruthy();
    expect(getByTitle("uncommitted changes")).toBeTruthy();
  });

  it("does not show a dirty marker for a clean repo", async () => {
    gitWorktreeStatus.mockResolvedValue({ branch: "main", dirty: false });
    const { findByText, queryByTitle } = render(() => (
      <StatusFooter workspace={ws([{ name: "clean", path: "/home/x/clean" }])} />
    ));
    expect(await findByText("main")).toBeTruthy();
    expect(queryByTitle("uncommitted changes")).toBeNull();
  });

  it("marks a folder that is not a git work tree", async () => {
    gitWorktreeStatus.mockRejectedValue(new Error("not a work tree"));
    const { findByText } = render(() => (
      <StatusFooter workspace={ws([{ name: "plain", path: "/tmp/plain" }])} />
    ));
    expect(await findByText("not a git repo")).toBeTruthy();
  });

  it("shows a placeholder when no workspace is selected", () => {
    const { getByText } = render(() => <StatusFooter workspace={null} />);
    expect(getByText("No workspace selected")).toBeTruthy();
  });

  describe("voice status", () => {
    afterEach(() => setVoiceStatus(null));

    it("shows the live transcription status with a spinner", async () => {
      setVoiceStatus({ text: "Preparing speech model (first run)…", kind: "working" });
      const { findByText, container } = render(() => <StatusFooter workspace={null} />);
      expect(await findByText("Preparing speech model (first run)…")).toBeTruthy();
      expect(container.querySelector(".footer-spinner")).toBeTruthy();
      expect(container.querySelector(".footer-rec-dot")).toBeNull();
    });

    it("shows a red record-dot while recording, not a spinner", async () => {
      setVoiceStatus({ text: "Recording…", kind: "recording" });
      const { findByText, container } = render(() => <StatusFooter workspace={null} />);
      expect(await findByText("Recording…")).toBeTruthy();
      expect(container.querySelector(".footer-rec-dot")).toBeTruthy();
      expect(container.querySelector(".footer-status.recording")).toBeTruthy();
      expect(container.querySelector(".footer-spinner")).toBeNull();
    });

    it("hides the status slot when idle", () => {
      setVoiceStatus(null);
      const { container } = render(() => <StatusFooter workspace={null} />);
      expect(container.querySelector(".footer-status")).toBeNull();
    });
  });

  it("polls and reflects a branch change without a workspace change", async () => {
    // Same project path throughout — only the branch changes (a checkout). The footer must
    // pick it up on its next poll, not stay pinned to the branch it first loaded.
    gitWorktreeStatus.mockResolvedValueOnce({ branch: "dev", dirty: false });
    gitWorktreeStatus.mockResolvedValue({ branch: "feat/rich-view", dirty: false });
    const { findByText } = render(() => (
      <StatusFooter workspace={ws([{ name: "y", path: "/p" }])} pollMs={30} />
    ));
    expect(await findByText("dev")).toBeTruthy();
    expect(await findByText("feat/rich-view")).toBeTruthy();
  });
});
