import { describe, it, expect, vi } from "vitest";
import { render } from "@solidjs/testing-library";

const gitWorktreeStatus = vi.fn();
vi.mock("../lib/ipc", () => ({
  gitWorktreeStatus: (p: string) => gitWorktreeStatus(p),
}));

import { StatusFooter } from "./status-footer";

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
});
