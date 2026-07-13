import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@solidjs/testing-library";
import { HelpPanel } from "./help-panel";

describe("HelpPanel", () => {
  it("renders the documentation sections and extensibility docs", () => {
    const { getAllByText } = render(() => <HelpPanel onClose={() => {}} />);
    // A section every user needs, and the extensibility docs power users need.
    expect(getAllByText("What is AutoDev").length).toBeGreaterThan(0);
    expect(getAllByText("Extending AutoDev").length).toBeGreaterThan(0);
    // The concrete, self-service instructions — an example backend spec and the data layout.
    expect(getAllByText(/~\/\.autodev\/backends/).length).toBeGreaterThan(0);
    expect(getAllByText(/opencode/).length).toBeGreaterThan(0);
  });

  it("closes when Close is clicked", () => {
    const onClose = vi.fn();
    const { getByText } = render(() => <HelpPanel onClose={onClose} />);
    fireEvent.click(getByText("Close"));
    expect(onClose).toHaveBeenCalled();
  });
});
