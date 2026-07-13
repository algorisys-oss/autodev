import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";

const openExternal = vi.fn((url: string) => {
  void url;
  return Promise.resolve();
});
vi.mock("../lib/ipc", () => ({ openExternal: (u: string) => openExternal(u) }));

import { AboutPanel } from "./about-panel";

describe("AboutPanel", () => {
  it("shows the version and the Algorisys credit, and opens the site externally", () => {
    const { getByText, getAllByText } = render(() => (
      <AboutPanel version="0.9.0" onClose={() => {}} />
    ));
    expect(getAllByText(/0\.9\.0/).length).toBeGreaterThan(0);
    const credit = getByText("Algorisys Open Source Team");
    expect(credit).toBeTruthy();
    fireEvent.click(credit);
    expect(openExternal).toHaveBeenCalledWith("https://www.algorisys.com");
  });

  it("closes when Close is clicked", () => {
    const onClose = vi.fn();
    const { getByText } = render(() => <AboutPanel version="0.9.0" onClose={onClose} />);
    fireEvent.click(getByText("Close"));
    expect(onClose).toHaveBeenCalled();
  });
});
